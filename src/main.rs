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
// mod http;
mod server;

use crate::cli::{app, Args};
use crate::server::serve;

pub type BoxResult<T> = Result<T, Box<dyn std::error::Error>>;

#[tokio::main]
async fn main() {
    let args = Args::parse(app()).unwrap_or_else(handle_err);
    serve(args).await.unwrap_or_else(handle_err);
}

fn handle_err<T>(err: Box<dyn std::error::Error>) -> T {
    dbg!(err);
    std::process::exit(1);
}
