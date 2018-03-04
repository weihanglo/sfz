// Copyright (c) 2018 Weihang Lo
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::io::{self, BufReader};
use std::fs::File;
use std::path::{PathBuf, Path};
use std::convert::AsRef;
use std::str::Utf8Error;
use std::sync::Arc;

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

use cli::Args;
use http::conditional_requests::{
    is_fresh,
    is_precondition_failed,
};
use http::range_requests::{
    is_range_fresh,
    is_satisfiable_range,
    extract_range,
};
use http::content_encoding::{
    get_prior_encoding,
    compress,
};
use BoxResult;
use extensions::{MimeExt, PathExt, SystemTimeExt, PathType};

const SERVER_VERSION: &str =
    concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));

/// Serializable `Item` that would be passed to Tera for template rendering.
/// The order of struct fields is deremined to ensure sorting precedence.
#[derive(Debug, Serialize, Eq, PartialEq, Ord, PartialOrd)]
pub struct Item {
    path_type: PathType,
    name: String,
    path: String,
}

/// Run the server.
pub fn serve(args: Arc<Args>) -> BoxResult<()> {
    let address = args.address()?;
    Http::new()
        .bind(&address, move || Ok(MyService::new(args.to_owned())))
        .and_then(|server| {
            let address = server.local_addr()?;
            println!("Files served on http://{}", address);
            server.run()
        })
        .or_else(|err| bail!("error: cannot establish server: {}", err))
}

struct MyService {
    args: Arc<Args>,
    gitignore: Gitignore,
}

impl Service for MyService {
    type Request = Request;
    type Response = Response;
    type Error = Error;
    type Future = Box<Future<Item=Self::Response, Error=Self::Error>>;

    fn call(&self, req: Self::Request) -> Self::Future {
        let res = match self.handle_request(req) {
            Ok(res) => res,
            Err(_) => MyService::internal_server_error(Response::new()),
        };
        Box::new(futures::future::ok(res))
    }
}

impl MyService {
    pub fn new(args: Arc<Args>) -> Self {
        let gitignore = Gitignore::new(args.path.join(".gitignore")).0;
        Self { args, gitignore }
    }

    /// Construct file path from request path.
    ///
    /// 1. Remove leading slash.
    /// 2. URI percent decode.
    fn file_path_from_path(&self, path: &str) -> Result<PathBuf, Utf8Error> {
        percent_decode(path[1..].as_bytes())
            .decode_utf8()
            .map(|path| self.args.path.join(path.into_owned()))
    }

