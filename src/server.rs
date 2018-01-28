// Copyright (c) 2018 Weihang Lo. All rights reserved.
//
// See the LICENSE file at the top-level directory of this distribution.

use std::io::{self, BufReader};
use std::env;
use std::fs::File;

use futures;
use futures::future::Future;
use hyper::Error;
use hyper::header::ContentLength;
use hyper::server::{Http, Request, Response, Service};
use percent_encoding::percent_decode;

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

fn handle_request(req: Request) -> io::Result<Vec<u8>> {
    // Remove leading slash.
    let path = &req.path()[1..].as_bytes();
    // URI percent decode.
    let path = percent_decode(path)
        .decode_utf8()
        .unwrap()  // TODO: convert error
        .into_owned();
    let path = env::current_dir()?.join(path); 

    if path.is_dir() {
        let mut paths = Vec::new();
        for entry in path.read_dir()? {
            let path = entry?.path();
            paths.push(path);
        }
        Ok(Vec::from(format!("{:?}", paths))) // TODO: use standard OsStr?
    } else {
        use std::io::prelude::*;
        let f = File::open(path)?;
        let mut buffer = Vec::new();
        BufReader::new(f).read_to_end(&mut buffer)?;
        Ok(buffer)
    }
}


pub fn serve(host: &str, port: u16) {
    let addr = format!("{}:{}", host, port).parse().unwrap();
    let server = Http::new().bind(&addr, || Ok(Server)).unwrap();
    server.run().unwrap();
}
