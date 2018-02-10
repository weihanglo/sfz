// Copyright (c) 2018 Weihang Lo. All rights reserved.
//
// See the LICENSE file at the top-level directory of this distribution.

use std::io::{self, BufReader};
use std::env;
use std::path::Path;
use std::fs::File;
use std::collections::HashMap;
use std::net::SocketAddr;

use futures;
use futures::future::Future;
use hyper::server::{Http, Request, Response, Service};
use hyper::{mime, Error};
use hyper::header::{
    AccessControlAllowHeaders,
    AccessControlAllowOrigin,
    ContentLength,
    ContentType,
    Headers,
    Server,
};
use unicase::Ascii;
use percent_encoding::percent_decode;
use tera::{Tera, Context};
use mime_guess::get_mime_type_opt;

const SERVER_VERSION: &str =
    concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));

#[derive(Debug, Clone)]
pub struct ServerOptions {
    pub cors: bool,
}

impl Default for ServerOptions {
    fn default() -> Self {
        Self {
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
        let resp = self.handle_request(&req);
        Box::new(futures::finished(resp))
    }
}

impl MyService {
    pub fn new(options: ServerOptions) -> Self {
        Self { options }
    }

    /// Request handler for `MyService`.
    fn handle_request(&self, req: &Request) -> Response {
        // Remove leading slash.
        let req_path = &req.path()[1..].as_bytes();
        // URI percent decode.
        let req_path = percent_decode(req_path)
            .decode_utf8()
            .unwrap()
            .into_owned();
        let req_path = env::current_dir().unwrap().join(req_path);

        let error_handler = |e: io::Error| Vec::from(format!("Error: {}", e));
        let body = if req_path.is_dir() {
            handle_dir(&req_path).unwrap_or_else(error_handler)
        } else {
            handle_file(&req_path).unwrap_or_else(error_handler)
        };

        // MIME type guessing.
        let mime_type = if req_path.is_dir() {
            mime::TEXT_HTML_UTF_8
        } else {
            match req_path.extension() {
                Some(ext) => {
                    get_mime_type_opt(ext.to_str().unwrap_or(""))
                        .unwrap_or(mime::TEXT_PLAIN)
                }
                None => mime::TEXT_PLAIN,
            }
        };

        let mut headers = Headers::new();
        // Default headers
        headers.set(ContentType(mime_type));
        headers.set(ContentLength(body.len() as u64));
        headers.set(Server::new(SERVER_VERSION));
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

        Response::new()
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
    let f = File::open(file_path)?;
    let mut buffer = Vec::new();
    BufReader::new(f).read_to_end(&mut buffer)?;
    Ok(buffer)
}
