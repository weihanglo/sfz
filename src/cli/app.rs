// Copyright (c) 2018 Weihang Lo
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

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

    let arg_all = Arg::with_name("all")
        .short("a")
        .long("all")
        .help("Serve hidden and dot (.) files");

    let arg_no_ignore = Arg::with_name("no-ignore")
        .long("no-ignore")
        .help("Don't respect gitignore file");

    app_from_crate!()
        .about(ABOUT)
        .arg(arg_address)
        .arg(arg_port)
        .arg(arg_cache)
        .arg(arg_cors)
        .arg(arg_path)
        .arg(arg_unzipped)
        .arg(arg_all)
        .arg(arg_no_ignore)
}
