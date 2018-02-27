// Copyright (c) 2018 Weihang Lo. All rights reserved.
//
// See the LICENSE file at the top-level directory of this distribution.

use clap::{Arg, App};

const ABOUT: &str = concat!("\n", crate_description!()); // Add extra newline.

pub fn build_app() -> App<'static, 'static> {
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
        .short("c")
        .long("cors")
        .help("Enable Cross-Origin Resource Sharing from any origin (*)");

    let arg_cache = Arg::with_name("cache")
        .long("cache")
        .default_value("0")
        .help("Specify max-age of HTTP caching in seconds")
        .value_name("SECONDS");

    let arg_path = Arg::with_name("path")
        .default_value(".")
        .help("Path to a directory for serving files");

    let arg_unzipped = Arg::with_name("unzipped")
        .short("Z")
        .long("unzipped")
        .help("Disable HTTP compression");

    app_from_crate!()
        .about(ABOUT)
        .arg(arg_address)
        .arg(arg_port)
        .arg(arg_cache)
        .arg(arg_cors)
        .arg(arg_path)
        .arg(arg_unzipped)
}
