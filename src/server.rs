// Copyright (c) 2018 Weihang Lo. All rights reserved.
//
// See the LICENSE file at the top-level directory of this distribution.

use std::io::{self, BufReader};
use std::env;
use std::path::Path;
use std::fs;
use std::collections::HashMap;
use std::net::SocketAddr;

use futures;
use futures::future::Future;
use hyper::server::{Http, Request, Response, Service};
use hyper::{StatusCode, mime, Error};
use hyper::header::{
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
    Server,
};
use unicase::Ascii;
use percent_encoding::percent_decode;
use tera::{Tera, Context};
use mime_guess::get_mime_type_opt;
use md5;

use ::conditional_requests::{is_precondition_failed, is_fresh};

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
        Box::new(futures::finished(resp))
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
        let response = Response::new();
        let mut headers = Headers::new();
        headers.set(Server::new(SERVER_VERSION));

        // Prepare response body.
        // Being mutable for further modification.
        let body = if req_path.is_dir() {
            handle_dir(&req_path)
        } else {
            handle_file(&req_path)
        }.unwrap_or_else(|e| Vec::from(format!("Error: {}", e)));

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
        if !req_path.is_dir() {
            // Cache-Control.
            headers.set(CacheControl(vec![
                CacheDirective::Public,
                CacheDirective::MaxAge(self.options.cache),
            ]));
            // Last-Modified-Time from file metadata `mtime`.
            let mtime = fs::metadata(&req_path)
                .and_then(|meta| meta.modified())
                .unwrap();
            // ETag from md5 hashing.
            let last_modified = LastModified(mtime.into());
            let etag = ETag(EntityTag::weak(
                format!("{:x}", md5::compute(&body)))
            );

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
        }

        // Common headers
        headers.set(ContentType(guess_mime_type(&req_path)));
        if body.len() > 0 {
            headers.set(ContentLength(body.len() as u64));
        }

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
    let (dir_name, dir_path_vec) = {
        let base_parent = base_path.parent().unwrap_or(base_path);
        let path = dir_path.strip_prefix(base_parent).unwrap();
        let dir_path_vec = path.iter()
            .map(|s| s.to_str().unwrap())
            .collect::<Vec<_>>();
        let dir_name = format!("{}/", path.to_str().unwrap());
        (dir_name, dir_path_vec)
    };

    // Item for popping back to parent directory.
    if base_path != dir_path {
        let parent_path = format!("/{}", dir_path
            .parent().unwrap()
            .strip_prefix(base_path).unwrap()
            .to_str().unwrap()
        ).to_owned();
        let mut map = HashMap::with_capacity(3);
        map.insert("name", "..".to_owned());
        map.insert("path", parent_path);
        map.insert("is_dir", "1".to_owned());
        files.push(map);
    }

    for entry in dir_path.read_dir()? {
        entry?.path()
            .strip_prefix(base_path) // Strip prefix to build a relative path.
            .and_then(|rel_path| {
                // Use HashMap for default serialization Tera provides.
                let mut map = HashMap::with_capacity(3);

                // Construct file name.
                let name = rel_path
                    .file_name().unwrap()
                    .to_str().unwrap()
                    .to_owned();
                map.insert("name", name);

                // Construct hyperlink.
                let path = format!("/{}", rel_path.to_str().unwrap());
                map.insert("path", path);

                // Indicate that the file is a directory.
                if rel_path.is_dir() {
                    map.insert("is_dir", "1".to_owned());
                }
                files.push(map);
                Ok(())
            }).unwrap_or(()); // Prevent returning Result.
    }

    // Render page with Tera template engine.
    let mut context = Context::new();
    context.add("files", &files);
    context.add("dir_name", &dir_name);
    context.add("dir_path_vec", &dir_path_vec);
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
