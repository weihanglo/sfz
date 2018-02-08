// Copyright (c) 2018 Weihang Lo. All rights reserved.
//
// See the LICENSE file at the top-level directory of this distribution.

extern crate clap;
extern crate futures;
extern crate hyper;
extern crate percent_encoding;
extern crate tera;
extern crate mime_guess;
extern crate unicase;

mod server;

use clap::App;
use server::MyServer;

fn main() {
    App::new("serve")
        .version("0.1.0")
        .author("Weihang Lo <weihanglotw@gmail.com>")
        .about("A simple static file serving command-line tool")
        .get_matches();
    let server = MyServer::new(Default::default());
    server.serve();
}
