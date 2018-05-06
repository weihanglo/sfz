// Copyright (c) 2018 Weihang Lo
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

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
extern crate ignore;
extern crate chrono;

#[cfg(test)]
extern crate tempdir;

macro_rules! bail {
    ($($tt:tt)*) => {
        return Err(From::from(format!($($tt)*)));
    }
}

mod server;
mod http;
mod cli;
mod extensions;

use std::process;
use std::error::Error;
use std::sync::Arc;
use cli::{Args, app};
use server::serve;

pub type BoxResult<T> = Result<T, Box<Error>>;

fn main() {
    let result = Args::parse(app())
        .map(Arc::new)
        .and_then(serve);
    match result {
        Ok(_) => (),
        Err(err) => {
            eprintln!("{}", err);
            process::exit(1)
        }
    }
}
