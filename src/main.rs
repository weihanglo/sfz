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

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let handle = tokio::spawn(async {
        use std::io::Read;
        std::io::stdin()
            .read_to_end(&mut Vec::new())
            .unwrap_or_else(|e| handle_err(Box::new(e)));
        tx.send(()).unwrap();
    });
    Args::parse(matches())
        .map(|args| async {
            tokio::join!(
                serve(args, async {
                    rx.await.ok();
                    eprintln!("Exit gracefully. Bye!");
                }),
                handle
            )
            .0
        })
        .unwrap_or_else(handle_err)
        .await
        .unwrap_or_else(handle_err);
}

fn handle_err<T>(err: Box<dyn std::error::Error>) -> T {
    eprintln!("Server error: {}", err);
    std::process::exit(1);
}
