// Copyright (c) 2018 Weihang Lo. All rights reserved.
//
// See the LICENSE file at the top-level directory of this distribution.

use std::env;
use std::path::PathBuf;
use std::net::SocketAddr;

use clap::ArgMatches;

#[derive(Debug, Clone)]
pub struct Args {
    pub address: String,
    pub port: u16,
    pub cache: u32,
    pub cors: bool,
    pub compress: bool,
    pub path: PathBuf,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            address: "127.0.0.1".to_owned(),
            port: 8888,
            cache: 0,
            compress: true,
            cors: false,
            path: env::current_dir().unwrap_or_default(),
        }
    }
}

impl Args {
    /// Parse arguments.
    pub fn parse(matches: ArgMatches) -> Args {
        let address = matches.value_of("address")
            .unwrap_or_default()
            .to_owned();
        let port = value_t!(matches.value_of("port"), u16)
            .unwrap_or_default();
        let cache = value_t!(matches.value_of("cache"), u32)
            .unwrap_or_default();
        let cors = matches.is_present("cors");
        let path = {
            let path = matches.value_of("path").unwrap_or_default();
            let path = PathBuf::from(path);
            if path.is_absolute() {
                path
            } else {
                env::current_dir().unwrap_or_default()
                    .join(path)
                    .canonicalize().unwrap()
            }
        };
        let compress = !matches.is_present("unzipped");

        Args {
            address,
            port,
            cache,
            cors,
            path,
            compress,
            ..Default::default()
        }
    }

    /// Construct socket address from arguments.
    pub fn address(&self) -> SocketAddr {
        format!("{}:{}", self.address, self.port).parse().unwrap()
    }
}
