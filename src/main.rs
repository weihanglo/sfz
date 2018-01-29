// Copyright (c) 2018 Weihang Lo. All rights reserved.
//
// See the LICENSE file at the top-level directory of this distribution.

extern crate clap;
extern crate futures;
extern crate hyper;
extern crate percent_encoding;
extern crate tera;

mod server;

use clap::App;
use server::serve;

fn main() {
    App::new("serve")
        .version("0.1.0")
        .author("Weihang Lo <weihanglotw@gmail.com>")
        .about("A simple static file serving command-line tool")
        .get_matches();
    serve("127.0.0.1", 8080);
}
