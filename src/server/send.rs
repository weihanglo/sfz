// Copyright (c) 2018 Weihang Lo
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::convert::AsRef;
use std::fs as std_fs;
use std::io::{self, Seek};
use std::ops::Range;
use std::path::Path;

use futures::Stream;
use ignore::WalkBuilder;
use once_cell::sync::OnceCell;
use serde::Serialize;
use sysinfo::{RefreshKind, System, SystemExt};
use tera::{Context, Tera};
use tokio::fs as tokio_fs;
use tokio_util::io::ReaderStream;
use zip::ZipWriter;

use crate::extensions::PathExt;
use crate::server::PathType;

const BUFFER_MB_RANGE: Range<u64> = 1..16;

fn get_sysinfo_collector() -> &'static System {
    static INSTANCE: OnceCell<System> = OnceCell::new();
    INSTANCE.get_or_init(|| System::new_with_specifics(RefreshKind::new().with_memory()))
}

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

/// Walking inside a directory recursively
fn get_dir_contents<P: AsRef<Path>>(
    dir_path: P,
    with_ignore: bool,
    show_all: bool,
    depth: Option<usize>,
) -> ignore::Walk {
    WalkBuilder::new(dir_path)
        .standard_filters(false) // Disable all standard filters.
        .git_ignore(with_ignore)
        .hidden(!show_all) // Filter out hidden entries on demand.
        .max_depth(depth) // Do not traverse subpaths.
        .build()
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
) -> io::Result<(impl Stream<Item = io::Result<bytes::Bytes>>, usize)> {
    let base_path = base_path.as_ref();
    let dir_path = dir_path.as_ref();
    // Prepare dirname of current dir relative to base path.
    let prefix = path_prefix.unwrap_or("");

    // Breadcrumbs for navigation.
    let breadcrumbs = create_breadcrumbs(dir_path, base_path, prefix);

    // Collect filename and there links.
    let files_iter = get_dir_contents(dir_path, with_ignore, show_all, Some(1))
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
                path: format!(
                    "{}/{}",
                    prefix,
                    if cfg!(windows) {
                        rel_path_ref.replace("\\", "/")
                    } else {
                        rel_path_ref.to_string()
                    }
                ),
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

    let content: Vec<u8> = render(dir_path.filename_str(), &files, &breadcrumbs).into();
    let size = content.len();
    Ok((ReaderStream::new(io::Cursor::new(content)), size))
}

/// Send a stream of file to client.
pub async fn send_file<P: AsRef<Path>>(
    file_path: P,
) -> io::Result<(impl Stream<Item = io::Result<bytes::Bytes>>, u64)> {
    let file = tokio_fs::File::open(file_path).await?;
    let size = file.metadata().await?.len();
    Ok((ReaderStream::new(file), size))
}

/// Sending a directory as zip buffer
pub async fn send_dir_as_zip<P: AsRef<Path>>(
    dir_path: P,
    show_all: bool,
    with_ignore: bool,
) -> io::Result<(impl Stream<Item = io::Result<bytes::Bytes>>, u64)> {
    let dir_path = dir_path.as_ref();

    // Creating a memory buffer to make zip file
    let mut file = tempfile::tempfile()?;

    {
        let mut zip_writer = ZipWriter::new(&mut file);
        let zip_options = zip::write::FileOptions::default()
            .compression_method(zip::CompressionMethod::Stored)
            .unix_permissions(0o755);

        // Recursively finding files and directories
        let files_iter = get_dir_contents(dir_path, with_ignore, show_all, None)
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.path() != dir_path);

        for dir_entry in files_iter {
            let file_path = dir_entry.path();
            let name = file_path.strip_prefix(dir_path).unwrap().to_str().unwrap();

            if file_path.is_dir() {
                zip_writer
                    .add_directory(name, zip_options)
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            } else {
                zip_writer
                    .start_file(name, zip_options)
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
                let mut file = std_fs::File::open(file_path)?;
                io::copy(&mut file, &mut zip_writer)?;
            }
        }

        zip_writer
            .finish()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    }

    file.seek(io::SeekFrom::Start(0))?;

    let size = file.metadata()?.len();
    let stream = ReaderStream::new(tokio_fs::File::from_std(file));
    Ok((stream, size))
}

