// Copyright (c) 2018 Weihang Lo
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::convert::AsRef;
use std::io;
use std::path::{Path, PathBuf};
use std::str::Utf8Error;
use std::sync::Arc;
use std::time::Duration;

use chrono::Local;
use futures::{StreamExt, TryStreamExt};
use headers::{
    AcceptRanges, AccessControlAllowHeaders, AccessControlAllowOrigin, CacheControl, ContentLength,
    ContentType, ETag, HeaderMapExt, LastModified, Range, Server,
};
// Can not use headers::ContentDisposition. Because of https://github.com/hyperium/headers/issues/8
use hyper::header::{HeaderValue, CONTENT_DISPOSITION};
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, StatusCode};
use ignore::gitignore::Gitignore;
use mime_guess::mime;
use percent_encoding::percent_decode;
use qstring::QString;
use serde::Serialize;

use crate::cli::Args;
use crate::extensions::{MimeExt, PathExt, SystemTimeExt};
use crate::http::conditional_requests::{is_fresh, is_precondition_failed};
use crate::http::content_encoding::{compress, encoding_to_static_str, get_prior_encoding};
use crate::http::range_requests::{is_range_fresh, is_satisfiable_range};
use crate::http::{BR, DEFLATE, GZIP};
use crate::server::send::{send_dir, send_dir_as_zip, send_file, send_file_with_range};
use crate::server::{res, Request, Response};
use crate::BoxResult;

const SERVER_VERSION: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));
const CROSS_ORIGIN_EMBEDDER_POLICY: &str = "Cross-Origin-Embedder-Policy";
const CROSS_ORIGIN_OPENER_POLICY: &str = "Cross-Origin-Opener-Policy";

/// Indicate that a path is a normal file/dir or a symlink to another path/dir.
///
/// This enum is serializable in order to rendering with Tera template engine.
/// And the order of enum variants is deremined to ensure sorting precedence.
#[derive(Debug, Serialize, Eq, PartialEq, Ord, PartialOrd)]
pub enum PathType {
    Dir,
    SymlinkDir,
    File,
    SymlinkFile,
}

/// Run the server.
pub async fn serve(args: Args) -> BoxResult<()> {
    let address = args.address()?;
    let path_prefix = args.path_prefix.clone().unwrap_or_default();

    let inner = Arc::new(InnerService::new(args));
    let make_svc = make_service_fn(move |_| {
        let inner = inner.clone();
        async {
            Ok::<_, hyper::Error>(service_fn(move |req| {
                let inner = inner.clone();
                inner.call(req)
            }))
        }
    });
    let server = hyper::Server::bind(&address).serve(make_svc);
    let address = server.local_addr();
    eprintln!("Files served on http://{address}{path_prefix}");

    if let Err(e) = server.await {
        eprintln!("Server error: {e:?}");
    };

    Ok(())
}

/// File and folder actions
enum Action {
    DownloadZip,
    ListDir,
    DownloadFile,
}

struct InnerService {
    args: Args,
    gitignore: Gitignore,
}

impl InnerService {
    pub fn new(args: Args) -> Self {
        let gitignore = Gitignore::new(args.path.join(".gitignore")).0;
        Self { args, gitignore }
    }

    pub async fn call(self: Arc<Self>, req: Request) -> Result<Response, hyper::Error> {
        let res = self
            .handle_request(&req)
            .await
            .unwrap_or_else(|_| res::internal_server_error(Response::default()));
        // Logging
        // TODO: use proper logging crate
        if self.args.log {
            println!(
                r#"[{}] "{} {}" - {}"#,
                Local::now().format("%d/%b/%Y %H:%M:%S"),
                req.method(),
                req.uri(),
                res.status(),
            );
        }
        // Returning response
        Ok(res)
    }

