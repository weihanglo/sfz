// Copyright (c) 2018 Weihang Lo
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

mod send;
mod res;

use std::io;
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
use ignore::gitignore::Gitignore;
use chrono::Local;

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
use self::send::{send_dir, send_file, send_file_with_range};

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
        let res = match self.handle_request(&req) {
            Ok(res) => res,
            Err(_) => res::internal_server_error(Response::new()),
        };
        // Logging
        if self.args.log {
            println!(r#"[{}] "{} {}" - {}"#,
                Local::now().format("%d/%b/%Y %H:%M:%S"),
                req.method(), 
                req.uri(),
                res.status(),
            );
        }
        // Returning response
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
    /// 3. Concatenate base path and requested path.
    fn file_path_from_path(&self, path: &str) -> Result<PathBuf, Utf8Error> {
        percent_decode(path[1..].as_bytes())
            .decode_utf8()
            .map(|path| self.args.path.join(path.into_owned()))
            .map(|path| if self.args.render_index && path.is_dir() {
                path.join("index.html")
            } else {
                path
            })
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
            !mime.is_compressed_format()
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
    fn handle_request(&self, req: &Request) -> BoxResult<Response> {
        let path = &self.file_path_from_path(req.path())?;

        // Construct response.
        let mut res = Response::new();
        res.headers_mut().set(Server::new(SERVER_VERSION));

        // CORS headers
        self.enable_cors(&mut res);

        // Check critera if the path should be ignore (404 NotFound).
        if !self.path_exists(path) {
            return Ok(res::not_found(res))
        }

        // Unless `follow_links` arg is on, any resource laid outside
        // current directory of basepath are forbidden.
        if !self.args.follow_links && !self.path_is_under_basepath(path) {
            return Ok(res::forbidden(res))
        }

        // Prepare response body.
        // Being mutable for further modifications.
        let mut body: io::Result<Vec<u8>> = Ok(Vec::new());

        // Extra process for serving files.
        if path.is_dir() {
            body = send_dir(
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
                return Ok(res::precondition_failed(res))
            }

            // Validate cache freshness.
            if is_fresh(&req, &etag, &last_modified) {
                return Ok(res::not_modified(res)
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
                            body = send_file_with_range(path, range);
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
                body = send_file(path);
            }
            res.headers_mut().set(last_modified);
            res.headers_mut().set(etag);
        }

        let mut body = body?;
        let mime_type = MyService::guess_path_mime(path);

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

    fn guess_path_mime<P: AsRef<Path>>(path: P) -> mime::Mime {
        let path = path.as_ref();
        path.mime().unwrap_or_else(|| if path.is_dir() {
            mime::TEXT_HTML_UTF_8
        } else {
            mime::TEXT_PLAIN_UTF_8
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use tempdir::TempDir;

    impl Default for Args {
        fn default() -> Self {
            Self {
                address: "127.0.0.1".to_owned(),
                port: 5000,
                cache: 0,
                cors: true,
                compress: true,
                path: PathBuf::from("."),
                all: true,
                ignore: true,
                follow_links: true,
                render_index: true,
                log: true,
            }
        }
    }

    fn bootstrap(args: Args) -> (MyService, Response) {
        (MyService::new(Arc::new(args)), Response::new())
    }

    fn temp_name() -> &'static str {
        concat!(env!("CARGO_PKG_NAME"), "-", env!("CARGO_PKG_VERSION"))
    }

    #[test]
    fn file_path_from_path() {
        let args = Args { 
            render_index: false,
            path: Path::new("/storage").to_owned(), 
            ..Default::default() 
        };
        let (service, _) = bootstrap(args);
        let path = "/%E4%BD%A0%E5%A5%BD%E4%B8%96%E7%95%8C";
        assert_eq!(
            service.file_path_from_path(path).unwrap(),
            Path::new("/storage/你好世界")
        );


        // Return index.html if `--render-index` flag is on.
        let dir = TempDir::new(temp_name()).unwrap();
        let args = Args { 
            path: dir.path().to_owned(),
            ..Default::default() 
        };
        let (service, _) = bootstrap(args);
        assert_eq!(
            service.file_path_from_path(".").unwrap(),
            dir.path().join("index.html"),
        );
    }

    #[test]
    fn guess_path_mime() {
        use std::env;
        let mime_type = MyService::guess_path_mime("file-wthout-extension");
        assert_eq!(mime_type, mime::TEXT_PLAIN_UTF_8);

        let mime_type = MyService::guess_path_mime(env::home_dir().unwrap());
        assert_eq!(mime_type, mime::TEXT_HTML_UTF_8);
    }

    #[test]
    fn enable_cors() {
        let args = Args { ..Default::default() };
        let (service, mut res) = bootstrap(args);
        service.enable_cors(&mut res);
        assert_eq!(
            *res.headers().get::<AccessControlAllowOrigin>().unwrap(),
            AccessControlAllowOrigin::Any,
        );
    }

    #[test]
    fn disable_cors() {
        let args = Args { cors: false, ..Default::default() };
        let (service, mut res) = bootstrap(args);
        service.enable_cors(&mut res);
        assert!(!res.headers().has::<AccessControlAllowOrigin>());
    }

    #[test]
    fn enable_cache_control() {
        let args = Args { ..Default::default() };
        let (service, mut res) = bootstrap(args);
        service.enable_cache_control(&mut res);
        assert!(res.headers().has::<CacheControl>());
    }

    #[test]
    fn can_compress() {
        let args = Args { ..Default::default() };
        let (service, _) = bootstrap(args);
        assert!(service.can_compress(StatusCode::Ok, &mime::TEXT_PLAIN));
    }

    #[test]
    fn cannot_compress() {
        let args = Args { compress: false, ..Default::default() };
        let (service, _) = bootstrap(args);
        assert!(!service.can_compress(StatusCode::Ok, &mime::TEXT_PLAIN));

        let args = Args { ..Default::default() };
        let (service, _) = bootstrap(args);
        assert!(!service.can_compress(
                StatusCode::PartialContent,
                &mime::STAR_STAR,
        ));
        assert!(!service.can_compress(
            StatusCode::Ok,
            &"video/*".parse::<mime::Mime>().unwrap(),
        ));
        assert!(!service.can_compress(
            StatusCode::Ok,
            &"audio/*".parse::<mime::Mime>().unwrap(),
        ));
    }

    #[ignore]
    #[test]
    fn path_exists() {
    }

    #[test]
    fn path_is_hidden() {
        // A file prefixed with `.` is considered as hidden.
        let args = Args { all: false, ..Default::default() };
        let (service, _) = bootstrap(args);
        assert!(service.path_is_hidden(".a-hidden-file"));
    }

    #[test]
    fn path_is_not_hidden() {
        // `--all` flag is on
        let args = Args { ..Default::default() };
        let (service, _) = bootstrap(args);
        assert!(!service.path_is_hidden(".a-hidden-file"));

        // `--all` flag is off and the file is not prefixed with `.`
        let args = Args { all: false, ..Default::default() };
        let (service, _) = bootstrap(args);
        assert!(!service.path_is_hidden("a-public-file"));
    }

    #[test]
    fn path_is_ignored() {
        let args = Args { ..Default::default() };
        let (service, _) = bootstrap(args);
        assert!(!service.path_is_ignored("target"));
    }

    #[test]
    fn path_is_not_ignored() {
        // `--no-ignore` flag is on
        let args = Args { ignore: false, ..Default::default() };
        let (service, _) = bootstrap(args);
        assert!(!service.path_is_ignored("target"));

        // README.md is not ignored.
        let args = Args { ..Default::default() };
        let (service, _) = bootstrap(args);
        assert!(!service.path_is_ignored("README.md"));
    }

    #[cfg(unix)]
    #[test]
    fn path_is_under_basepath() {
        use std::os::unix::fs::symlink;

        let src_dir = TempDir::new(temp_name()).unwrap();
        let src_dir = src_dir.path().canonicalize().unwrap();
        let src_path = src_dir.join("src_file.txt");
        let _ = File::create(&src_path);

        // Is under service's base path
        let symlink_path = src_dir.join("symlink");
        let args = Args { path: src_dir, ..Default::default() };
        let (service, _) = bootstrap(args);
        symlink(&src_path, &symlink_path).unwrap();
        assert!(service.path_is_under_basepath(&symlink_path));

        // Not under base path.
        let args = Args { ..Default::default() };
        let (service, _) = bootstrap(args);
        assert!(!service.path_is_under_basepath(&symlink_path));
    }

    #[cfg(windows)]
    #[test]
    fn path_is_under_basepath() {
        use std::os::windows::fs::symlink_file;

        let src_dir = TempDir::new(temp_name()).unwrap();
        let src_dir = src_dir.path().canonicalize().unwrap();
        let src_path = src_dir.join("src_file.txt");
        let _ = File::create(&src_path);

        // Is under service's base path
        let symlink_path = src_dir.join("symlink");
        let args = Args { path: src_dir, ..Default::default() };
        let (service, _) = bootstrap(args);
        symlink_file(&src_path, &symlink_path).unwrap();
        assert!(service.path_is_under_basepath(&symlink_path));

        // Not under base path.
        let args = Args { ..Default::default() };
        let (service, _) = bootstrap(args);
        assert!(!service.path_is_under_basepath(&symlink_path));
    }

    #[ignore]
    #[test]
    fn handle_request() {
    }
}
