// Copyright (c) 2018 Weihang Lo
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::io::{self, BufReader};
use std::fs;
use std::path::{PathBuf, Path};

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
    ContentEncoding,
    ContentLength,
    ContentType,
    ETag,
    EntityTag,
    Headers,
    LastModified,
    Range,
    RangeUnit,
    Server,
    Vary,
};
use unicase::Ascii;
use percent_encoding::percent_decode;
use tera::{Tera, Context};
use mime_guess::get_mime_type_opt;
use ignore::gitignore::Gitignore;
use ignore::WalkBuilder;

use ::cli::Args;
use ::http::conditional_requests::{
    is_fresh,
    is_precondition_failed,
};
use ::http::range_requests::{
    is_range_fresh,
    is_satisfiable_range,
    extract_range,
};
use ::http::content_encoding::{
    get_prior_encoding,
    compress,
};

const SERVER_VERSION: &str =
    concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));

#[derive(Debug, Serialize, Eq, PartialEq, Ord, PartialOrd)]
pub struct FileItem {
    is_file: bool, // Indicate that file is a directory.
    name: String,
    path: String,
}

/// Run the server.
pub fn serve(args: Args) {
    let server = Http::new().bind(&args.address(), move || {
        Ok(MyService::new(args.to_owned()))
    }).unwrap();
    server.run().unwrap();
}

struct MyService {
    args: Args,
    gitignore: Gitignore,
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
    pub fn new(args: Args) -> Self {
        let gitignore = Gitignore::new(args.path.join(".gitignore")).0;
        Self {
            args,
            gitignore,
        }
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
            self.args.path.join(path)
        };

        // Construct response.
        let mut response = Response::new();
        let mut headers = Headers::new();
        headers.set(Server::new(SERVER_VERSION));

        // CORS headers
        if self.args.cors {
            headers.set(AccessControlAllowOrigin::Any);
            headers.set(AccessControlAllowHeaders(vec![
                Ascii::new("Range".to_owned()),
                Ascii::new("Content-Type".to_owned()),
                Ascii::new("Accept".to_owned()),
                Ascii::new("Origin".to_owned()),
            ]));
        }

        // 404 NotFound for
        //
        // 1. path does not exist
        // 2. is a hidden path without `--all` flag
        // 3. ignore by .gitignore behind `--no-ignore` flag
        if !req_path.exists()
            || (!self.args.all && is_hidden(&req_path))
            || (self.args.ignore &&
                self.gitignore.matched(&req_path, req_path.is_dir()).is_ignore()
            ) {
            let body = "Not Found";
            headers.set(ContentLength(body.len() as u64));
            return response
                .with_status(StatusCode::NotFound)
                .with_headers(headers)
                .with_body(body)
        }

        // Prepare response body.
        // Being mutable for further modification.
        let mut body: io::Result<Vec<u8>> = Ok(vec![]);

        // Extra process for serving files.
        if req_path.is_dir() {
            body = handle_dir(
                &req_path,
                &self.args.path,
                self.args.all,
                self.args.ignore,
            );
        } else {
            // Cache-Control.
            headers.set(CacheControl(vec![
                CacheDirective::Public,
                CacheDirective::MaxAge(self.args.cache),
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

        let mut body = body.unwrap_or_else(|e| Vec::from(format!("Error: {}", e)));

        // Deal with compression.
        if self.args.compress {
            let encoding = get_prior_encoding(&req);
            if let Ok(buf) = compress(&body, &encoding) {
                body = buf;
                // Representation varies, so responds with a `Vary` header.
                headers.set(ContentEncoding(vec![encoding]));
                headers.set(Vary::Items(vec![
                    Ascii::new("Accept-Encoding".to_owned())
                ]));
            }
        }

        // Common headers
        headers.set(AcceptRanges(vec![RangeUnit::Bytes]));
        headers.set(ContentType(guess_mime_type(&req_path)));
        headers.set(ContentLength(body.len() as u64));

        response
            .with_headers(headers)
            .with_body(body)
    }
}

// Detect is file hidden.
fn is_hidden(path: &Path) -> bool {
    path.file_name()
        .and_then(|s| s.to_str())
        .map(|s| s.starts_with("."))
        .unwrap_or(false)
}

/// Send a HTML page of all files under the path.
///
/// # Parameters
///
/// * `dir_path` - Directory to be listed files.
/// * `base_path` - The base path resolving all filepaths under `dir_path`.
/// * `show_all` - Whether to show hidden and 'dot' files.
/// * `with_ignore` - Whether to respet gitignore files.
fn handle_dir(
    dir_path: &Path,
    base_path: &Path,
    show_all: bool,
    with_ignore: bool,
) -> io::Result<Vec<u8>> {
    // Prepare dirname of current dir relative to base path.
    let (dir_name, paths) = {
        let dir_name = base_path.file_name().unwrap_or_default()
            .to_str().unwrap();
        let path = dir_path.strip_prefix(base_path).unwrap();
        let path_names = path.iter()
            .map(|s| s.to_str().unwrap());
        let abs_paths = path.iter()
            .scan(PathBuf::new(), |state, path| {
                state.push(path);
                Some(state.to_owned())
            })
            .map(|s| format!("/{}", s.to_str().unwrap()));
        // Tuple structure: (name, path)
        let paths = vec![(dir_name, String::from("/"))]
            .into_iter()
            .chain(path_names.zip(abs_paths))
            .collect::<Vec<_>>();
        (dir_name, paths)
    };

    let files_iter = WalkBuilder::new(dir_path)
        .standard_filters(false) // Disable all standard filters.
        .git_ignore(with_ignore)
        .hidden(!show_all) // Filter out hidden entries on demand.
        .max_depth(Some(1)) // Do not traverse subpaths.
        .build()
        .filter_map(|entry| entry.ok())
        .filter(|entry| dir_path != entry.path()) // Exclude `.`
        .map(|entry| {
            let abs_path = entry.path();
            // Exclude directories only (include symlinks).
            let is_file = !abs_path.is_dir();
            // Get relative path.
            let rel_path = abs_path.strip_prefix(base_path).unwrap();
            let name = rel_path.file_name()
                .and_then(|s| s.to_str())
                .unwrap_or_default()
                .to_owned();
            // Construct hyperlink.
            let path = format!("/{}", rel_path.to_str().unwrap_or_default());
            FileItem { name, path, is_file }
        });

    let mut files = if base_path == dir_path {
        // CWD == base dir
        files_iter.collect::<Vec<_>>()
    } else {
        // CWD == sub dir of base dir
        // Append an item for popping back to parent directory.
        let path = format!("/{}", dir_path
            .parent().unwrap()
            .strip_prefix(base_path).unwrap()
            .to_str().unwrap()
        );
        vec![FileItem {
            name: "..".to_owned(),
            path,
            is_file: false,
        }].into_iter().chain(files_iter).collect::<Vec<_>>()
    };
    // Sort files (dir-first and lexicographic ordering).
    files.sort_unstable();

    // Render page with Tera template engine.
    let mut context = Context::new();
    context.add("files", &files);
    context.add("dir_name", &dir_name);
    context.add("paths", &paths);
    context.add("style", include_str!("static/style.css"));
    let page = Tera::one_off(include_str!("static/index.html"), &context, true)
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
///
/// # Parameters
///
/// * `file_path` - Path to the file that is going to send.
/// * `range` - Tuple of `(start, end)` range (inclusive).
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

/// Guess MIME type from a path.
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