    /// Construct file path from request path.
    ///
    /// 1. Remove leading slash.
    /// 2. Strip path prefix if defined
    /// 3. URI percent decode.
    /// 4. If on windows, switch slashes
    /// 5. Concatenate base path and requested path.
    fn file_path_from_path(&self, path: &str) -> Result<Option<PathBuf>, Utf8Error> {
        let decoded = percent_decode(path[1..].as_bytes()).decode_utf8()?;
        let slashes_switched = if cfg!(windows) {
            decoded.replace("/", "\\")
        } else {
            decoded.into_owned()
        };
        let stripped_path = match self.strip_path_prefix(&slashes_switched) {
            Some(path) => path,
            None => return Ok(None),
        };
        let mut path = self.args.path.join(stripped_path);
        if self.args.render_index && path.is_dir() {
            path.push("index.html")
        }

        Ok(Some(path))
    }

    /// Enable HTTP cache control (current always enable with max-age=0)
    fn enable_cache_control(&self, res: &mut Response) {
        let header = CacheControl::new()
            .with_public()
            .with_max_age(Duration::from_secs(self.args.cache));
        res.headers_mut().typed_insert(header);
    }

    /// Enable cross-origin resource sharing for given response.
    fn enable_cors(&self, res: &mut Response) {
        if self.args.cors {
            res.headers_mut()
                .typed_insert(AccessControlAllowOrigin::ANY);
            res.headers_mut().typed_insert(
                vec![
                    hyper::header::RANGE,
                    hyper::header::CONTENT_TYPE,
                    hyper::header::ACCEPT,
                    hyper::header::ORIGIN,
                ]
                .into_iter()
                .collect::<AccessControlAllowHeaders>(),
            );
        }
    }

    /// Enable cross-origin isolation for given response.
    fn enable_coi(&self, res: &mut Response) {
        if self.args.coi {
            res.headers_mut().insert(
                CROSS_ORIGIN_EMBEDDER_POLICY,
                HeaderValue::from_str("require-corp").unwrap(),
            );
            res.headers_mut().insert(
                CROSS_ORIGIN_OPENER_POLICY,
                HeaderValue::from_str("same-origin").unwrap(),
            );
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
        self.args.compress && status != StatusCode::PARTIAL_CONTENT && !mime.is_compressed_format()
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
        path.exists() && !self.path_is_hidden(path) && !self.path_is_ignored(path)
    }

    /// Determine if given path is hidden.
    ///
    /// A path is considered as hidden if matches all rules below:
    ///
    /// 1. `all` arg is false
    /// 2. any component of the path is hidden (prefixed with dot `.`)
    fn path_is_hidden<P: AsRef<Path>>(&self, path: P) -> bool {
        !self.args.all && path.as_ref().is_relatively_hidden()
    }

    /// Determine if given path is ignored.
    ///
    /// A path is considered as ignored if matches all rules below:
    ///
    /// 1. `ignore` arg is true
    /// 2. matches any rules in .gitignore
    fn path_is_ignored<P: AsRef<Path>>(&self, path: P) -> bool {
        let path = path.as_ref();
        self.args.ignore && self.gitignore.matched(path, path.is_dir()).is_ignore()
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

    /// Strip the path prefix of the request path.
    ///
    /// If there is a path prefix defined and `strip_prefix` returns `None`,
    /// return None. Otherwise return the path with the prefix stripped.
    fn strip_path_prefix<'a, P: AsRef<Path>>(&self, path: &'a P) -> Option<&'a Path> {
        let path = path.as_ref();
        match self.args.path_prefix.as_deref() {
            Some(prefix) => {
                let prefix = prefix.trim_start_matches('/');
                path.strip_prefix(prefix).ok()
            }
            None => Some(path),
        }
    }

