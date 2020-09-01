// Copyright (c) 2018 Weihang Lo
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::path::{Component, Path};
use std::time::SystemTime;

use mime_guess::{mime, Mime};

use crate::server::PathType;

pub trait PathExt {
    fn mime(&self) -> Option<Mime>;
    fn is_relatively_hidden(&self) -> bool;
    fn mtime(&self) -> SystemTime;
    fn filename_str(&self) -> &str;
    fn size(&self) -> u64;
    fn type_(&self) -> PathType;
}

impl PathExt for Path {
    /// Guess MIME type from a path.
    fn mime(&self) -> Option<Mime> {
        mime_guess::from_path(&self).first()
    }

    /// Check if a path is relatively hidden.
    ///
    /// A path is "relatively hidden" means that if any component of the path
    /// is hidden, no matter whether the path's basename is prefixed with `.`
    /// or not, it is considered as hidden.
    fn is_relatively_hidden(&self) -> bool {
        self.components()
            .filter_map(|c| match c {
                Component::Normal(os_str) => os_str.to_str(),
                _ => None,
            })
            .any(|s| s.starts_with('.'))
    }

    /// Get modified time from a path.
    fn mtime(&self) -> SystemTime {
        self.metadata().and_then(|meta| meta.modified()).unwrap()
    }

    /// Get file size, in bytes, from a path.
    fn size(&self) -> u64 {
        self.metadata().map(|meta| meta.len()).unwrap_or_default()
    }

    /// Get a filename `&str` from a path.
    fn filename_str(&self) -> &str {
        self.file_name()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
    }

    /// Determine given path is a normal file/directory or a symlink.
    fn type_(&self) -> PathType {
        self.symlink_metadata()
            .map(|meta| {
                let is_symlink = meta.file_type().is_symlink();
                let is_dir = self.is_dir();
                match (is_symlink, is_dir) {
                    (true, true) => PathType::SymlinkDir,
                    (false, true) => PathType::Dir,
                    (true, false) => PathType::SymlinkFile,
                    (false, false) => PathType::File,
                }
            })
            .unwrap_or(PathType::File)
    }
}

pub trait SystemTimeExt {
    fn timestamp(&self) -> u64;
}

impl SystemTimeExt for SystemTime {
    /// Convert `SystemTime` to timestamp in seconds.
    fn timestamp(&self) -> u64 {
        self.duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }
}

pub trait MimeExt {
    fn is_compressed_format(&self) -> bool;
}

impl MimeExt for Mime {
    /// Detect if MIME type is
    ///
    /// - `video/*`
    /// - `audio/*`
    /// - `*/GIF`
    /// - `*/JPEG`
    /// - `*/PNG`
    fn is_compressed_format(&self) -> bool {
        match (self.type_(), self.subtype()) {
            (mime::VIDEO, _) | (mime::AUDIO, _) => true,
            (_, mime::GIF) | (_, mime::JPEG) | (_, mime::PNG) => true,
            _ => false,
        }
    }
}

#[cfg(test)]
mod t_extensions {
    use super::*;
    use std::path::PathBuf;

    fn file_txt_path() -> PathBuf {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("./tests/file.txt");
        path
    }

    fn hidden_html_path() -> PathBuf {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("./tests/.hidden.html");
        path
    }

    #[test]
    fn path_mime() {
        assert_eq!(file_txt_path().mime(), Some(mime::TEXT_PLAIN));
        assert_eq!(hidden_html_path().mime(), Some(mime::TEXT_HTML));
    }

    #[test]
    fn path_is_relatively_hidden() {
        assert!(hidden_html_path().is_relatively_hidden());

        let path = "./.hidden/visible.html";
        assert!(PathBuf::from(path).is_relatively_hidden());
    }

    #[test]
    fn path_is_not_relatively_hidden() {
        assert!(!file_txt_path().is_relatively_hidden());

        let path = "./visible/visible.html";
        assert!(!PathBuf::from(path).is_relatively_hidden());
    }

    #[ignore]
    #[test]
    fn path_mtime() {}

    #[test]
    fn path_size() {
        assert_eq!(file_txt_path().size(), 8);
        assert_eq!(hidden_html_path().size(), 0);
    }

    #[test]
    fn path_filename_str() {
        assert_eq!(file_txt_path().filename_str(), "file.txt");
        assert_eq!(hidden_html_path().filename_str(), ".hidden.html");
    }

    #[test]
    fn path_type_() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

        let mut dir_path = path.clone();
        dir_path.push("./tests/dir");
        assert_eq!(dir_path.type_(), PathType::Dir);

        let mut symlink_dir_path = path.clone();
        symlink_dir_path.push("./tests/symlink_dir");
        assert_eq!(symlink_dir_path.type_(), PathType::SymlinkDir);

        assert_eq!(file_txt_path().type_(), PathType::File);

        let mut symlink_file_txt_path = path.clone();
        symlink_file_txt_path.push("./tests/symlink_file.txt");
        assert_eq!(symlink_file_txt_path.type_(), PathType::SymlinkFile);
    }

    #[test]
    fn system_time_to_timestamp() {
        use std::time::Duration;
        let secs = 1000;
        let tm = SystemTime::UNIX_EPOCH + Duration::from_secs(secs);
        assert_eq!(tm.timestamp(), secs);
    }

    #[test]
    fn mime_is_compressed() {
        assert!("video/*"
            .parse::<mime::Mime>()
            .unwrap()
            .is_compressed_format());
        assert!("audio/*"
            .parse::<mime::Mime>()
            .unwrap()
            .is_compressed_format());
        assert!("*/gif"
            .parse::<mime::Mime>()
            .unwrap()
            .is_compressed_format());
        assert!("*/jpeg"
            .parse::<mime::Mime>()
            .unwrap()
            .is_compressed_format());
        assert!("*/png"
            .parse::<mime::Mime>()
            .unwrap()
            .is_compressed_format());
        assert!(!"text/*"
            .parse::<mime::Mime>()
            .unwrap()
            .is_compressed_format());
    }
}
