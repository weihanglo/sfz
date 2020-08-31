// Copyright (c) 2018 Weihang Lo
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use clap::{app_from_crate, crate_authors, crate_description, crate_name, crate_version};
use clap::{Arg, ArgMatches};

const ABOUT: &str = concat!("\n", crate_description!()); // Add extra newline.

pub fn matches<'a>() -> ArgMatches<'a> {
    let arg_port = Arg::with_name("port")
        .short("p")
        .long("port")
        .default_value("5000")
        .help("Specify port to listen on")
        .value_name("port");

    let arg_address = Arg::with_name("address")
        .short("b")
        .long("bind")
        .default_value("127.0.0.1")
        .help("Specify bind address")
        .value_name("address");

    let arg_cors = Arg::with_name("cors")
        .short("C")
        .long("cors")
        .help("Enable Cross-Origin Resource Sharing from any origin (*)");

    let arg_cache = Arg::with_name("cache")
        .short("c")
        .long("cache")
        .default_value("0")
        .help("Specify max-age of HTTP caching in seconds")
        .value_name("seconds");

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
        .short("I")
        .long("no-ignore")
        .help("Don't respect gitignore file");

    let arg_no_log = Arg::with_name("no-log")
        .long("--no-log")
        .help("Don't log any request/response information.");

    let arg_follow_links = Arg::with_name("follow-links")
        .short("L")
        .long("--follow-links")
        .help("Follow symlinks outside current serving base path");

    let arg_render_index = Arg::with_name("render-index")
        .short("r")
        .long("--render-index")
        .help("Render existing index.html when requesting a directory.");

    let arg_path_prefix = Arg::with_name("path-prefix")
        .long("path-prefix")
        .help("Specify an url path prefix, helpful when running behing a reverse proxy")
        .value_name("path");

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
        .arg(arg_no_log)
        .arg(arg_follow_links)
        .arg(arg_render_index)
        .arg(arg_path_prefix)
        .get_matches()
}

#[cfg(test)]
mod t {
    use super::*;

    #[test]
    fn get_matches() {
        let matches = matches();
        assert!(matches.usage().starts_with("USAGE"))
    }
}
