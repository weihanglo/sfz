// Copyright (c) 2018 Weihang Lo
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::convert::AsRef;
use std::fs::File;
use std::io::{self, BufReader};
use std::path::{Path, PathBuf};

use ignore::WalkBuilder;
use tera::{Context, Tera};

use super::Item;
use crate::extensions::{PathExt, PathType};

/// Send a HTML page of all files under the path.
///
/// # Parameters
///
/// * `dir_path` - Directory to be listed files.
/// * `base_path` - The base path resolving all filepaths under `dir_path`.
/// * `show_all` - Whether to show hidden and 'dot' files.
/// * `with_ignore` - Whether to respet gitignore files.
/// * `path_prefix` - The url path prefix optionally defined
pub fn send_dir<P1: AsRef<Path>, P2: AsRef<Path>>(
    dir_path: P1,
    base_path: P2,
    show_all: bool,
    with_ignore: bool,
    path_prefix: Option<&str>,
) -> io::Result<Vec<u8>> {
    let base_path = base_path.as_ref();
    let dir_path = dir_path.as_ref();
    // Prepare dirname of current dir relative to base path.
    let prefix = path_prefix.unwrap_or("");

    let (dir_name, paths) = {
        let dir_name = base_path.filename_str();
        let path = dir_path.strip_prefix(base_path).unwrap();

        let path_names = path.iter().map(|s| s.to_str().unwrap());
        let abs_paths = path
            .iter()
            .scan(PathBuf::new(), |state, path| {
                state.push(path);
                Some(state.to_owned())
            })
            .map(|s| format!("/{}", s.to_str().unwrap()));
        // Tuple structure: (name, path)
        let paths = vec![(dir_name, String::new())];

        let paths = paths
            .into_iter()
            .chain(path_names.zip(abs_paths))
            .map(|mut p| {
                p.1 = format!("{}{}", prefix, if p.1.is_empty() { "/" } else { &p.1 },);
                p
            })
            .collect::<Vec<_>>();
        (dir_name, paths)
    };

    let files_iter = WalkBuilder::new(dir_path)
        .standard_filters(false) // Disable all standard filters.
        .git_ignore(with_ignore)
        .hidden(!show_all) // Filter out hidden entries on demand.
        .max_depth(Some(1)) // Do not traverse subpaths.
        .build()
        .filter_map(|entry| entry.ok())
        .filter(|entry| dir_path != entry.path()) // Exclude `.`
        .map(|entry| {
            let abs_path = entry.path();
            // Get relative path.
            let rel_path = abs_path.strip_prefix(base_path).unwrap();
            let rel_path_ref = rel_path.to_str().unwrap_or_default();

            Item {
                path_type: abs_path.type_(),
                name: rel_path.filename_str().to_owned(),
                path: format!("{}/{}", prefix, rel_path_ref),
            }
        });

    let mut files = if base_path == dir_path {
        // CWD == base dir
        files_iter.collect::<Vec<_>>()
    } else {
        // CWD == sub dir of base dir
        // Append an item for popping back to parent directory.

        let path = format!(
            "{}/{}",
            prefix,
            dir_path
                .parent()
                .unwrap()
                .strip_prefix(base_path)
                .unwrap()
                .to_str()
                .unwrap()
        );
        vec![Item {
            name: "..".to_owned(),
            path,
            path_type: PathType::Dir,
        }]
        .into_iter()
        .chain(files_iter)
        .collect::<Vec<_>>()
    };
    // Sort files (dir-first and lexicographic ordering).
    files.sort_unstable();

    // Render page with Tera template engine.
    let mut ctx = Context::new();
    ctx.insert("files", &files);
    ctx.insert("dir_name", &dir_name);
    ctx.insert("paths", &paths);
    ctx.insert("style", include_str!("style.css"));
    let page = Tera::one_off(include_str!("index.html"), &ctx, true)
        .unwrap_or_else(|e| format!("500 Internal server error: {}", e));
    Ok(Vec::from(page))
}

/// Send a buffer of file to client.
pub fn send_file<P: AsRef<Path>>(file_path: P) -> io::Result<Vec<u8>> {
    use std::io::prelude::*;
    let f = File::open(file_path)?;
    let mut buffer = Vec::new();
    BufReader::new(f).read_to_end(&mut buffer)?;
    Ok(buffer)
}

/// Send a buffer with specific range.
///
/// # Parameters
///
/// * `file_path` - Path to the file that is going to send.
/// * `range` - Tuple of `(start, end)` range (inclusive).
pub fn send_file_with_range<P: AsRef<Path>>(
    file_path: P,
    range: (u64, u64),
) -> io::Result<Vec<u8>> {
    use std::io::prelude::*;
    use std::io::SeekFrom;
    let (start, end) = range; // TODO: should return HTTP 416
    if end < start {
        return Err(io::Error::from(io::ErrorKind::InvalidInput));
    }
    let mut f = File::open(file_path)?;
    let mut buffer = Vec::new();
    f.seek(SeekFrom::Start(start))?;
    BufReader::new(f)
        .take(end - start + 1)
        .read_to_end(&mut buffer)?;
    Ok(buffer)
}

#[cfg(test)]
mod t_send {
    use super::*;

    fn file_txt_path() -> std::path::PathBuf {
        let mut path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("./tests/file.txt");
        path
    }

    fn missing_file_path() -> std::path::PathBuf {
        let mut path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("./missing/file");
        path
    }

    #[ignore]
    #[test]
    fn t_send_dir() {}

    #[test]
    fn t_send_file_success() {
        let buf = send_file(file_txt_path());
        assert_eq!(&buf.unwrap(), b"01234567");
    }

    #[test]
    fn t_send_file_not_found() {
        let buf = send_file(missing_file_path());
        assert_eq!(buf.unwrap_err().kind(), std::io::ErrorKind::NotFound);
    }

    #[test]
    fn t_send_file_with_range_one_byte() {
        for i in 0..=7 {
            let buf = send_file_with_range(file_txt_path(), (i, i));
            assert_eq!(buf.unwrap(), i.to_string().as_bytes());
        }
    }

    #[test]
    fn t_send_file_with_range_multiple_bytes() {
        let buf = send_file_with_range(file_txt_path(), (0, 1));
        assert_eq!(buf.unwrap(), b"01");
        let buf = send_file_with_range(file_txt_path(), (1, 2));
        assert_eq!(buf.unwrap(), b"12");
        let buf = send_file_with_range(file_txt_path(), (1, 4));
        assert_eq!(buf.unwrap(), b"1234");
        let buf = send_file_with_range(file_txt_path(), (7, 65535));
        assert_eq!(buf.unwrap(), b"7");
        let buf = send_file_with_range(file_txt_path(), (8, 8));
        assert_eq!(buf.unwrap(), b"");
    }

    #[test]
    fn t_send_file_with_range_not_found() {
        let buf = send_file_with_range(missing_file_path(), (0, 0));
        assert_eq!(buf.unwrap_err().kind(), std::io::ErrorKind::NotFound);
    }

    #[test]
    fn t_send_file_with_range_invalid_range() {
        // TODO: HTTP code 416
        let buf = send_file_with_range(file_txt_path(), (1, 0));
        assert_eq!(buf.unwrap_err().kind(), std::io::ErrorKind::InvalidInput);
    }
}