    /// Request handler for `MyService`.
    async fn handle_request(&self, req: &Request) -> BoxResult<Response> {
        // Construct response.
        let mut res = Response::default();
        res.headers_mut()
            .typed_insert(Server::from_static(SERVER_VERSION));

        let path = match self.file_path_from_path(req.uri().path())? {
            Some(path) => path,
            None => return Ok(res::not_found(res)),
        };

        let default_action = if path.is_dir() {
            Action::ListDir
        } else {
            Action::DownloadFile
        };

        let action = match req.uri().query() {
            Some(query) => {
                let query = QString::from(query);

                match query.get("action") {
                    Some(action_str) => match action_str {
                        "zip" => {
                            if path.is_dir() {
                                Action::DownloadZip
                            } else {
                                bail!("error: invalid action");
                            }
                        }
                        _ => bail!("error: invalid action"),
                    },
                    None => default_action,
                }
            }
            None => default_action,
        };

        // CORS headers
        self.enable_cors(&mut res);

        // COOP and COEP headers
        self.enable_coi(&mut res);

        // Check critera if the path should be ignore (404 NotFound).
        if !self.path_exists(&path) {
            return Ok(res::not_found(res));
        }

        // Unless `follow_links` arg is on, any resource laid outside
        // current directory of basepath are forbidden.
        if !self.args.follow_links && !self.path_is_under_basepath(&path) {
            return Ok(res::forbidden(res));
        }

        // Prepare response body.
        // Being mutable for further modifications.
        let mut body = Body::empty();
        let mut content_length: Option<u64> = None;

        // Extra process for serving files.
        match action {
            Action::ListDir => {
                let (content, size) = send_dir(
                    &path,
                    &self.args.path,
                    self.args.all,
                    self.args.ignore,
                    self.args.path_prefix.as_deref(),
                )?;
                body = Body::from(content);
                content_length = Some(size as u64);
            }
            Action::DownloadFile => {
                // Cache-Control.
                self.enable_cache_control(&mut res);

                // Last-Modified-Time from file metadata _mtime_.
                let (mtime, size) = (path.mtime(), path.size());
                let last_modified = LastModified::from(mtime);
                // Concatenate _modified time_ and _file size_ to
                // form a (nearly) strong validator.
                let etag = format!(r#""{}-{}""#, mtime.timestamp(), size)
                    .parse::<ETag>()
                    .unwrap();

                // Validate preconditions of conditional requests.
                if is_precondition_failed(req, &etag, mtime) {
                    return Ok(res::precondition_failed(res));
                }

                // Validate cache freshness.
                if is_fresh(req, &etag, mtime) {
                    res.headers_mut().typed_insert(last_modified);
                    res.headers_mut().typed_insert(etag);
                    return Ok(res::not_modified(res));
                }

                // Range Request support.
                if let Some(range) = req.headers().typed_get::<Range>() {
                    #[allow(clippy::single_match)]
                    match (
                        is_range_fresh(req, &etag, &last_modified),
                        is_satisfiable_range(&range, size as u64),
                    ) {
                        (true, Some(content_range)) => {
                            // 206 Partial Content.
                            if let Some(range) = content_range.bytes_range() {
                                let stream = send_file_with_range(&path, range)?;
                                body = Body::wrap_stream(stream);
                            }
                            res.headers_mut().typed_insert(content_range);
                            *res.status_mut() = StatusCode::PARTIAL_CONTENT;
                        }
                        // Respond entire entity if Range header contains
                        // unsatisfiable range.
                        _ => (),
                    }
                }

                if res.status() != StatusCode::PARTIAL_CONTENT {
                    let (stream, size) = send_file(&path)?;
                    body = Body::wrap_stream(stream);
                    content_length = Some(size);
                }
                res.headers_mut().typed_insert(last_modified);
                res.headers_mut().typed_insert(etag);
            }
            Action::DownloadZip => {
                let (stream, size) = send_dir_as_zip(&path, self.args.all, self.args.ignore)?;
                body = Body::wrap_stream(stream);
                content_length = Some(size);

                // Changing the filename
                res.headers_mut().insert(
                    CONTENT_DISPOSITION,
                    HeaderValue::from_str(&format!(
                        "attachment; filename=\"{}.zip\"",
                        path.file_name().unwrap().to_str().unwrap()
                    ))
                    .unwrap(),
                );
            }
        }

        let mime_type = InnerService::guess_path_mime(&path, action);

        let body = if self.can_compress(res.status(), &mime_type) {
            let encoding = req
                .headers()
                .get(hyper::header::ACCEPT_ENCODING)
                .map(get_prior_encoding)
                .unwrap_or_default();
            let enc = encoding_to_static_str(encoding);
            match enc {
                BR | DEFLATE | GZIP => {
                    let body = Body::wrap_stream(
                        body.map_err(|_e| io::Error::from(io::ErrorKind::InvalidData))
                            .map(move |b| match b {
                                Ok(b) => compress(&b, enc),
                                Err(e) => Err(e),
                            }),
                    );
                    res.headers_mut().insert(
                        hyper::header::CONTENT_ENCODING,
                        hyper::header::HeaderValue::from_static(encoding),
                    );
                    // Representation varies, so responds with a `Vary` header.
                    res.headers_mut().insert(
                        hyper::header::VARY,
                        hyper::header::HeaderValue::from_name(hyper::header::ACCEPT_ENCODING),
                    );
                    body
                }
                // No Accept-Encoding would not be compress.
                _ => body,
            }
        } else {
            // Set Content-Length only when body is not compressed,
            // otherwise the client will get confused
            // e.g. curl: (18) transfer closed with N bytes remaining to read
            if let Some(content_length) = content_length {
                res.headers_mut()
                    .typed_insert(ContentLength(content_length));
            }
            body
        };

        // Common headers
        res.headers_mut().typed_insert(AcceptRanges::bytes());
        res.headers_mut().typed_insert(ContentType::from(mime_type));

        *res.body_mut() = body;
        Ok(res)
    }

    fn guess_path_mime<P: AsRef<Path>>(path: P, action: Action) -> mime::Mime {
        let path = path.as_ref();
        path.mime()
            .map(|x| match x.get_param(mime::CHARSET) {
                Some(_) => x,
                None => x
                    .guess_charset()
                    .and_then(|c| format!("{}; charset={}", x, c).parse().ok())
                    .unwrap_or(x),
            })
            .unwrap_or_else(|| match action {
                Action::ListDir => mime::TEXT_HTML_UTF_8,
                Action::DownloadFile => mime::TEXT_PLAIN_UTF_8,
                Action::DownloadZip => mime::APPLICATION_OCTET_STREAM,
            })
    }
}

#[cfg(test)]
mod t_server {
    use super::*;
    use crate::test_utils::{get_tests_dir, with_current_dir};
    use std::fs::File;
    use tempfile::Builder;

