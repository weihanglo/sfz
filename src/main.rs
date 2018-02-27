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
extern crate flate2;
extern crate brotli;

mod server;
mod http;
mod cli;

use ::cli::{Args, build_app};
use ::server::serve;

fn main() {
    let app = build_app();
    let args = Args::parse(app.get_matches());

    println!("Files serve on {}", args.address());
    serve(args);
}
