// Copyright (c) 2018 Weihang Lo
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

mod extensions;

use std::io::{self, BufReader};
use std::fs;
use std::path::{PathBuf, Path};
use std::convert::AsRef;

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
    LastModified,
    Range,
    RangeUnit,
    Server,
    Vary,
};
use unicase::Ascii;
use percent_encoding::percent_decode;
use tera::{Tera, Context};
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
use self::extensions::{MimeExt, PathExt, SystemTimeExt};

const SERVER_VERSION: &str =
    concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));

#[derive(Debug, Serialize, Eq, PartialEq, Ord, PartialOrd)]
pub struct FileItem {
    is_file: bool, // Indicate that file is a normal file.
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
        Self { args, gitignore }
    }

    /// Construct file path from request path.
    fn file_path_from_path(&self, path: &str) -> PathBuf {
        // Remove leading slash.
        let path = path[1..].as_bytes();
        // URI percent decode.
        let path = percent_decode(path)
            .decode_utf8()
            .unwrap()
            .into_owned();
        self.args.path.join(path)
    }

    /// Enable cross-origin resource sharing for given response.
    fn enable_cors(&self, res: &mut Response) {
        if self.args.cors {
            res.headers_mut().set(AccessControlAllowOrigin::Any);
            res.headers_mut().set(AccessControlAllowHeaders(vec![
                Ascii::new("Range".to_owned()),
                Ascii::new("Content-Type".to_owned()),
                Ascii::new("Accept".to_owned()),
                Ascii::new("Origin".to_owned()),
            ]));
        }
    }

    /// Determine critera if the file should be served or not (404 NotFound).
    ///
    /// 404 NotFound for
    ///
    /// 1. path does not exist
    /// 2. is a hidden path without `--all` flag
    /// 3. ignore by .gitignore behind `--no-ignore` flag
    fn should_return_not_found<P: AsRef<Path>>(
        &self, 
        path: P,
        res: &mut Response
    ) -> bool {
        let path = path.as_ref();
        if !path.exists() || 
            (!self.args.all && path.is_hidden()) || 
            (self.args.ignore &&
                self.gitignore.matched(path, path.is_dir()).is_ignore()
            ) {
            let body = "Not Found";
            res.headers_mut().set(ContentLength(body.len() as u64));
            res.set_status(StatusCode::NotFound);
            res.set_body(body);
            return true
        }
        false
    }

    /// Request handler for `MyService`.
    fn handle_request(&self, req: Request) -> Response {
        let path = &self.file_path_from_path(req.path());

        // Construct response.
        let mut res = Response::new();
        res.headers_mut().set(Server::new(SERVER_VERSION));

        // CORS headers
        self.enable_cors(&mut res);

        // Check critera if the file cannot be served (404 NotFound)
        if self.should_return_not_found(path, &mut res) {
            return res
        }

        // Prepare response body.
        // Being mutable for further modification.
        let mut body: io::Result<Vec<u8>> = Ok(vec![]);

        // Extra process for serving files.
        if path.is_dir() {
            body = handle_dir(
                path,
                &self.args.path,
                self.args.all,
                self.args.ignore,
            );
        } else {
            // Cache-Control.
            res.headers_mut().set(CacheControl(vec![
                CacheDirective::Public,
                CacheDirective::MaxAge(self.args.cache),
            ]));
            // Last-Modified-Time from file metadata _mtime_.
            let mtime = path.mtime();
            let size = path.size();
            let last_modified = LastModified(mtime.into());
            // Concatenate _mtime_ and _file size_ to form a strong validator.
            let etag = ETag(EntityTag::strong(
                format!("{}-{}", mtime.timestamp_sec(), size))
            );

            // Validate preconditions of conditional requests.
            if is_precondition_failed(&req, &etag, &last_modified) {
                return res.with_status(StatusCode::PreconditionFailed)
            }

            // Validate cache freshness.
            if is_fresh(&req, &etag, &last_modified) {
                return res.with_status(StatusCode::NotModified)
            }

            // Range Request support.
            if let Some(range) = req.headers().get::<Range>() {
                match (
                    is_range_fresh(&req, &etag, &last_modified),
                    is_satisfiable_range(range, size as u64)
                ) {
                    (true, Some(content_range)) => {
                        // 206 Partial Content.
                        if let Some(range) = extract_range(&content_range) {
                            body = handle_file_with_range(path, range);
                        }
                        res.headers_mut().set(content_range);
                        res.set_status(StatusCode::PartialContent);
                    }
                    // Respond entire entity if Range header contains
                    // unsatisfiable range.
                    _ => (),
                }
            }

            if res.status() != StatusCode::PartialContent {
                body = handle_file(path);
            }
            res.headers_mut().set(last_modified);
            res.headers_mut().set(etag);
        }

        let mut body = body
            .unwrap_or_else(|e| Vec::from(format!("Error: {}", e)));
        let mime_type = path.mime().unwrap_or_else(|| if path.is_dir() {
            mime::TEXT_HTML_UTF_8
        } else {
            mime::TEXT_PLAIN_UTF_8
        });


        // Deal with compression.
        // Prevent compression on partial responses and media contents.
        if self.args.compress &&
            res.status() != StatusCode::PartialContent && 
            !mime_type.is_media() {
            let encoding = get_prior_encoding(&req);
            if let Ok(buf) = compress(&body, &encoding) {
                body = buf;
                // Representation varies, so responds with a `Vary` header.
                res.headers_mut().set(ContentEncoding(vec![encoding]));
                res.headers_mut().set(Vary::Items(vec![
                    Ascii::new("Accept-Encoding".to_owned())
                ]));
            }
        }

        // Common headers
        res.headers_mut().set(AcceptRanges(vec![RangeUnit::Bytes]));
        res.headers_mut().set(ContentType(mime_type));
        res.headers_mut().set(ContentLength(body.len() as u64));

        res.with_body(body)
    }
}

/// Send a HTML page of all files under the path.
///
/// # Parameters
///
/// * `dir_path` - Directory to be listed files.
/// * `base_path` - The base path resolving all filepaths under `dir_path`.
/// * `show_all` - Whether to show hidden and 'dot' files.
/// * `with_ignore` - Whether to respet gitignore files.
fn handle_dir<P1: AsRef<Path>, P2: AsRef<Path>>(
    dir_path: P1,
    base_path: P2,
    show_all: bool,
    with_ignore: bool,
) -> io::Result<Vec<u8>> {
    let base_path = base_path.as_ref();
    let dir_path = dir_path.as_ref();
    // Prepare dirname of current dir relative to base path.
    let (dir_name, paths) = {
        let dir_name = base_path.filename_str();
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
            let name = rel_path.filename_str().to_owned();
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
    let mut ctx = Context::new();
    ctx.add("files", &files);
    ctx.add("dir_name", &dir_name);
    ctx.add("paths", &paths);
    ctx.add("style", include_str!("style.css"));
    let page = Tera::one_off(include_str!("index.html"), &ctx, true)
        .unwrap_or_else(|e| format!("500 Internal server error: {}", e));
    Ok(Vec::from(page))
}

/// Send a buffer of file to client.
fn handle_file<P: AsRef<Path>>(file_path: P) -> io::Result<Vec<u8>> {
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
fn handle_file_with_range<P: AsRef<Path>>(
    file_path: P,
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
