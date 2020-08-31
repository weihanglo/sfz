// Copyright (c) 2018 Weihang Lo
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::cmp::Ordering;
use std::io::{self, BufReader};

use flate2::read::{DeflateEncoder, GzEncoder};
use flate2::Compression;
use hyper::header::HeaderValue;

const IDENTITY: &str = "identity";
const DEFLATE: &str = "deflate";
const GZIP: &str = "gzip";
const BR: &str = "br";

/// Inner helper type to store quality values.
///
/// - 0: content enconding
/// - 1: weight from 0 to 1000
#[derive(Debug, PartialEq)]
struct QualityValue<'a>(&'a str, u32);

/// Inner helper type for comparsion by intrinsic enum variant order.
#[derive(Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Encoding {
    Identity,
    Deflate,
    Gzip,
    Brotli,
}

impl From<&str> for Encoding {
    fn from(s: &str) -> Self {
        match s {
            DEFLATE => Self::Deflate,
            GZIP => Self::Gzip,
            BR => Self::Brotli,
            _ => Self::Identity,
        }
    }
}

/// This match expression is necessary to return a `&'static str`.
fn encoding_to_static_str<'a>(encoding: &'a str) -> &'static str {
    match encoding {
        DEFLATE => DEFLATE,
        GZIP => GZIP,
        BR => BR,
        _ => IDENTITY,
    }
}

/// Sorting encodings according to the weight of quality values and then the
/// intrinsic rank of `Encoding` enum varaint.
///
/// The function only accecpt Brotli, Gzip and Deflate encodings, passing other
/// encodings in may lead to a unexpected result.
fn sort_encoding(a: &QualityValue, b: &QualityValue) -> Ordering {
    a.1.cmp(&b.1)
        .then_with(|| Encoding::from(a.0).cmp(&Encoding::from(b.0)))
}

/// According to RFC7231, a [Quality Values][1] is defined as follow grammar:
///
/// ```text
/// weight = OWS ";" OWS "q=" qvalue
/// qvalue = ( "0" [ "." 0*3DIGIT ] )
///        / ( "1" [ "." 0*3("0") ] )
/// ```
///
/// Note that:
///
/// - Quality value of 0 means unacceptable.
/// - The weight ranges from 0 to 1 in real number with three digit at most.
/// - Weight defaults to 1 if not present.
/// - We define unrecognized qvalue as zero.
///
/// [1]: https://tools.ietf.org/html/rfc7231#section-5.3.1
fn parse_qvalue(q: &str) -> Option<QualityValue> {
    let mut iter = q.trim().split_terminator(';').take(2);
    let content = iter.next().map(str::trim_end)?;
    let weight = match iter.next() {
        Some(s) => s
            .trim_start()
            .trim_start_matches("q=")
            .parse::<f32>()
            .ok()
            .map(|num| (num * 1000.0) as u32)
            .filter(|v| *v <= 1000)
            .unwrap_or_default(),
        None => 1000,
    };
    Some(QualityValue(content, weight))
}

/// Get prior encoding from `Accept-Encoding` header field.
///
/// Note that:
///
/// - Only accept `br` / `gzip` / `deflate`
/// - Highest non-zero qvalue is preferred.
pub fn get_prior_encoding<'a>(accept_encoding: &'a HeaderValue) -> &'static str {
    accept_encoding
        .to_str()
        .ok()
        .and_then(|accept_encoding| {
            let mut quality_values = accept_encoding
                .split(',')
                .filter_map(parse_qvalue)
                .collect::<Vec<_>>();
            // Sort by quality value, than by encoding type.
            quality_values.sort_unstable_by(sort_encoding);
            // Get the last encoding (highest priority).
            quality_values.last().map(|q| encoding_to_static_str(q.0))
        })
        // Default using identity encoding, which means no content encoding.
        .unwrap_or(IDENTITY)
}

/// Compress data.
///
/// # Parameters
///
/// * `data` - Data to be compressed.
/// * `encoding` - Only support `br`, `gzip`, `deflate` and `identity`.
pub fn compress(data: &[u8], encoding: &str) -> io::Result<Vec<u8>> {
    use std::io::prelude::*;
    let mut buf = Vec::new();
    match encoding {
        BR => {
            BufReader::new(brotli::CompressorReader::new(data, 4096, 6, 20))
                .read_to_end(&mut buf)?;
        }
        GZIP => {
            BufReader::new(GzEncoder::new(data, Compression::default())).read_to_end(&mut buf)?;
        }
        DEFLATE => {
            BufReader::new(DeflateEncoder::new(data, Compression::default()))
                .read_to_end(&mut buf)?;
        }
        _ => return Err(io::Error::new(io::ErrorKind::Other, "Unsupported Encoding")),
    };
    Ok(buf)
}

