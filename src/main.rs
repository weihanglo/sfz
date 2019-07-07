// Copyright (c) 2018 Weihang Lo
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

macro_rules! bail {
    ($($tt:tt)*) => {
        return Err(From::from(format!($($tt)*)));
    }
}

mod cli;
mod extensions;
mod http;
mod server;

use std::error::Error;
use std::process;
use std::sync::Arc;

use crate::cli::{app, Args};
use crate::server::serve;

pub type BoxResult<T> = Result<T, Box<dyn Error>>;

fn main() {
    let result = Args::parse(app()).map(Arc::new).and_then(serve);
    match result {
        Ok(_) => (),
        Err(err) => {
            dbg!(err);
            process::exit(1)
        }
    }
}
