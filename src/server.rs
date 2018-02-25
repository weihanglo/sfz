// Copyright (c) 2018 Weihang Lo. All rights reserved.
//
// See the LICENSE file at the top-level directory of this distribution.

use std::io::{self, BufReader};
use std::{env, fs};
use std::path::Path;
use std::net::SocketAddr;

use futures;
use futures::future::Future;
use hyper::server::{Http, Request, Response, Service};
use hyper::{StatusCode, mime, Error};
use hyper::header::{
    AcceptRanges,
    AccessControlAllowHeaders,
    AccessControlAllowOrigin,
    CacheControl,
    CacheDirective,
    ContentLength,
    ContentType,
    ETag,
    EntityTag,
    Headers,
    LastModified,
    Range,
    RangeUnit,
    Server,
};
use unicase::Ascii;
use percent_encoding::percent_decode;
use tera::{Tera, Context};
use mime_guess::get_mime_type_opt;

use ::conditional_requests::{
    is_fresh,
    is_precondition_failed,
};
use ::range_requests::{
    is_range_fresh,
    is_satisfiable_range,
    extract_range,
};

const SERVER_VERSION: &str =
    concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));

#[derive(Debug, Clone)]
pub struct ServerOptions {
    pub cache: u32,
    pub cors: bool,
}

impl Default for ServerOptions {
    fn default() -> Self {
        Self {
            cache: 0,
            cors: false,
        }
    }
}

#[derive(Debug, Serialize, Eq, PartialEq, Ord, PartialOrd)]
pub struct FileItem {
    is_file: bool, // Indicate that file is a directory.
    name: String,
    path: String,
}

/// Run the server.
pub fn serve(addr: &SocketAddr, options: ServerOptions) {
    let server = Http::new().bind(&addr, move || {
        Ok(MyService::new(options.clone()))
    }).unwrap();
    server.run().unwrap();
}

struct MyService {
    options: ServerOptions,
}

impl Service for MyService {
    type Request = Request;
    type Response = Response;
    type Error = Error;
    type Future = Box<Future<Item=Self::Response, Error=Self::Error>>;

    fn call(&self, req: Self::Request) -> Self::Future {
        let resp = self.handle_request(req);
        Box::new(futures::future::ok(resp))
    }
}

impl MyService {
    pub fn new(options: ServerOptions) -> Self {
        Self { options }
    }

