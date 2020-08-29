// Copyright (c) 2018 Weihang Lo
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use headers::{ContentRange, ETag, HeaderMapExt, IfRange, LastModified, Range};

use crate::server::Request;

/// Check if given value from `If-Range` header field is fresh.
///
/// According to RFC7232, to validate `If-Range` header, the implementation
/// must use a strong comparison.
pub fn is_range_fresh(req: &Request, etag: &ETag, last_modified: &LastModified) -> bool {
    // Ignore `If-Range` if `Range` header is not present.
    if req.headers().typed_get::<Range>().is_none() {
        return false;
    }

    req.headers()
        .typed_get::<IfRange>()
        .map(|if_range| !if_range.is_modified(Some(etag), Some(last_modified)))
        // Always be fresh if there is no validators
        .unwrap_or(true)
}

/// Convert `Range` header field in incoming request to `Content-Range` header
/// field for response.
///
/// Here are all situations mapped to returning `Option`:
///
/// - None byte-range -> None
/// - One satisfiable byte-range -> Some
/// - One not satisfiable byte-range -> None
/// - Two or more byte-ranges -> None
/// - bytes-units are not in "bytes" -> None
///
/// A satisfiable byte range must conform to following criteria:
///
/// - Invalid if the last-byte-pos is present and less than the first-byte-pos.
/// - First-byte-pos must be less than complete length of the representation.
/// - If suffix-byte-range-spec is present, it must not be zero.
pub fn is_satisfiable_range(range: &Range, complete_length: u64) -> Option<ContentRange> {
    use core::ops::Bound::{Included, Unbounded};
    let mut iter = range.iter();
    let bounds = iter.next();

    if iter.next().is_some() {
        // Found multiple byte-range-spec. Drop.
        return None;
    }

    bounds.and_then(|b| match b {
        (Included(start), Included(end)) if start <= end && start < complete_length => {
            ContentRange::bytes(
                start..=end.min(complete_length.saturating_sub(1)),
                complete_length,
            )
            .ok()
        }
        (Included(start), Unbounded) if start < complete_length => {
            ContentRange::bytes(start.., complete_length).ok()
        }
        (Unbounded, Included(end)) if end > 0 => {
            ContentRange::bytes(complete_length.saturating_sub(end).., complete_length).ok()
        }
        _ => None,
    })
}

#[cfg(test)]
mod t_range {
    use super::*;
    use std::time::{Duration, SystemTime};

    #[test]
    fn no_range_header() {
        // Ignore range freshness validation. Return ture.
        let req = &mut Request::default();
        let last_modified = &LastModified::from(SystemTime::now());
        let etag = &"\"strong\"".to_string().parse::<ETag>().unwrap();
        let if_range = IfRange::etag(etag.clone());
        req.headers_mut().typed_insert(if_range);
        assert!(!is_range_fresh(req, etag, last_modified));
    }

    #[test]
    fn no_if_range_header() {
        // Ignore if-range freshness validation. Return ture.
        let req = &mut Request::default();
        req.headers_mut().typed_insert(Range::bytes(0..).unwrap());
        let last_modified = &LastModified::from(SystemTime::now());
        let etag = &"\"strong\"".to_string().parse::<ETag>().unwrap();
        // Always be fresh if there is no validators
        assert!(is_range_fresh(req, etag, last_modified));
    }

    #[test]
    fn weak_validator_as_falsy() {
        let req = &mut Request::default();
        req.headers_mut().typed_insert(Range::bytes(0..).unwrap());

        let last_modified = &LastModified::from(SystemTime::now());
        let etag = &"W/\"weak\"".to_string().parse::<ETag>().unwrap();
        let if_range = IfRange::etag(etag.clone());
        req.headers_mut().typed_insert(if_range);
        assert!(!is_range_fresh(req, etag, last_modified));
    }