/// Send a buffer with specific range.
///
/// # Parameters
///
/// * `file_path` - Path to the file that is going to send.
/// * `range` - Tuple of `(start, end)` range (inclusive).
pub async fn send_file_with_range<P: AsRef<Path>>(
    file_path: P,
    range: (u64, u64),
) -> io::Result<impl Stream<Item = io::Result<bytes::Bytes>>> {
    use tokio::io::{AsyncReadExt, AsyncSeekExt, BufReader};

    let (start, end) = range; // TODO: should return HTTP 416
    if end < start {
        return Err(io::Error::from(io::ErrorKind::InvalidInput));
    }

    let available_kb = get_sysinfo_collector().available_memory();
    if available_kb < BUFFER_MB_RANGE.start {
        return Err(io::Error::from(io::ErrorKind::OutOfMemory));
    }

    let mut f = tokio_fs::File::open(file_path).await?;
    let mut buffer = Vec::new();
    f.seek(io::SeekFrom::Start(start)).await?;

    let capacity = available_kb.clamp(BUFFER_MB_RANGE.start, BUFFER_MB_RANGE.end) * 1_024 * 1_024;
    let capacity = capacity.clamp(0, end - start + 1);
    BufReader::new(f)
        .take(capacity)
        .read_to_end(&mut buffer)
        .await?;

    Ok(ReaderStream::new(io::Cursor::new(buffer)))
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
        let breadcrumbs = create_breadcrumbs(dir_path, base_path, "/xdd~帥//");
        assert_eq!(breadcrumbs.len(), 3);
        assert_eq!(breadcrumbs[0].name, "a");
        assert_eq!(breadcrumbs[0].path, "/xdd~帥///");
        assert_eq!(breadcrumbs[1].name, "b");
        assert_eq!(breadcrumbs[1].path, "/xdd~帥///b");
        assert_eq!(breadcrumbs[2].name, "c");
        assert_eq!(breadcrumbs[2].path, "/xdd~帥///b/c");
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
    use std::marker;

    use tokio_util::io::StreamReader;

    use super::*;

    fn file_txt_path() -> std::path::PathBuf {
        let mut path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("./tests/file.txt");
        path
    }

    fn dir_with_sub_dir_path() -> std::path::PathBuf {
        let mut path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("./tests/dir_with_sub_dirs/");
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

    async fn vec_from_stream(
        s: impl Stream<Item = io::Result<bytes::Bytes>> + marker::Unpin,
    ) -> Vec<u8> {
        use tokio::io::AsyncReadExt;

        let mut buf = Vec::new();
        StreamReader::new(s).read_to_end(&mut buf).await.unwrap();
        buf
    }

    #[tokio::test]
    async fn t_send_file_success() {
        let (s, size) = send_file(file_txt_path()).await.unwrap();
        assert!(size > 0);

        let v = vec_from_stream(s).await;
        assert_eq!(v, b"01234567");
    }

    #[tokio::test]
    async fn t_send_file_not_found() {
        let buf = send_file(missing_file_path()).await;
        assert_eq!(buf.err().unwrap().kind(), io::ErrorKind::NotFound);
    }

    #[tokio::test]
    async fn t_send_file_with_range_one_byte() {
        for i in 0..=7 {
            let s = send_file_with_range(file_txt_path(), (i, i)).await;
            let v = vec_from_stream(s.unwrap()).await;
            assert_eq!(v, i.to_string().as_bytes());
        }
    }

    #[tokio::test]
    async fn t_send_file_with_range_multiple_bytes() {
        let s = send_file_with_range(file_txt_path(), (0, 1)).await;
        let v = vec_from_stream(s.unwrap()).await;
        assert_eq!(v, b"01");
        let s = send_file_with_range(file_txt_path(), (1, 2)).await;
        let v = vec_from_stream(s.unwrap()).await;
        assert_eq!(v, b"12");
        let s = send_file_with_range(file_txt_path(), (1, 4)).await;
        let v = vec_from_stream(s.unwrap()).await;
        assert_eq!(v, b"1234");
        let s = send_file_with_range(file_txt_path(), (7, 65535)).await;
        let v = vec_from_stream(s.unwrap()).await;
        assert_eq!(v, b"7");
        let s = send_file_with_range(file_txt_path(), (8, 8)).await;
        let v = vec_from_stream(s.unwrap()).await;
        assert_eq!(v, b"");
    }

    #[tokio::test]
    async fn t_send_file_with_range_not_found() {
        let buf = send_file_with_range(missing_file_path(), (0, 0)).await;
        assert_eq!(buf.err().unwrap().kind(), io::ErrorKind::NotFound);
    }

    #[tokio::test]
    async fn t_send_file_with_range_invalid_range() {
        // TODO: HTTP code 416
        let buf = send_file_with_range(file_txt_path(), (1, 0)).await;
        assert_eq!(buf.err().unwrap().kind(), io::ErrorKind::InvalidInput);
    }

    #[tokio::test]
    async fn t_send_dir_as_zip() {
        use bytes::Buf;
        use futures::StreamExt;
        use std::io::Read;

        let (mut stream, size) = send_dir_as_zip(dir_with_sub_dir_path(), true, false)
            .await
            .unwrap();
        assert!(size > 0);

        let mut buf: [u8; 4] = [0; 4];
        let mut reader = stream.next().await.unwrap().unwrap().reader();
        reader.read_exact(&mut buf).unwrap();
        // https://users.cs.jmu.edu/buchhofp/forensics/formats/pkzip.html#localheader
        assert_eq!(&buf[0..4], &[0x50, 0x4b, 0x03, 0x04]);
    }
}