    /// Request handler for `MyService`.
    fn handle_request(&self, req: Request) -> Response {
        let req_path = {
            // Remove leading slash.
            let path = &req.path()[1..].as_bytes();
            // URI percent decode.
            let path = percent_decode(path)
                .decode_utf8()
                .unwrap()
                .into_owned();
            env::current_dir().unwrap().join(path)
        };

        // Construct response.
        let mut response = Response::new();
        let mut headers = Headers::new();
        headers.set(Server::new(SERVER_VERSION));

        // Prepare response body.
        // Being mutable for further modification.
        let mut body: io::Result<Vec<u8>> = Ok(vec![]);

        // CORS headers
        if self.options.cors {
            headers.set(AccessControlAllowOrigin::Any);
            headers.set(AccessControlAllowHeaders(vec![
                Ascii::new("Range".to_owned()),
                Ascii::new("Content-Type".to_owned()),
                Ascii::new("Accept".to_owned()),
                Ascii::new("Origin".to_owned()),
            ]));
        }

        // Extra process for serving files.
        if req_path.is_dir() {
            body = handle_dir(&req_path);
        } else {
            // Cache-Control.
            headers.set(CacheControl(vec![
                CacheDirective::Public,
                CacheDirective::MaxAge(self.options.cache),
            ]));
            // Last-Modified-Time from file metadata _mtime_.
            let (mtime, size) = fs::metadata(&req_path)
                .map(|meta| (meta.modified().unwrap(), meta.len()))
                .unwrap();
            let last_modified = LastModified(mtime.into());
            // Concatenate _mtime_ and _file size_ to form a strong validator.
            let etag = {
                let mtime = mtime
                    .duration_since(::std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                ETag(EntityTag::strong(format!("{}-{}", mtime, size)))
            };

            // Validate preconditions of conditional requests.
            if is_precondition_failed(&req, &etag, &last_modified) {
                return response
                    .with_status(StatusCode::PreconditionFailed)
                    .with_headers(headers)
            }

            // Validate cache freshness.
            if is_fresh(&req, &etag, &last_modified) {
                return response
                    .with_status(StatusCode::NotModified)
                    .with_headers(headers)
            }

            // Range Request support.
            let mut is_range_request = false;
            if let Some(range) = req.headers().get::<Range>() {
                match (
                    is_range_fresh(&req, &etag, &last_modified),
                    is_satisfiable_range(range, size as u64)
                ) {
                    (true, Some(content_range)) => {
                        // 206 Partial Content.
                        is_range_request = true;
                        if let Some(range) = extract_range(&content_range) {
                            body = handle_file_with_range(&req_path, range);
                        }
                        headers.set(content_range);
                        response.set_status(StatusCode::PartialContent);
                    }
                    // Respond entire entity if Range header contains
                    // unsatisfiable range.
                    _ => (),
                }
            }

            if !is_range_request {
                body = handle_file(&req_path);
            }
        }

        let body = body.unwrap_or_else(|e| Vec::from(format!("Error: {}", e)));

        // Common headers
        headers.set(AcceptRanges(vec![RangeUnit::Bytes]));
        headers.set(ContentType(guess_mime_type(&req_path)));
        headers.set(ContentLength(body.len() as u64));

        response
            .with_headers(headers)
            .with_body(body)
    }
}

/// Send a HTML page of all files under the path.
fn handle_dir(dir_path: &Path) -> io::Result<Vec<u8>> {
    let mut files = Vec::new();
    let base_path = &env::current_dir()?;

    // Prepare dirname of current dir relative to base path.
    let (dir_name, paths) = {
        let dir_name = base_path.file_name().unwrap().to_str().unwrap();
        let path = dir_path.strip_prefix(base_path).unwrap();
        let path_names = path.iter()
            .map(|s| s.to_str().unwrap());
        let abs_paths = path.iter()
            .scan(::std::path::PathBuf::new(), |state, path| {
                state.push(path);
                Some(state.to_owned())
            })
            .map(|s| format!("/{}", s.to_str().unwrap()));
        let mut paths = path_names
            .zip(abs_paths)
            .collect::<Vec<_>>();
        paths.insert(0, (dir_name, String::from("/")));
        (dir_name, paths)
    };

    for entry in dir_path.read_dir()? {
        entry?.path()
            .strip_prefix(base_path) // Strip prefix to build a relative path.
            .and_then(|rel_path| {
                // Construct file name.
                let name = rel_path
                    .file_name().unwrap()
                    .to_str().unwrap()
                    .to_owned();
                // Construct hyperlink.
                let path = format!("/{}", rel_path.to_str().unwrap());
                let item = FileItem {
                    name,
                    path,
                    is_file: !rel_path.is_dir(),
                };
                files.push(item);
                Ok(())
            }).unwrap_or(()); // Prevent returning Result.
    }

    files.sort();

    // Item for popping back to parent directory.
    if base_path != dir_path {
        let path = format!("/{}", dir_path
            .parent().unwrap()
            .strip_prefix(base_path).unwrap()
            .to_str().unwrap()
        );
        let item = FileItem {
            name: "..".to_owned(),
            path,
            is_file: false,
        };
        files.insert(0, item);
    }

    // Render page with Tera template engine.
    let mut context = Context::new();
    context.add("files", &files);
    context.add("dir_name", &dir_name);
    context.add("paths", &paths);
    context.add("style", include_str!("style.css"));
    let page = Tera::one_off(include_str!("template.html"), &context, true)
        .unwrap_or_else(|e| format!("500 Internal server error: {}", e));
    Ok(Vec::from(page))
}

/// Send a buffer of file to client.
fn handle_file(file_path: &Path) -> io::Result<Vec<u8>> {
    use std::io::prelude::*;
    let f = fs::File::open(file_path)?;
    let mut buffer = Vec::new();
    BufReader::new(f).read_to_end(&mut buffer)?;
    Ok(buffer)
}

/// Send a buffer with specific range.
fn handle_file_with_range(
    file_path: &Path,
    range: (u64, u64),
) -> io::Result<Vec<u8>> {
    use std::io::SeekFrom;
    use std::io::prelude::*;
    let (start, end) = range; // TODO: handle end - start < 0
    if end <= start {
        return Err(io::Error::from(io::ErrorKind::InvalidData))
    }
    let mut f = fs::File::open(file_path)?;
    let mut buffer = Vec::new();
    f.seek(SeekFrom::Start(start))?;
    BufReader::new(f)
        .take(end - start)
        .read_to_end(&mut buffer)?;
    Ok(buffer)
}

/// Guess MIME type of a path.
/// Return `text/html` if the path refers to a directory.
fn guess_mime_type(path: &Path) -> mime::Mime {
    if path.is_dir() {
        mime::TEXT_HTML_UTF_8
    } else {
        match path.extension() {
            Some(ext) => {
                get_mime_type_opt(ext.to_str().unwrap_or(""))
                    .unwrap_or(mime::TEXT_PLAIN_UTF_8)
            }
            None => mime::TEXT_PLAIN_UTF_8,
        }
    }
}