    #[test]
    fn only_accept_exact_match_mtime() {
        let req = &mut Request::default();
        let etag = &"\"strong\"".to_string().parse::<ETag>().unwrap();
        let date = SystemTime::now();
        let last_modified = &LastModified::from(date);
        req.headers_mut().typed_insert(Range::bytes(0..).unwrap());

        // Same date.
        req.headers_mut().typed_insert(IfRange::date(date));
        assert!(is_range_fresh(req, etag, last_modified));

        // Before 10 sec.
        let past = date - Duration::from_secs(10);
        req.headers_mut().typed_insert(IfRange::date(past));
        assert!(!is_range_fresh(req, etag, last_modified));

        // After 10 sec.
        //
        // TODO: Uncomment the assertion after someone fixes the issue.
        //
        // [RFC7233: 3.2. If-Range][1] describe that `If-Range` validation must
        // comparison by exact match. However, the [current implementation][2]
        // is doing it wrong!
        //
        // [1]: https://tools.ietf.org/html/rfc7233#section-3.2
        // [2]: https://github.com/hyperium/headers/blob/2e8c12b/src/common/if_range.rs#L66
        let future = date + Duration::from_secs(10);
        req.headers_mut().typed_insert(IfRange::date(future));
        // assert!(!is_range_fresh(req, etag, last_modified));
    }

    #[test]
    fn strong_validator() {
        let req = &mut Request::default();
        req.headers_mut().typed_insert(Range::bytes(0..).unwrap());

        let last_modified = &LastModified::from(SystemTime::now());
        let etag = &"\"strong\"".to_string().parse::<ETag>().unwrap();
        let if_range = IfRange::etag(etag.clone());
        req.headers_mut().typed_insert(if_range);
        assert!(is_range_fresh(req, etag, last_modified));
    }
}

#[cfg(test)]
mod t_satisfiable {
    use super::*;

    #[test]
    fn zero_byte_range() {
        let range = &Range::bytes(1..1).unwrap();
        assert!(is_satisfiable_range(range, 10).is_none());
    }

    #[test]
    fn one_satisfiable_byte_range() {
        let range = &Range::bytes(4..=6).unwrap();
        let complete_length = 10;
        let content_range = is_satisfiable_range(range, complete_length);
        assert_eq!(
            content_range,
            ContentRange::bytes(4..7, complete_length).ok()
        );

        // only first-byte-pos and retrieve to the end
        let range = &Range::bytes(3..).unwrap();
        let complete_length = 10;
        let content_range = is_satisfiable_range(range, complete_length);
        assert_eq!(
            content_range,
            ContentRange::bytes(3..10, complete_length).ok()
        );

        // last-byte-pos exceeds complete length
        let range = &Range::bytes(7..20).unwrap();
        let complete_length = 10;
        let content_range = is_satisfiable_range(range, complete_length);
        assert_eq!(
            content_range,
            ContentRange::bytes(7..10, complete_length).ok()
        );

        // suffix-byte-range-spec
        let range = &Range::bytes(..=3).unwrap();
        let complete_length = 10;
        let content_range = is_satisfiable_range(range, complete_length);
        assert_eq!(
            content_range,
            ContentRange::bytes(7..10, complete_length).ok()
        );

        // suffix-byte-range-spec greater than complete length
        let range = &Range::bytes(..20).unwrap();
        let complete_length = 10;
        let content_range = is_satisfiable_range(range, complete_length);
        assert_eq!(
            content_range,
            ContentRange::bytes(0..10, complete_length).ok()
        );
    }

    #[test]
    fn one_unsatisfiable_byte_range() {
        // First-byte-pos is greater than complete length.
        let range = &Range::bytes(20..).unwrap();
        assert!(is_satisfiable_range(range, 10).is_none());

        // Last-bypte-pos is less than first-byte-pos
        let range = &Range::bytes(5..3).unwrap();
        assert!(is_satisfiable_range(range, 10).is_none());

        // suffix-byte-range-spec must be non-zero
        let mut headers = headers::HeaderMap::new();
        headers.insert(
            hyper::header::RANGE,
            headers::HeaderValue::from_static("bytes=-0"),
        );
        let range = &headers.typed_get::<Range>().unwrap();
        assert!(is_satisfiable_range(range, 10).is_none());
    }

    #[test]
    fn multiple_byte_ranges() {
        let mut headers = headers::HeaderMap::new();
        headers.insert(
            hyper::header::RANGE,
            headers::HeaderValue::from_static("bytes=0-1,30-40"),
        );
        let range = &headers.typed_get::<Range>().unwrap();
        assert!(is_satisfiable_range(range, 10).is_none());
    }
}
