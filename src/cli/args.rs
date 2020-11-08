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
use std::path::{Path, PathBuf};

use clap::{value_t, ArgMatches};

use crate::BoxResult;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Args {
    pub address: String,
    pub port: u16,
    pub cache: u64,
    pub cors: bool,
    pub compress: bool,
    pub path: PathBuf,
    pub all: bool,
    pub ignore: bool,
    pub follow_links: bool,
    pub render_index: bool,
    pub log: bool,
    pub path_prefix: Option<String>,
}

impl Args {
    /// Parse command-line arguments.
    ///
    /// If a parsing error ocurred, exit the process and print out informative
    /// error message to user.
    pub fn parse(matches: ArgMatches<'_>) -> BoxResult<Args> {
        let address = matches.value_of("address").unwrap_or_default().to_owned();
        let port = value_t!(matches.value_of("port"), u16)?;
        let cache = value_t!(matches.value_of("cache"), u64)?;
        let cors = matches.is_present("cors");
        let path = matches.value_of("path").unwrap_or_default();
        let path = Args::parse_path(path)?;

        let compress = !matches.is_present("unzipped");
        let all = matches.is_present("all");
        let ignore = !matches.is_present("no-ignore");
        let follow_links = matches.is_present("follow-links");
        let render_index = matches.is_present("render-index");
        let log = !matches.is_present("no-log");
        let path_prefix = matches
            .value_of("path-prefix")
            .map(|s| format!("/{}", s.trim_start_matches('/')));

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
            path_prefix,
        })
    }

    /// Parse path.
    fn parse_path<P: AsRef<Path>>(path: P) -> BoxResult<PathBuf> {
        let path = path.as_ref();
        if !path.exists() {
            bail!("error: path \"{}\" doesn't exist", path.display());
        }

        env::current_dir()
            .and_then(|mut p| {
                p.push(path); // If path is absolute, it replaces the current path.
                canonicalize(p)
            })
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
    use crate::matches;
    use crate::test_utils::with_current_dir;
    use std::fs::File;
    use tempfile::Builder;

    impl Default for Args {
        /// Just for convenience. We do not need a default at this time.
        fn default() -> Self {
            Self {
                address: "127.0.0.1".to_owned(),
                port: 5000,
                cache: 0,
                cors: true,
                compress: true,
                path: ".".into(),
                all: true,
                ignore: true,
                follow_links: true,
                render_index: true,
                log: true,
                path_prefix: None,
            }
        }
    }

    const fn temp_name() -> &'static str {
        concat!(env!("CARGO_PKG_NAME"), "-", env!("CARGO_PKG_VERSION"))
    }

    #[test]
    fn parse_default() {
        let current_dir = env!("CARGO_MANIFEST_DIR");
        with_current_dir(current_dir, || {
            let args = Args::parse(matches()).unwrap();

            // See following link to figure out why we need canonicalize here.
            // Thought this leaks some internal implementations....
            //
            // - https://stackoverflow.com/a/41233992/8851735
            // - https://stackoverflow.com/q/50322817/8851735
            let path = PathBuf::from(current_dir).canonicalize().unwrap();

            assert_eq!(
                args,
                Args {
                    address: "127.0.0.1".to_string(),
                    all: false,
                    cache: 0,
                    compress: true,
                    cors: false,
                    follow_links: false,
                    ignore: true,
                    log: true,
                    path,
                    path_prefix: None,
                    render_index: false,
                    port: 5000
                }
            );
        });
    }

    #[test]
    fn parse_absolute_path() {
        let tmp_dir = Builder::new().prefix(temp_name()).tempdir().unwrap();
        let path = tmp_dir.path().join("temp.txt");
        assert!(path.is_absolute());
        // error: No exists
        assert!(Args::parse_path(&path).is_err());
        // create file
        File::create(&path).unwrap();
        assert_eq!(
            Args::parse_path(&path).unwrap(),
            path.canonicalize().unwrap(),
        );
    }

    #[test]
    fn parse_relative_path() {
        let tmp_dir = Builder::new().prefix(temp_name()).tempdir().unwrap();
        with_current_dir(tmp_dir.path(), || {
            let relative_path: &Path = "temp.txt".as_ref();
            File::create(relative_path).unwrap();

            assert!(relative_path.is_relative());
            assert_eq!(
                Args::parse_path(relative_path).unwrap(),
                tmp_dir.path().join(relative_path).canonicalize().unwrap(),
            );
        });
    }

    #[test]
    fn parse_addresses() {
        // IPv4
        let args = Args::default();
        assert!(args.address().is_ok());

        // IPv6
        let args = Args {
            address: "[::1]".to_string(),
            ..Default::default()
        };
        assert!(args.address().is_ok());

        // Invalid
        let args = Args {
            address: "".to_string(),
            ..Default::default()
        };
        assert!(args.address().is_err());
    }
}
