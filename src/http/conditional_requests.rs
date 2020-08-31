// Copyright (c) 2018 Weihang Lo
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::time::SystemTime;

use headers::{ETag, HeaderMapExt, IfMatch, IfModifiedSince, IfNoneMatch, IfUnmodifiedSince};
use hyper::Method;

use crate::server::Request;

/// Indicates that conditions given in the request header evaluted to false.
/// Return true if any preconditions fail.
///
/// Note that this method is only implemented partial precedence of
/// conditions defined in [RFC7232][1] which is only related to precondition
/// (Status Code 412) but not caching response (Status Code 304). Caller must
/// handle caching responses by themselves.
///
/// [1]: https://tools.ietf.org/html/rfc7232#section-6
pub fn is_precondition_failed(req: &Request, etag: &ETag, last_modified: SystemTime) -> bool {
    // 3. Evaluate If-None-Match
    let eval_if_none_match = || {
        req.headers().typed_get::<IfNoneMatch>().is_some()
            && req.method() != Method::GET
            && req.method() != Method::HEAD
    };

    // 1. Evaluate If-Match
    let eval_if_match = req
        .headers()
        .typed_get::<IfMatch>()
        .map(|if_match| !if_match.precondition_passes(etag) || eval_if_none_match());

    // 2. Evaluate If-Unmodified-Since
    let eval_if_unmodified_since = || {
        req.headers()
            .typed_get::<IfUnmodifiedSince>()
            .map(|if_unmodified_since| {
                !if_unmodified_since.precondition_passes(last_modified) || eval_if_none_match()
            })
    };

    eval_if_match
        .or_else(eval_if_unmodified_since)
        .or_else(|| Some(eval_if_none_match()))
        .unwrap_or_default()
}

/// Determine freshness of requested resource by validate `If-None-Match`
/// and `If-Modified-Sinlsece` precondition header fields containing validators.
///
/// See more on [RFC7234, 4.3.2. Handling a Received Validation Request][1].
///
/// [1]: https://tools.ietf.org/html/rfc7234#section-4.3.2
pub fn is_fresh(req: &Request, etag: &ETag, last_modified: SystemTime) -> bool {
    // `If-None-Match` takes presedence over `If-Modified-Since`.
    if let Some(if_none_match) = req.headers().typed_get::<IfNoneMatch>() {
        !if_none_match.precondition_passes(etag)
    } else if let Some(if_modified_since) = req.headers().typed_get::<IfModifiedSince>() {
        !if_modified_since.is_modified(last_modified)
    } else {
        false
    }
}

#[cfg(test)]
fn init_request() -> (Request, ETag, SystemTime) {
    (
        Request::default(),
        "\"hello\"".to_string().parse::<ETag>().unwrap(),
        SystemTime::now(),
    )
}

#[cfg(test)]
mod t_precondition {
    use super::*;
    use std::time::Duration;

    #[test]
    fn ok_without_any_precondition() {
        let (req, etag, date) = init_request();
        assert!(!is_precondition_failed(&req, &etag, date));
    }

    #[test]
    fn failed_with_if_match_not_passes() {
        let (mut req, etag, date) = init_request();
        let if_match = IfMatch::from("\"\"".to_string().parse::<ETag>().unwrap());
        req.headers_mut().typed_insert(if_match);
        assert!(is_precondition_failed(&req, &etag, date));
    }

    #[test]
    fn with_if_match_passes() {
        let (mut req, etag, date) = init_request();
        let if_match = IfMatch::from("\"hello\"".to_string().parse::<ETag>().unwrap());
        let if_none_match = IfNoneMatch::from("\"world\"".to_string().parse::<ETag>().unwrap());
        req.headers_mut().typed_insert(if_match);
        req.headers_mut().typed_insert(if_none_match);
        // OK with GET HEAD methods
        assert!(!is_precondition_failed(&req, &etag, date));
        // Failed with method other than GET HEAD
        *req.method_mut() = Method::PUT;
        assert!(is_precondition_failed(&req, &etag, date));
    }

    #[test]
    fn failed_with_if_unmodified_since_not_passes() {
        let (mut req, etag, date) = init_request();
        let past = date - Duration::from_secs(1);
        let if_unmodified_since = IfUnmodifiedSince::from(past);
        req.headers_mut().typed_insert(if_unmodified_since);
        assert!(is_precondition_failed(&req, &etag, date));
    }

    #[test]
    fn with_if_unmodified_since_passes() {
        let (mut req, etag, date) = init_request();
        let if_unmodified_since = IfUnmodifiedSince::from(date);
        let if_none_match = IfNoneMatch::from("\"nonematch\"".to_string().parse::<ETag>().unwrap());
        req.headers_mut().typed_insert(if_unmodified_since);
        req.headers_mut().typed_insert(if_none_match);
        // OK with GET HEAD methods
        assert!(!is_precondition_failed(&req, &etag, date));
        // Failed with method other than GET HEAD
        *req.method_mut() = Method::PUT;
        assert!(is_precondition_failed(&req, &etag, date));
    }
}

#[cfg(test)]
mod t_fresh {
    use super::*;
    use std::time::Duration;

    #[test]
    fn no_precondition_header_fields() {
        let (req, etag, date) = init_request();
        assert!(!is_fresh(&req, &etag, date));
    }

    #[test]
    fn if_none_match_precedes_if_modified_since() {
        let (mut req, etag, date) = init_request();
        let if_none_match = IfNoneMatch::from(etag.clone());
        let future = date + Duration::from_secs(1);
        let if_modified_since = IfModifiedSince::from(future);
        req.headers_mut().typed_insert(if_none_match);
        req.headers_mut().typed_insert(if_modified_since);
        assert!(is_fresh(&req, &etag, date));
    }

    #[test]
    fn only_if_modified_since() {
        let (mut req, etag, date) = init_request();
        let future = date + Duration::from_secs(1);
        let if_modified_since = IfModifiedSince::from(future);
        req.headers_mut().typed_insert(if_modified_since);
        assert!(is_fresh(&req, &etag, date));
    }
}
