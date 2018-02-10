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

mod server;

use clap::Arg;
use server::{serve, ServerOptions};

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
            .long("cors")
            .help("Enable Cross-Origin Resource Sharing from any origin (*)");

    let matches = app_from_crate!()
        .arg(arg_port)
        .arg(arg_address)
        .arg(arg_cors)
        .get_matches();

    let address = {
        let ip = matches.value_of("address").unwrap_or_default();
        let port = matches.value_of("port").unwrap_or_default();
        format!("{}:{}", ip, port).parse()
            .map_err(|e| format!("Error: {}", e))
            .unwrap()
    };

    let cors = matches.is_present("cors");
    let options = ServerOptions {
        cors,
        ..Default::default()
    };

    println!("Files serve on {}", address);
    serve(&address, options);
}