    /// Enable HTTP cache control (current always enable with max-age=0)
    fn enable_cache_control(&self, res: &mut Response) {
        res.headers_mut().set(CacheControl(vec![
            CacheDirective::Public,
            CacheDirective::MaxAge(self.args.cache),
        ]));
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

    /// Determine if payload should be compressed.
    ///
    /// Enable compression when all criteria are met:
    ///
    /// - `compress` arg is true
    /// - is not partial responses 
    /// - is not media contents
    ///
    /// # Parameters
    ///
    /// * `status` - Current status code prepared to respond.
    /// * `mime` - MIME type of the payload.
    fn can_compress(&self, status: StatusCode, mime: &mime::Mime) -> bool {
        self.args.compress &&
            status != StatusCode::PartialContent &&
            !mime.is_media()
    }

    /// Determine critera if given path exists or not.
    ///
    /// A path exists if matches all rules below:
    ///
    /// 1. exists
    /// 2. is not hidden
    /// 3. is not ignored
    fn path_exists<P: AsRef<Path>>(&self, path: P) -> bool {
        let path = path.as_ref();
        path.exists() && 
            !self.path_is_hidden(path) &&
            !self.path_is_ignored(path)
    }

    /// Determine if given path is hidden.
    ///
    /// A path is considered as hidden if matches all rules below:
    ///
    /// 1. `all` arg is false
    /// 2. is hidden (prefixed with dot `.`)
    fn path_is_hidden <P: AsRef<Path>>(&self, path: P) -> bool {
        !self.args.all && path.as_ref().is_hidden()
    }

    /// Determine if given path is ignored.
    ///
    /// A path is considered as ignored if matches all rules below:
    ///
    /// 1. `ignore` arg is true
    /// 2. matches any rules in .gitignore
    fn path_is_ignored <P: AsRef<Path>>(&self, path: P) -> bool {
        let path = path.as_ref();
        self.args.ignore && self
            .gitignore.matched(path, path.is_dir()).is_ignore()
    }

    /// Check if requested resource is under directory of basepath.
    /// 
    /// The given path must be resolved (canonicalized) to eliminate 
    /// incorrect path reported by symlink path.
    fn path_is_under_basepath<P: AsRef<Path>>(&self, path: P) -> bool {
        let path = path.as_ref();
        match path.canonicalize() {
            Ok(path) => path.starts_with(&self.args.path),
            Err(_) => false,
        }
    }

    /// Request handler for `MyService`.
    fn handle_request(&self, req: Request) -> BoxResult<Response> {
        let path = &self.file_path_from_path(req.path())?;

        // Construct response.
        let mut res = Response::new();
        res.headers_mut().set(Server::new(SERVER_VERSION));

        // CORS headers
        self.enable_cors(&mut res);

        // Check critera if the path should be ignore (404 NotFound).
        if !self.path_exists(path) {
            return Ok(MyService::not_found(res))
        }

        // Unless `follow_links` arg is on, any resource laid outside
        // current directory of basepath are forbidden.
        if !self.args.follow_links && !self.path_is_under_basepath(path) {
            return Ok(MyService::forbidden(res))
        }

        // Prepare response body.
        // Being mutable for further modifications.
        let mut body: io::Result<Vec<u8>> = Ok(Vec::new());

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
            self.enable_cache_control(&mut res);

            // Last-Modified-Time from file metadata _mtime_.
            let (mtime, size) = (path.mtime(), path.size());
            let last_modified = LastModified(mtime.into());
            // Concatenate _modified time_ and _file size_ to
            // form a (nearly) strong validator.
            let etag = ETag(EntityTag::strong(
                format!("{}-{}", mtime.timestamp(), size))
            );

            // Validate preconditions of conditional requests.
            if is_precondition_failed(&req, &etag, &last_modified) {
                return Ok(MyService::precondition_failed(res))
            }

            // Validate cache freshness.
            if is_fresh(&req, &etag, &last_modified) {
                return Ok(MyService::not_modified(res)
                  .with_header(last_modified)
                  .with_header(etag)
                )
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

        let mut body = body?;
        let mime_type = MyService::path_mime(path);

        if self.can_compress(res.status(), &mime_type) {
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

        Ok(res.with_body(body))
    }

    fn path_mime<P: AsRef<Path>>(path: P) -> mime::Mime {
        let path = path.as_ref();
        path.mime().unwrap_or_else(|| if path.is_dir() {
            mime::TEXT_HTML_UTF_8
        } else {
            mime::TEXT_PLAIN_UTF_8
        })
    }

    /// Generate 304 NotModified response.
    fn not_modified(res: Response) -> Response {
        res.with_status(StatusCode::NotModified)
    }

    /// Generate 403 Forbidden response.
    fn forbidden(res: Response) -> Response {
        let body = "403 Forbidden";
        res.with_status(StatusCode::Forbidden)
            .with_header(ContentLength(body.len() as u64))
            .with_body(body)
    }

    /// Generate 404 NotFound response.
    fn not_found(res: Response) -> Response {
        let body = "404 Not Found";
        res.with_status(StatusCode::NotFound)
            .with_header(ContentLength(body.len() as u64))
            .with_body(body)
    }

    /// Generate 412 PreconditionFailed response.
    fn precondition_failed(res: Response) -> Response {
        let body = "412 Precondition Failed";
        res.with_status(StatusCode::PreconditionFailed)
            .with_header(ContentLength(body.len() as u64))
            .with_body(body)
    }

    /// Generate 500 InternalServerError response.
    fn internal_server_error(res: Response) -> Response {
        let body = "500 Internal Server Error";
        res.with_status(StatusCode::InternalServerError)
            .with_header(ContentLength(body.len() as u64))
            .with_body(body)
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
            // Get relative path.
            let rel_path = abs_path.strip_prefix(base_path).unwrap();
            // Add "/" prefix to construct absolute hyperlink.
            let path = format!("/{}", rel_path.to_str().unwrap_or_default());
            Item {
                path_type: abs_path.type_(),
                name: rel_path.filename_str().to_owned(),
                path,
            }
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
        vec![Item {
            name: "..".to_owned(),
            path,
            path_type: PathType::Dir,
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
    let f = File::open(file_path)?;
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
    let mut f = File::open(file_path)?;
    let mut buffer = Vec::new();
    f.seek(SeekFrom::Start(start))?;
    BufReader::new(f)
        .take(end - start)
        .read_to_end(&mut buffer)?;
    Ok(buffer)
}
