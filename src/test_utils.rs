// Copyright (c) 2020 Weihang Lo
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::convert::AsRef;
use std::env;
use std::ops::FnOnce;
use std::panic;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use once_cell::sync::Lazy;

static LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

static TESTS_DIR: Lazy<PathBuf> = Lazy::new(|| {
    let path: &Path = env!("CARGO_MANIFEST_DIR").as_ref();
    path.join("tests")
});

pub fn get_tests_dir() -> impl AsRef<Path> {
    TESTS_DIR.as_path()
}

/// Ensure only one thread can access `std::env::set_current_dir` at the same
/// time. Also reset current working directory after dropping.
pub fn with_current_dir<P, F>(current_dir: P, f: F)
where
    P: AsRef<Path>,
    F: FnOnce() + panic::UnwindSafe,
{
    let _lock = LOCK.lock().unwrap();

    let old_cwd = env::current_dir().expect("store current working directory");
    env::set_current_dir(current_dir).expect("set current working directory");

    let result = panic::catch_unwind(f);

    env::set_current_dir(old_cwd).expect("restore current working directory");

    if let Err(e) = result {
        panic::resume_unwind(e)
    }
}
