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
#[cfg(test)]
pub mod test_utils;

use crate::cli::{matches, Args};
use crate::server::serve;

pub type BoxResult<T> = Result<T, Box<dyn std::error::Error>>;

#[tokio::main]
async fn main() {
    Args::parse(matches())
        .map(serve)
        .unwrap_or_else(handle_err)
        .await
        .unwrap_or_else(handle_err);
}

fn handle_err<T>(err: Box<dyn std::error::Error>) -> T {
    eprintln!("Server error: {}", err);
    std::process::exit(1);
}
