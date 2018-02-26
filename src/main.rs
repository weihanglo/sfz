// Copyright (c) 2018 Weihang Lo. All rights reserved.
//
// See the LICENSE file at the top-level directory of this distribution.

#[macro_use]
extern crate clap;
extern crate futures;
extern crate hyper;
extern crate percent_encoding;
extern crate tera;
extern crate mime_guess;
extern crate unicase;
extern crate serde;
#[macro_use]
extern crate serde_derive;

mod server;
mod conditional_requests;
mod range_requests;

use clap::Arg;
use server::{serve, ServerOptions};
use std::env;
use std::path::PathBuf;

fn main() {
    let arg_port = Arg::with_name("port")
        .short("p")
        .long("port")
        .default_value("8888")
        .help("Specify port to listen on")
        .value_name("PORT");

    let arg_address = Arg::with_name("address")
        .long("bind")
        .default_value("127.0.0.1")
        .help("Specify bind address")
        .value_name("ADDRESS");

    let arg_cors = Arg::with_name("cors")
        .short("C")
        .long("cors")
        .help("Enable Cross-Origin Resource Sharing from any origin (*)");

    let arg_cache = Arg::with_name("cache")
        .short("c")
        .long("cache")
        .default_value("0")
        .help("Specify max-age of HTTP caching in seconds")
        .value_name("SECONDS");

    let arg_path = Arg::with_name("path")
        .default_value(".")
        .help("Path to a directory for serving files");

    let matches = app_from_crate!()
        .arg(arg_address)
        .arg(arg_port)
        .arg(arg_cache)
        .arg(arg_cors)
        .arg(arg_path)
        .get_matches();

    let address = {
        let ip = matches.value_of("address").unwrap_or_default();
        let port = matches.value_of("port").unwrap_or_default();
        format!("{}:{}", ip, port).parse()
            .map_err(|e| format!("Error: {}", e))
            .unwrap()
    };
    let cache = value_t!(matches.value_of("cache"), u32).unwrap_or_default();
    let cors = matches.is_present("cors");
    let path = {
        let path = matches.value_of("path").unwrap_or_default();
        let path = PathBuf::from(path);
        if path.is_absolute() {
            path
        } else {
            env::current_dir().unwrap_or_default()
                .join(path)
                .canonicalize().unwrap()
        }
    };

    let options = ServerOptions {
        cache,
        cors,
        path,
        ..Default::default()
    };

    println!("Files serve on {}", address);
    serve(&address, options);
}
