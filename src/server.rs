// Copyright (c) 2018 Weihang Lo. All rights reserved.
//
// See the LICENSE file at the top-level directory of this distribution.

use std::io::{self, BufReader};
use std::env;
use std::path::Path;
use std::fs::File;
use std::collections::HashMap;

use futures;
use futures::future::Future;
use hyper::Error;
use hyper::header::ContentLength;
use hyper::server::{Http, Request, Response, Service};
use percent_encoding::percent_decode;
use tera::{Tera, Context};

struct Server;

impl Service for Server {
    type Request = Request;
    type Response = Response;
    type Error = Error;
    type Future = Box<Future<Item=Self::Response, Error=Self::Error>>;

    fn call(&self, req: Self::Request) -> Self::Future {
        let result = handle_request(req).unwrap_or_else(|e| {
            Vec::from(format!("Error: {}", e))
        });
        let resp = Response::new()
            .with_header(ContentLength(result.len() as u64))
            .with_body(result);
        Box::new(futures::finished(resp))
    }
}

/// Request handler for `Server`.
fn handle_request(req: Request) -> io::Result<Vec<u8>> {
    // Remove leading slash.
    let req_path = &req.path()[1..].as_bytes();
    // URI percent decode.
    let req_path = percent_decode(req_path)
        .decode_utf8()
        .unwrap()  // TODO: convert error
        .into_owned();
    let req_path = env::current_dir()?.join(req_path); 

    if req_path.is_dir() {
        handle_dir(&req_path)
    } else {
        handle_file(&req_path)
    }
}

/// Send a HTML page of all files under the path.
fn handle_dir(dir_path: &Path) -> io::Result<Vec<u8>> {
    let mut files = Vec::new();
    let base_path = &env::current_dir()?;

    // Prepare dirname of current dir relative to base path.
    let dir_name = { 
        let base_parent = base_path.parent().unwrap_or(base_path);
        let path = dir_path.strip_prefix(base_parent).unwrap();
        format!("{}/", path.to_str().unwrap())
    };

    // Item for popping back to parent directory.
    if base_path != dir_path {
        let parent_path = format!("/{}", dir_path
            .parent().unwrap()
            .strip_prefix(base_path).unwrap()
            .to_str().unwrap()
        ).to_owned();
        let mut map = HashMap::with_capacity(2);
        map.insert("name", "..".to_owned());
        map.insert("path", parent_path);
        files.push(map);
    }

    for entry in dir_path.read_dir()? {
        entry?.path()
            .strip_prefix(base_path) // Strip prefix to build a relative path.
            .and_then(|rel_path| {
                // Construct file name.
                let name = {
                    let mut name = rel_path
                        .file_name().unwrap()
                        .to_str().unwrap()
                        .to_owned();
                    if rel_path.is_dir() {
                        name.push('/');
                    }
                    name
                };
                // Construct hyperlink.
                let path = format!("/{}", rel_path.to_str().unwrap());
                // Use HashMap for default serialization Tera provides.
                let mut map = HashMap::with_capacity(2);
                map.insert("name", name);
                map.insert("path", path);
                files.push(map);
                Ok(())
            }).unwrap_or(()); // Prevent returning Result.
    }

    // Render page with Tera template engine.
    let mut context = Context::new();
    context.add("files", &files);
    context.add("dir_name", &dir_name);
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

/// Run the server.
pub fn serve(host: &str, port: u16) {
    let addr = format!("{}:{}", host, port).parse().unwrap();
    let server = Http::new().bind(&addr, || Ok(Server)).unwrap();
    server.run().unwrap();
}
