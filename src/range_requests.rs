// Copyright (c) 2018 Weihang Lo. All rights reserved.
//
// See the LICENSE file at the top-level directory of this distribution.

use hyper::server::Request;
use hyper::header::{
    ContentRange,
    ContentRangeSpec,
    ETag,
    IfRange,
    LastModified,
    Range,
};

/// Check if given value from `If-Range` header field is fresh.
///
/// According to RFC7232, to validate `If-Range` header, the implementation
/// must use a strong comparison.
pub fn is_range_fresh(
    req: &Request,
    etag: &ETag,
    last_modified: &LastModified
) -> bool {
    // Ignore `If-Range` if `Range` header is not present.
    if !req.headers().has::<Range>() {
        return false
    }
    if let Some(if_range) = req.headers().get::<IfRange>() {
        return match *if_range {
            IfRange::EntityTag(ref tag) => tag.strong_eq(etag),
            IfRange::Date(date) => {
                let LastModified(modified) = *last_modified;
                // Exact match
                modified == date
            }
        }
    }
    // Always be fresh if there is no validators
    true
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
///
/// Note that invalid and multiple byte-range are treaded as an unsatisfiable
/// range.
pub fn is_satisfiable_range(
    range: &Range,
    instance_length: u64,
) -> Option<ContentRange> {
    match *range {
        // Try to extract byte range specs from range-unit.
        Range::Bytes(ref byte_range_specs) => Some(byte_range_specs),
        _ => None,
    }
        .and_then(|specs| if specs.len() == 1 {
            Some(specs[0].to_owned())
        } else {
            None
        })
        .and_then(|spec| spec.to_satisfiable_range(instance_length))
        .and_then(|range| Some(ContentRange(ContentRangeSpec::Bytes {
            range: Some(range),
            instance_length: Some(instance_length),
        })))
}

/// Extract range from `ContentRange` header field.
pub fn extract_range(content_range: &ContentRange) -> Option<(u64, u64)> {
    let ContentRange(ref range_spec) = *content_range;
    match *range_spec {
        ContentRangeSpec::Bytes { range, .. } => range,
        _ => None,
    }
}

#[cfg(test)]
mod t_range {
    use super::*;
    use hyper::Method;
    use hyper::header::EntityTag;
    use std::time::{SystemTime, Duration};

    #[test]
    fn no_range_header() {
        // Ignore range freshness validation. Return ture.
        let mut req = Request::new(Method::Get, "localhost".parse().unwrap());
        let last_modified = LastModified(SystemTime::now().into());
        let etag = EntityTag::strong("".to_owned());
        let if_range = IfRange::EntityTag(etag.to_owned());
        req.headers_mut().set(if_range);
        assert!(!is_range_fresh(&req, &ETag(etag), &last_modified));
    }

    #[test]
    fn no_if_range_header() {
        // Ignore range freshness validation. Return ture.
        let mut req = Request::new(Method::Get, "localhost".parse().unwrap());
        req.headers_mut().set(Range::Bytes(vec![]));
        let last_modified = LastModified(SystemTime::now().into());
        let etag = EntityTag::strong("".to_owned());
        assert!(!is_range_fresh(&req, &ETag(etag), &last_modified));
    }

    #[test]
    fn weak_validator_as_falsy() {
        let mut req = Request::new(Method::Get, "localhost".parse().unwrap());
        req.headers_mut().set(Range::Bytes(vec![]));

        let last_modified = LastModified(SystemTime::now().into());
        let etag = EntityTag::weak("im_weak".to_owned());
        let if_range = IfRange::EntityTag(etag.to_owned());
        req.headers_mut().set(if_range);
        assert!(!is_range_fresh(&req, &ETag(etag), &last_modified));
    }

    #[test]
    fn only_accept_exact_match_mtime() {
        let mut req = Request::new(Method::Get, "localhost".parse().unwrap());
        let etag = &ETag(EntityTag::strong("".to_owned()));
        let date = SystemTime::now();
        let last_modified = &LastModified(date.into());
        req.headers_mut().set(Range::Bytes(vec![]));

        // Same date.
        req.headers_mut().set(IfRange::Date(date.into()));
        assert!(is_range_fresh(&req, etag, last_modified));

        // Before 10 sec.
        let past = date - Duration::from_secs(10);
        req.headers_mut().set(IfRange::Date(past.into()));
        assert!(!is_range_fresh(&req, etag, last_modified));

        // After 10 sec.
        let future = date + Duration::from_secs(10);
        req.headers_mut().set(IfRange::Date(future.into()));
        assert!(!is_range_fresh(&req, etag, last_modified));
    }

    #[test]
    fn strong_validator() {
        let mut req = Request::new(Method::Get, "localhost".parse().unwrap());
        req.headers_mut().set(Range::Bytes(vec![]));

        let last_modified = LastModified(SystemTime::now().into());
        let etag = EntityTag::strong("im_strong".to_owned());
        let if_range = IfRange::EntityTag(etag.to_owned());
        req.headers_mut().set(if_range);
        assert!(is_range_fresh(&req, &ETag(etag), &last_modified));
    }
}

#[cfg(test)]
mod t_satisfiable {
    use super::*;

    #[test]
    fn zero_byte_range() {
        let range = &Range::Unregistered("".to_owned(), "".to_owned());
        assert!(is_satisfiable_range(range, 10).is_none());
    }

    #[test]
    fn one_satisfiable_byte_range() {
        let range = &Range::bytes(0, 10);
        assert!(is_satisfiable_range(range, 10).is_some());
    }

    #[test]
    fn one_unsatisfiable_byte_range() {
        let range = &Range::bytes(20, 10);
        assert!(is_satisfiable_range(range, 10).is_none());
    }

    #[test]
    fn multiple_byte_ranges() {
        let range = &Range::bytes_multi(vec![(0, 5), (5, 6)]);
        assert!(is_satisfiable_range(range, 10).is_none());
    }
}
