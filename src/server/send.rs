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
use std::path::Path;

use ignore::WalkBuilder;
use serde::Serialize;
use tera::{Context, Tera};

use crate::extensions::PathExt;
use crate::server::PathType;

/// Serializable `Item` that would be passed to Tera for template rendering.
/// The order of struct fields is deremined to ensure sorting precedence.
#[derive(Debug, Serialize, Eq, PartialEq, Ord, PartialOrd)]
struct Item {
    path_type: PathType,
    name: String,
    path: String,
}

/// Breadcrumb represents a directory name and a path.
#[derive(Debug, Serialize)]
struct Breadcrumb<'a> {
    name: &'a str,
    path: String,
}

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

    // Breadcrumbs for navigation.
    let breadcrumbs = create_breadcrumbs(dir_path, base_path, prefix);

    // Collect filename and there links.
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

    Ok(render(dir_path.filename_str(), &files, &breadcrumbs).into())
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

/// Create breadcrumbs for navigation.
fn create_breadcrumbs<'a>(
    dir_path: &'a Path,
    base_path: &'a Path,
    prefix: &str,
) -> Vec<Breadcrumb<'a>> {
    let base_breadcrumb = Breadcrumb {
        name: base_path.filename_str(),
        path: format!("{}/", prefix),
    };
    vec![base_breadcrumb]
        .into_iter()
        .chain(
            dir_path
                .strip_prefix(base_path)
                .unwrap()
                .iter()
                .map(|s| s.to_str().unwrap())
                .scan(prefix.to_string(), |path, name| {
                    path.push('/');
                    path.push_str(name);
                    Some(Breadcrumb {
                        name,
                        path: path.clone(),
                    })
                }),
        )
        .collect::<Vec<_>>()
}

/// Render page with Tera template engine.
fn render(dir_name: &str, files: &[Item], breadcrumbs: &[Breadcrumb]) -> String {
    let mut ctx = Context::new();
    ctx.insert("dir_name", dir_name);
    ctx.insert("files", files);
    ctx.insert("breadcrumbs", breadcrumbs);
    ctx.insert("style", include_str!("style.css"));
    Tera::one_off(include_str!("index.html"), &ctx, true)
        .unwrap_or_else(|e| format!("500 Internal server error: {}", e))
}

#[cfg(test)]
mod t {
    use super::*;

    #[test]
    fn render_successfully() {
        let page = render("", &vec![], &vec![]);
        assert!(page.starts_with("<!DOCTYPE html>"))
    }
    #[test]
    fn breadcrumbs() {
        // Only one level
        let base_path = Path::new("/a");
        let dir_path = Path::new("/a");
        let breadcrumbs = create_breadcrumbs(dir_path, base_path, "");
        assert_eq!(breadcrumbs.len(), 1);
        assert_eq!(breadcrumbs[0].name, "a");
        assert_eq!(breadcrumbs[0].path, "/");

        // Nested two levels
        let base_path = Path::new("/a");
        let dir_path = Path::new("/a/b");
        let breadcrumbs = create_breadcrumbs(dir_path, base_path, "");
        assert_eq!(breadcrumbs.len(), 2);
        assert_eq!(breadcrumbs[0].name, "a");
        assert_eq!(breadcrumbs[0].path, "/");
        assert_eq!(breadcrumbs[1].name, "b");
        assert_eq!(breadcrumbs[1].path, "/b");

        // Nested four levels
        let base_path = Path::new("/a");
        let dir_path = Path::new("/a/b/c/d");
        let breadcrumbs = create_breadcrumbs(dir_path, base_path, "");
        assert_eq!(breadcrumbs.len(), 4);
        assert_eq!(breadcrumbs[0].name, "a");
        assert_eq!(breadcrumbs[0].path, "/");
        assert_eq!(breadcrumbs[1].name, "b");
        assert_eq!(breadcrumbs[1].path, "/b");
        assert_eq!(breadcrumbs[2].name, "c");
        assert_eq!(breadcrumbs[2].path, "/b/c");
        assert_eq!(breadcrumbs[3].name, "d");
        assert_eq!(breadcrumbs[3].path, "/b/c/d");
    }

    #[test]
    fn breadcrumbs_with_slashes() {
        let base_path = Path::new("////a/b");
        let dir_path = Path::new("////////a//////b///c////////////");
        let breadcrumbs = create_breadcrumbs(dir_path, base_path, "");
        assert_eq!(breadcrumbs.len(), 2);
        assert_eq!(breadcrumbs[0].name, "b");
        assert_eq!(breadcrumbs[0].path, "/");
        assert_eq!(breadcrumbs[1].name, "c");
        assert_eq!(breadcrumbs[1].path, "/c");
    }

    #[test]
    fn prefixed_breadcrumbs() {
        let base_path = Path::new("/a");
        let dir_path = Path::new("/a/b/c");
        let breadcrumbs = create_breadcrumbs(dir_path, base_path, "xdd~帥//");
        assert_eq!(breadcrumbs.len(), 3);
        assert_eq!(breadcrumbs[0].name, "a");
        assert_eq!(breadcrumbs[0].path, "xdd~帥///");
        assert_eq!(breadcrumbs[1].name, "b");
        assert_eq!(breadcrumbs[1].path, "xdd~帥///b");
        assert_eq!(breadcrumbs[2].name, "c");
        assert_eq!(breadcrumbs[2].path, "xdd~帥///b/c");
    }

    #[test]
    fn breadcrumbs_from_root() {
        let base_path = Path::new("/");
        let dir_path = Path::new("/a/b");
        let breadcrumbs = create_breadcrumbs(dir_path, base_path, "");
        assert_eq!(breadcrumbs.len(), 3);
        assert_eq!(breadcrumbs[0].name, "");
        assert_eq!(breadcrumbs[0].path, "/");
        assert_eq!(breadcrumbs[1].name, "a");
        assert_eq!(breadcrumbs[1].path, "/a");
        assert_eq!(breadcrumbs[2].name, "b");
        assert_eq!(breadcrumbs[2].path, "/a/b");
    }
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
