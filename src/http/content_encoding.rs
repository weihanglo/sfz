// Copyright (c) 2018 Weihang Lo
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::cmp::Ordering;
use std::io::{self, BufReader};

use hyper::Request;
use hyper::header::{AcceptEncoding, Encoding, QualityItem, q};
use brotli;
use flate2::Compression;
use flate2::read::{GzEncoder, DeflateEncoder};

/// Sorting encodings according to the rank:
///
/// 1. Brotili
/// 2. Gzip
/// 3. Deflate
///
/// The function only accecpt Brotli, Gzip and Deflate encodings, passing other
/// encodings in may lead to a unexpected result.
fn sort_encoding(
    a: &&QualityItem<Encoding>,
    b: &&QualityItem<Encoding>,
) -> Ordering {
    a.quality.cmp(&b.quality).then_with(|| {
        match (&a.item, &b.item) {
            (&Encoding::Brotli, _) => Ordering::Greater,
            (_, &Encoding::Brotli) => Ordering::Less,
            (&Encoding::Gzip, &Encoding::Deflate) => Ordering::Greater,
            (&Encoding::Deflate, &Encoding::Gzip) => Ordering::Less,
            _ => Ordering::Equal,
        }
    })
}

/// Get prior encoding from `Accept-Encoding` header field.
///
/// To get the priority of accept encodings, compare quality values first.
/// If one's quality values equals to the other's, sort by actual encodings.
pub fn get_prior_encoding(req: &Request) -> Encoding {
    req.headers().get::<AcceptEncoding>()
        .and_then(|accept_encoding| {
            let mut vec = vec![];
            let AcceptEncoding(ref encodings) = *accept_encoding;
            let zero = q(0); // Zero means unacceptable.
            for encoding in encodings {
                if encoding.quality <= zero {
                    continue;
                }
                match encoding.item {
                    Encoding::Brotli | Encoding::Gzip | Encoding::Deflate => {
                        vec.push(encoding);
                    }
                    _ => (),
                }
            }
            // Sort by quality value, than by encoding type.
            vec.sort_unstable_by(sort_encoding);
            vec.pop() // Pop the last (largest).
        })
        .map(|encoding| encoding.item.to_owned())
        .unwrap_or(Encoding::Identity) // Default using identity encoding.
}

/// Compress data.
///
/// # Parameters
///
/// * `data` - Data to be compressed.
/// * `encoding` - Only support `Bortli`, `Gzip` and `Deflate`.
pub fn compress(data: &[u8], encoding: &Encoding) -> io::Result<Vec<u8>> {
    use std::io::prelude::*;
    let mut buf = Vec::new();
    match *encoding {
        Encoding::Brotli => {
            BufReader::new(brotli::CompressorReader::new(data, 4096, 6, 20))
                .read_to_end(&mut buf)?;
            Ok(buf)
        }
        Encoding::Gzip => {
            BufReader::new(GzEncoder::new(data, Compression::default()))
                .read_to_end(&mut buf)?;
            Ok(buf)
        }
        Encoding::Deflate => {
            BufReader::new(DeflateEncoder::new(data, Compression::default()))
                .read_to_end(&mut buf)?;
            Ok(buf)
        }
        _ => Err(io::Error::new(io::ErrorKind::Other, "Unsupported Encoding")),
    }
}

#[cfg(test)]
mod t_sort {
    use super::*;
    use hyper::header::{q, qitem};

    #[test]
    fn same_qualities() {
        let brotli = &&qitem(Encoding::Brotli);
        let gzip = &&qitem(Encoding::Gzip);
        let deflate = &&qitem(Encoding::Deflate);
        assert_eq!(sort_encoding(brotli, gzip), Ordering::Greater);
        assert_eq!(sort_encoding(brotli, deflate), Ordering::Greater);
        assert_eq!(sort_encoding(gzip, deflate), Ordering::Greater);
        assert_eq!(sort_encoding(gzip, brotli), Ordering::Less);
        assert_eq!(sort_encoding(deflate, brotli), Ordering::Less);
    }

    #[test]
    fn second_item_with_greater_quality() {
        let a = QualityItem { item: Encoding::Brotli, quality: q(500) };
        let b = qitem(Encoding::Gzip);
        assert_eq!(sort_encoding(&&a, &&b), Ordering::Less);
    }
}


#[cfg(test)]
mod t_prior {
    use super::*;
    use hyper::Method;
    use hyper::header::{q, qitem};

    #[test]
    fn no_accept_encoding_header() {
        let req = Request::new(Method::Get, "localhost".parse().unwrap());
        assert_eq!(get_prior_encoding(&req), Encoding::Identity);
    }

    #[test]
    fn with_unsupported_encoding() {
        let mut req = Request::new(Method::Get, "localhost".parse().unwrap());
        // Empty encoding
        let accept_encoding = AcceptEncoding(vec![]);
        req.headers_mut().set(accept_encoding);
        assert_eq!(get_prior_encoding(&req), Encoding::Identity);

        // Deprecated encoding.
        let accept_encoding = AcceptEncoding(vec![qitem(Encoding::Compress)]);
        req.headers_mut().set(accept_encoding);
        assert_eq!(get_prior_encoding(&req), Encoding::Identity);
    }

    #[test]
    fn pick_brotli() {
        let mut req = Request::new(Method::Get, "localhost".parse().unwrap());
        let accept_encoding = AcceptEncoding(vec![
            qitem(Encoding::Deflate),
            qitem(Encoding::Brotli),
            qitem(Encoding::Gzip),
        ]);
        req.headers_mut().set(accept_encoding);
        assert_eq!(get_prior_encoding(&req), Encoding::Brotli);
    }

    #[test]
    fn filter_out_zero_quality() {
        let mut req = Request::new(Method::Get, "localhost".parse().unwrap());
        let accept_encoding = AcceptEncoding(vec![
            QualityItem { item: Encoding::Brotli, quality: q(0) }
        ]);
        req.headers_mut().set(accept_encoding);
        assert_eq!(get_prior_encoding(&req), Encoding::Identity);
    }
}

#[cfg(test)]
mod t_compress {
    use super::*;

    #[test]
    fn failed() {
        let error = compress(b"hello", &Encoding::Identity).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::Other);
    }
}
