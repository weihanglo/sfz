// Copyright (c) 2018 Weihang Lo
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::path::Path;
use std::time::SystemTime;

use hyper::mime::{self, Mime};
use mime_guess::guess_mime_type_opt;

pub trait PathExt {
    fn mime(&self) -> Option<Mime>;
    fn is_hidden(&self) -> bool;
    fn mtime(&self) -> SystemTime;
    fn filename_str(&self) -> &str;
    fn size(&self) -> u64;
}

impl PathExt for Path {
    /// Guess MIME type from a path.
    fn mime(&self) -> Option<Mime> {
        guess_mime_type_opt(&self)
    }

    /// Check if path is hidden.
    fn is_hidden(&self) -> bool {
        self.file_name()
            .and_then(|s| s.to_str())
            .map(|s| s.starts_with("."))
            .unwrap_or(false)
    }

    /// Get modified time from a path.
    fn mtime(&self) -> SystemTime {
        self.metadata()
            .and_then(|meta| meta.modified())
            .unwrap()
    }

    /// Get file size from a path.
    fn size(&self) -> u64 {
        self.metadata()
            .map(|meta| meta.len())
            .unwrap_or_default()
    }

    /// Get a filename `String` from a path.
    fn filename_str(&self) -> &str {
        self.file_name()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
    }
}

pub trait SystemTimeExt {
    fn timestamp_sec(&self) -> u64;
}

impl SystemTimeExt for SystemTime {
    /// Convert `SystemTime` to timestamp in seconds.
    fn timestamp_sec(&self) -> u64 {
        self.duration_since(::std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }
}

pub trait MimeExt {
    fn is_media(&self) -> bool;
}

impl MimeExt for Mime {
    /// Detect if MIME type is `video/*` or `audio/*`
    fn is_media(&self) -> bool {
        match self.type_() {
            mime::VIDEO | mime::AUDIO  => true,
            _ => false,
        }
    }
}


#[cfg(test)]
mod t {
    use super::*;

    #[test]
    fn mime_is_media() {
        assert!("video/*".parse::<mime::Mime>().unwrap().is_media());
        assert!("audio/*".parse::<mime::Mime>().unwrap().is_media());
        assert!(!"text/*".parse::<mime::Mime>().unwrap().is_media());
    }
}
