// Copyright (c) 2018 Weihang Lo
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::env;
use std::fs::canonicalize;
use std::net::SocketAddr;
use std::path::PathBuf;

use clap::App;

use BoxResult;

#[derive(Debug, Clone)]
pub struct Args {
    pub address: String,
    pub port: u16,
    pub cache: u32,
    pub cors: bool,
    pub compress: bool,
    pub path: PathBuf,
    pub all: bool,
    pub ignore: bool,
    pub follow_links: bool,
    pub render_index: bool,
    pub log: bool,
}

impl Args {
    /// Parse command-line arguments.
    ///
    /// If a parsing error ocurred, exit the process and print out informative
    /// error message to user.
    pub fn parse(app: App) -> BoxResult<Args> {
        let matches = app.get_matches();
        let address = matches.value_of("address").unwrap_or_default().to_owned();
        let port = value_t!(matches.value_of("port"), u16)?;
        let cache = value_t!(matches.value_of("cache"), u32)?;
        let cors = matches.is_present("cors");
        let path = matches.value_of("path").unwrap_or_default();
        let path = Args::parse_path(path)?;

        let compress = !matches.is_present("unzipped");
        let all = matches.is_present("all");
        let ignore = !matches.is_present("no-ignore");
        let follow_links = matches.is_present("follow-links");
        let render_index = matches.is_present("render-index");
        let log = !matches.is_present("no-log");

        Ok(Args {
            address,
            port,
            cache,
            cors,
            path,
            compress,
            all,
            ignore,
            follow_links,
            render_index,
            log,
        })
    }

    /// Parse path.
    fn parse_path(path: &str) -> BoxResult<PathBuf> {
        let path = PathBuf::from(path);
        if !path.exists() {
            bail!("error: path \"{}\" doesn't exist", path.display());
        }

        (if path.is_absolute() {
            path.canonicalize()
        } else {
            env::current_dir().map(|p| p.join(&path))
        })
        .and_then(canonicalize)
        .or_else(|err| {
            bail!(
                "error: failed to access path \"{}\": {}",
                path.display(),
                err,
            )
        })
    }

    /// Construct socket address from arguments.
    pub fn address(&self) -> BoxResult<SocketAddr> {
        format!("{}:{}", self.address, self.port)
            .parse()
            .or_else(|err| {
                bail!(
                    "error: invalid address {}:{} : {}",
                    self.address,
                    self.port,
                    err,
                )
            })
    }
}

#[cfg(test)]
mod t {
    use super::*;
    use std::fs::File;
    use tempdir::TempDir;

    fn temp_name() -> &'static str {
        concat!(env!("CARGO_PKG_NAME"), "-", env!("CARGO_PKG_VERSION"))
    }

    #[test]
    fn parse_absolute_path() {
        let tmp_dir = TempDir::new(temp_name()).unwrap();
        let path = tmp_dir.path().join("temp.txt");
        let path_str = path.to_str().unwrap();
        assert!(path.is_absolute());
        // error: No exists
        assert!(Args::parse_path(path_str).is_err());
        // create file
        File::create(&path).unwrap();
        assert!(Args::parse_path(path_str).is_ok());
    }

    #[test]
    fn parse_relative_path() {
        let tmp_dir = TempDir::new(temp_name()).unwrap();
        let path = tmp_dir.path().join("temp.txt");
        let relative_path = path.strip_prefix(tmp_dir.path()).unwrap();
        let relative_path_str = path.to_str().unwrap();
        env::set_current_dir(tmp_dir.path()).unwrap();
        File::create(&relative_path).unwrap();
        assert!(relative_path.is_relative());
        // Relative path is ok
        assert!(Args::parse_path(relative_path_str).is_ok());
    }
}