    fn bootstrap(args: Args) -> (InnerService, Response) {
        (InnerService::new(args), Response::default())
    }

    const fn temp_name() -> &'static str {
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
            Some(PathBuf::from("/storage/你好世界"))
        );

        // Return index.html if `--render-index` flag is on.
        let dir = Builder::new().prefix(temp_name()).tempdir().unwrap();
        let args = Args {
            path: dir.path().to_owned(),
            ..Default::default()
        };
        let (service, _) = bootstrap(args);
        assert_eq!(
            service.file_path_from_path(".").unwrap(),
            Some(dir.path().join("index.html")),
        );
    }

    #[test]
    fn guess_path_mime() {
        let mime_type =
            InnerService::guess_path_mime("file-wthout-extension", Action::DownloadFile);
        assert_eq!(mime_type, mime::TEXT_PLAIN_UTF_8);

        let mime_type = InnerService::guess_path_mime("file.json", Action::DownloadFile);
        let json_utf8 = "application/json; charset=utf-8"
            .parse::<mime::Mime>()
            .unwrap();
        assert_eq!(mime_type, json_utf8);
        assert_eq!(mime_type.get_param(mime::CHARSET), Some(mime::UTF_8));

        let mime_type = InnerService::guess_path_mime("lib.wasm", Action::DownloadFile);
        let wasm = "application/wasm".parse::<mime::Mime>().unwrap();
        assert_eq!(mime_type, wasm);
        assert_eq!(mime_type.get_param(mime::CHARSET), None);

        let dir_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let mime_type = InnerService::guess_path_mime(dir_path, Action::ListDir);
        assert_eq!(mime_type, mime::TEXT_HTML_UTF_8);

        let dir_path = PathBuf::from("./tests");
        let mime_type = InnerService::guess_path_mime(dir_path, Action::DownloadZip);
        assert_eq!(mime_type, mime::APPLICATION_OCTET_STREAM);
    }

    #[test]
    fn enable_cors() {
        let args = Args::default();
        let (service, mut res) = bootstrap(args);
        service.enable_cors(&mut res);
        assert_eq!(
            res.headers()
                .typed_get::<AccessControlAllowOrigin>()
                .unwrap(),
            AccessControlAllowOrigin::ANY,
        );
        assert_eq!(
            res.headers()
                .typed_get::<AccessControlAllowHeaders>()
                .unwrap(),
            vec![
                hyper::header::RANGE,
                hyper::header::CONTENT_TYPE,
                hyper::header::ACCEPT,
                hyper::header::ORIGIN,
            ]
            .into_iter()
            .collect::<AccessControlAllowHeaders>(),
        );
    }

    #[test]
    fn enable_coi() {
        let args = Args::default();
        let (service, mut res) = bootstrap(args);
        service.enable_coi(&mut res);
        assert_eq!(
            res.headers().get(CROSS_ORIGIN_OPENER_POLICY).unwrap(),
            "same-origin",
        );
        assert_eq!(
            res.headers().get(CROSS_ORIGIN_EMBEDDER_POLICY).unwrap(),
            "require-corp",
        );
    }

    #[test]
    fn disable_cors() {
        let args = Args {
            cors: false,
            ..Default::default()
        };
        let (service, mut res) = bootstrap(args);
        service.enable_cors(&mut res);
        assert!(res
            .headers()
            .typed_get::<AccessControlAllowOrigin>()
            .is_none());
    }

    #[test]
    fn enable_cache_control() {
        let args = Args::default();
        let (service, mut res) = bootstrap(args);
        service.enable_cache_control(&mut res);
        assert_eq!(
            res.headers().typed_get::<CacheControl>().unwrap(),
            CacheControl::new()
                .with_public()
                .with_max_age(Duration::default()),
        );

        let cache = 3600;
        let args = Args {
            cache,
            ..Default::default()
        };
        let (service, mut res) = bootstrap(args);
        service.enable_cache_control(&mut res);
        assert_eq!(
            res.headers().typed_get::<CacheControl>().unwrap(),
            CacheControl::new()
                .with_public()
                .with_max_age(Duration::from_secs(3600)),
        );
    }

    #[test]
    fn can_compress() {
        let args = Args::default();
        let (service, _) = bootstrap(args);
        assert!(service.can_compress(StatusCode::OK, &mime::TEXT_PLAIN));
    }

    #[test]
    fn cannot_compress() {
        let args = Args {
            compress: false,
            ..Default::default()
        };
        let (service, _) = bootstrap(args);
        assert!(!service.can_compress(StatusCode::OK, &mime::STAR_STAR));
        assert!(!service.can_compress(StatusCode::OK, &mime::TEXT_PLAIN));
        assert!(!service.can_compress(StatusCode::OK, &mime::IMAGE_JPEG));

        let args = Args::default();
        let (service, _) = bootstrap(args);
        assert!(!service.can_compress(StatusCode::PARTIAL_CONTENT, &mime::STAR_STAR));
        assert!(!service.can_compress(StatusCode::PARTIAL_CONTENT, &mime::TEXT_PLAIN));
        assert!(!service.can_compress(StatusCode::PARTIAL_CONTENT, &mime::IMAGE_JPEG));
        assert!(!service.can_compress(StatusCode::OK, &"video/*".parse::<mime::Mime>().unwrap()));
        assert!(!service.can_compress(StatusCode::OK, &"audio/*".parse::<mime::Mime>().unwrap()));
    }

    #[test]
    fn path_exists() {
        with_current_dir(get_tests_dir(), || {
            let args = Args::default();
            let (service, _) = bootstrap(args);
            // Exists but not hidden nor ignored
            assert!(service.path_exists("file.txt"));
        });
    }

    #[test]
    fn path_does_not_exists() {
        with_current_dir(get_tests_dir(), || {
            let args = Args {
                all: false,
                ..Default::default()
            };
            let (service, _) = bootstrap(args);

            // Not exists
            let path = "NOT_EXISTS_README.md";
            assert!(!PathBuf::from(path).exists());
            assert!(!service.path_exists(path));

            // Exists but hidden
            let path = ".hidden.html";
            assert!(PathBuf::from(path).exists());
            assert!(service.path_is_hidden(path));
            assert!(!service.path_exists(path));

            // Exists but the file's parent is hidden
            let path = ".hidden/nested.html";
            assert!(PathBuf::from(path).exists());
            assert!(service.path_is_hidden(path));
            assert!(!service.path_exists(path));

            // Exists and not hidden but ignored
            let path = "ignore_pattern";
            assert!(PathBuf::from(path).exists());
            assert!(!service.path_is_hidden(path));
            assert!(!service.path_exists(path));
        });
    }

    #[test]
    fn path_is_hidden() {
        // A file prefixed with `.` is considered as hidden.
        let args = Args {
            all: false,
            ..Default::default()
        };
        let (service, _) = bootstrap(args);
        assert!(service.path_is_hidden(".a-hidden-file"));
    }

    #[test]
    fn path_is_not_hidden() {
        // `--all` flag is on
        let args = Args::default();
        let (service, _) = bootstrap(args);
        assert!(!service.path_is_hidden(".a-hidden-file"));
        assert!(!service.path_is_hidden("a-public-file"));

        // `--all` flag is off and the file is not prefixed with `.`
        let args = Args {
            all: false,
            ..Default::default()
        };
        let (service, _) = bootstrap(args);
        assert!(!service.path_is_hidden("a-public-file"));
    }

    #[test]
    fn path_is_ignored() {
        with_current_dir(get_tests_dir(), || {
            let args = Args::default();
            let (service, _) = bootstrap(args);
            assert!(service.path_is_ignored("ignore_pattern"));
            assert!(service.path_is_ignored("dir/ignore_pattern"));
        });
    }

    #[test]
    fn path_is_not_ignored() {
        with_current_dir(get_tests_dir(), || {
            // `--no-ignore` flag is on
            let args = Args {
                ignore: false,
                ..Default::default()
            };
            let (service, _) = bootstrap(args);
            assert!(!service.path_is_ignored("ignore_pattern"));
            assert!(!service.path_is_ignored("dir/ignore_pattern"));

            // file.txt and .hidden.html is not ignored.
            let args = Args::default();
            let (service, _) = bootstrap(args);
            assert!(!service.path_is_ignored("file.txt"));
            assert!(!service.path_is_ignored(".hidden.html"));
        });
    }

    #[test]
    fn path_is_under_basepath() {
        #[cfg(unix)]
        use std::os::unix::fs::symlink as symlink_file;
        #[cfg(windows)]
        use std::os::windows::fs::symlink_file;

        let src_dir = Builder::new().prefix(temp_name()).tempdir().unwrap();
        let src_dir = src_dir.path().canonicalize().unwrap();
        let src_path = src_dir.join("src_file.txt");
        let _ = File::create(&src_path);

        // Is under service's base path
        let symlink_path = src_dir.join("symlink");
        let args = Args {
            path: src_dir,
            ..Default::default()
        };
        let (service, _) = bootstrap(args);
        symlink_file(&src_path, &symlink_path).unwrap();
        assert!(service.path_is_under_basepath(&symlink_path));

        // Not under base path.
        let args = Args::default();
        let (service, _) = bootstrap(args);
        assert!(!service.path_is_under_basepath(&symlink_path));
    }

    #[test]
    fn strips_path_prefix() {
        let args = Args {
            path_prefix: Some("/foo".into()),
            ..Default::default()
        };
        let (service, _) = bootstrap(args);

        assert_eq!(
            service.strip_path_prefix(&Path::new("foo/dir/to/bar.txt")),
            Some(Path::new("dir/to/bar.txt"))
        );

        assert_eq!(
            service.strip_path_prefix(&Path::new("dir/to/bar.txt")),
            None
        );

        let args = Args::default();
        let (service, _) = bootstrap(args);

        assert_eq!(
            service.strip_path_prefix(&Path::new("foo/dir/to/bar.txt")),
            Some(Path::new("foo/dir/to/bar.txt"))
        );
    }

    #[ignore]
    #[test]
    fn handle_request() {}
}