#[cfg(test)]
mod t_parse_qvalue {
    use super::*;

    #[test]
    fn parse_successfully() {
        let cases = vec![
            (Some(QualityValue(BR, 1000)), "br;q=1"),
            (Some(QualityValue(BR, 0)), "br;q=0"),
            (Some(QualityValue(BR, 1000)), "br;q=1.000"),
            (Some(QualityValue(BR, 0)), "br;q=0.000"),
            (Some(QualityValue(BR, 1000)), "br"),
            (Some(QualityValue(BR, 1000)), "br;"),
            (Some(QualityValue(BR, 0)), "br;1234asd"),
            (Some(QualityValue(BR, 500)), "       br    ;   q=0.5    "),
            (Some(QualityValue("*", 1000)), "*"),
            (Some(QualityValue("*", 300)), "*;q=0.3"),
            (Some(QualityValue("q=123", 1000)), "q=123"),
            (None, ""),
        ];
        for case in cases {
            let res = parse_qvalue(case.1);
            assert_eq!(res, case.0, "failed on case: {:?}", case);
        }
    }
}

#[cfg(test)]
mod t_sort {
    use super::*;

    #[test]
    fn same_qualities() {
        let brotli = &QualityValue(BR, 1000);
        let gzip = &QualityValue(GZIP, 1000);
        let deflate = &QualityValue(DEFLATE, 1000);
        assert_eq!(sort_encoding(brotli, gzip), Ordering::Greater);
        assert_eq!(sort_encoding(brotli, deflate), Ordering::Greater);
        assert_eq!(sort_encoding(gzip, deflate), Ordering::Greater);
        assert_eq!(sort_encoding(gzip, brotli), Ordering::Less);
        assert_eq!(sort_encoding(deflate, brotli), Ordering::Less);
    }

    #[test]
    fn second_item_with_greater_quality() {
        let a = &QualityValue(BR, 500);
        let b = &QualityValue(DEFLATE, 1000);
        assert_eq!(sort_encoding(a, b), Ordering::Less);
    }
}

#[cfg(test)]
mod t_prior {
    use super::*;
    use hyper::header::HeaderValue;

    #[test]
    fn with_unsupported_encoding() {
        // Empty encoding
        let accept_encoding = HeaderValue::from_static("");
        let encoding = get_prior_encoding(&accept_encoding);
        assert_eq!(encoding, IDENTITY);

        // Deprecated encoding.
        let accept_encoding = HeaderValue::from_static("compress");
        let encoding = get_prior_encoding(&accept_encoding);
        assert_eq!(encoding, IDENTITY);
    }

    #[test]
    fn pick_highest_priority() {
        let cases = vec![
            (BR, "br,gzip,deflate"),
            (BR, "gzip,br,deflate"),
            (BR, "deflate,gzip,br"),
            (BR, "br;q=0.8,gzip;q=0.5,deflate;q=0.2"),
            (GZIP, "br;q=0.5,gzip,deflate;q=0.8"),
        ];
        for case in cases {
            let accept_encoding = HeaderValue::from_static(case.1);
            let encoding = get_prior_encoding(&accept_encoding);
            assert_eq!(encoding, case.0, "failed on case: {:?}", case);
        }
    }

    #[test]
    fn filter_out_zero_quality() {
        let accept_encoding = HeaderValue::from_static("brotli;q=0,gzip;q=0,deflate");
        let encoding = get_prior_encoding(&accept_encoding);
        assert_eq!(encoding, DEFLATE);
    }
}

#[cfg(test)]
mod t_compress {
    use super::*;

    #[test]
    fn failed() {
        let error = compress(b"hello", "unrecognized").unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::Other);
    }

    #[test]
    fn compressed() {
        let buf = compress(b"xxxxx", DEFLATE);
        assert!(!buf.unwrap().is_empty());
        let buf = compress(b"xxxxx", GZIP);
        assert!(!buf.unwrap().is_empty());
        let buf = compress(b"xxxxx", BR);
        assert!(!buf.unwrap().is_empty());
    }
}
