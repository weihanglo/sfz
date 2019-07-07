// Copyright (c) 2018 Weihang Lo
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::time::SystemTime;

use hyper::header::{ETag, IfMatch, IfModifiedSince, IfNoneMatch, IfUnmodifiedSince, LastModified};
use hyper::server::Request;
use hyper::Method;

use crate::extensions::SystemTimeExt;

/// Validate precondition of `If-Match` header.
///
/// Note that an origin server MUST use the strong comparison function when
/// comparing entity-tags for `If-Match`.
///
/// [RFC7232: If-Match](https://tools.ietf.org/html/rfc7232#section-3.1)
fn check_if_match(etag: &ETag, if_match: &IfMatch) -> bool {
    match *if_match {
        IfMatch::Any => true,
        IfMatch::Items(ref tags) => tags.iter().any(|tag| tag.strong_eq(etag)),
    }
}

/// Validate precondition of `If-None-Match` header.
///
/// Note that a recipient MUST use the weak comparison function when comparing
/// entity-tags for `If-None-Match`.
///
/// [RFC7232: If-None-Match](https://tools.ietf.org/html/rfc7232#section-3.2)
fn check_if_none_match(etag: &ETag, if_none_match: &IfNoneMatch) -> bool {
    match *if_none_match {
        IfNoneMatch::Any => false,
        IfNoneMatch::Items(ref tags) => tags.iter().all(|tag| tag.weak_ne(etag)),
    }
}

/// Validate precondition of `If-Unmodified-Since` header.
fn check_if_unmodified_since(
    last_modified: &LastModified,
    if_unmodified_since: &IfUnmodifiedSince,
) -> bool {
    let IfUnmodifiedSince(since) = *if_unmodified_since;
    let LastModified(modified) = *last_modified;
    // Convert to seconds to omit subsecs precision.
    let modified: SystemTime = modified.into();
    let since: SystemTime = since.into();
    modified.timestamp() <= since.timestamp()
}

/// Validate precondition of `If-Modified-Since` header.
fn check_if_modified_since(
    last_modified: &LastModified,
    if_modified_since: &IfModifiedSince,
) -> bool {
    let IfModifiedSince(since) = *if_modified_since;
    !check_if_unmodified_since(last_modified, &IfUnmodifiedSince(since))
}

fn is_method_get_head(method: &Method) -> bool {
    match *method {
        Method::Get | Method::Head => true,
        _ => false,
    }
}

/// Indicates that conditions given in the request header evaluted to false.
/// Return true if any preconditions fail.
///
/// Note that this method is only implemented partial precedence of
/// conditions defined in [RFC7232][1] which is only related to precondition
/// (Status Code 412) but not caching response (Status Code 304). Caller must
/// handle caching responses by themselves.
///
/// [1]: https://tools.ietf.org/html/rfc7232#section-6
pub fn is_precondition_failed(req: &Request, etag: &ETag, last_modified: &LastModified) -> bool {
    // 1. Evaluate If-Match
    if let Some(if_match) = req.headers().get::<IfMatch>() {
        if check_if_match(etag, if_match) {
            // 3. Evaluate If-None-Match
            if let Some(_) = req.headers().get::<IfNoneMatch>() {
                if !is_method_get_head(req.method()) {
                    return true;
                }
            }
        } else {
            return true;
        }
    }

    // 2. Evaluate If-Unmodified-Since
    if let Some(if_unmodified_since) = req.headers().get::<IfUnmodifiedSince>() {
        if check_if_unmodified_since(last_modified, if_unmodified_since) {
            // 3. Evaluate If-None-Match
            if let Some(_) = req.headers().get::<IfNoneMatch>() {
                if !is_method_get_head(req.method()) {
                    return true;
                }
            }
        } else {
            return true;
        }
    }

    // 3. Evaluate If-None-Match
    if let Some(_) = req.headers().get::<IfNoneMatch>() {
        if !is_method_get_head(req.method()) {
            return true;
        }
    }

    false
}

/// Determine freshness of requested resource by validate `If-None-Match`
/// and `If-Modified-Since` precondition header fields containing validators.
///
/// See more on [RFC7234, 4.3.2. Handling a Received Validation Request][1].
///
/// [1]: https://tools.ietf.org/html/rfc7234#section-4.3.2
pub fn is_fresh(req: &Request, etag: &ETag, last_modified: &LastModified) -> bool {
    // `If-None-Match` takes presedence over `If-Modified-Since`.
    if let Some(if_none_match) = req.headers().get::<IfNoneMatch>() {
        !check_if_none_match(etag, &if_none_match)
    } else if let Some(since) = req.headers().get::<IfModifiedSince>() {
        !check_if_modified_since(last_modified, &since)
    } else {
        false
    }
}

#[cfg(test)]
mod t {
    use super::*;
    use hyper::header::EntityTag;
    use std::time::Duration;

    mod match_none_match {
        use super::*;

        #[test]
        fn any() {
            let etag = ETag(EntityTag::strong("".to_owned()));
            assert!(check_if_match(&etag, &IfMatch::Any));
            assert!(!check_if_none_match(&etag, &IfNoneMatch::Any));
        }

        #[test]
        fn one() {
            let etag = ETag(EntityTag::strong("2".to_owned()));
            let tags = vec![
                EntityTag::strong("0".to_owned()),
                EntityTag::strong("1".to_owned()),
                EntityTag::strong("2".to_owned()),
            ];
            let if_match = IfMatch::Items(tags.to_owned());
            assert!(check_if_match(&etag, &if_match));
            let if_none_match = IfNoneMatch::Items(tags.to_owned());
            assert!(!check_if_none_match(&etag, &if_none_match));
        }

        #[test]
        fn none() {
            let etag = ETag(EntityTag::strong("1".to_owned()));
            let tags = vec![EntityTag::strong("0".to_owned())];
            let if_match = IfMatch::Items(tags.to_owned());
            assert!(!check_if_match(&etag, &if_match));
            let if_none_match = IfNoneMatch::Items(tags.to_owned());
            assert!(check_if_none_match(&etag, &if_none_match));
        }
    }

    mod modified_unmodified_since {
        use super::*;

        fn init_since() -> (SystemTime, LastModified) {
            let now = SystemTime::now();
            (now, LastModified(now.into()))
        }

        #[test]
        fn now() {
            let (now, last_modified) = init_since();
            assert!(!check_if_modified_since(
                &last_modified,
                &IfModifiedSince(now.into()),
            ));
            assert!(check_if_unmodified_since(
                &last_modified,
                &IfUnmodifiedSince(now.into()),
            ));
        }

        #[test]
        fn after_one_sec() {
            let (now, last_modified) = init_since();
            let modified = now + Duration::from_secs(1);
            assert!(!check_if_modified_since(
                &last_modified,
                &IfModifiedSince(modified.into()),
            ));
            assert!(check_if_unmodified_since(
                &last_modified,
                &IfUnmodifiedSince(modified.into()),
            ));
        }

        #[test]
        fn one_sec_ago() {
            let (now, last_modified) = init_since();
            let modified = now - Duration::from_secs(1);
            assert!(check_if_modified_since(
                &last_modified,
                &IfModifiedSince(modified.into()),
            ));
            assert!(!check_if_unmodified_since(
                &last_modified,
                &IfUnmodifiedSince(modified.into()),
            ));
        }
    }

    fn init_request() -> (Request, EntityTag, SystemTime) {
        (
            Request::new(Method::Get, "localhost".parse().unwrap()),
            EntityTag::strong("hello".to_owned()),
            SystemTime::now(),
        )
    }

    mod fresh {
        use super::*;

        #[test]
        fn no_precondition_header_fields() {
            let (req, etag, date) = init_request();
            assert!(!is_fresh(&req, &ETag(etag), &LastModified(date.into())));
        }

        #[test]
        fn if_none_match_precedes_if_modified_since() {
            let (mut req, etag, date) = init_request();
            let if_none_match = IfNoneMatch::Items(vec![etag.to_owned()]);
            let future = date + Duration::from_secs(1);
            let if_modified_since = IfModifiedSince(future.into());
            req.headers_mut().set(if_none_match);
            req.headers_mut().set(if_modified_since);
            assert!(is_fresh(&req, &ETag(etag), &LastModified(date.into())));
        }
    }

    mod precondition {
        use super::*;

        #[test]
        fn ok_without_any_precondition() {
            let (req, etag, date) = init_request();
            assert!(!is_precondition_failed(
                &req,
                &ETag(etag),
                &LastModified(date.into())
            ));
        }

        #[test]
        fn failed_with_if_match_not_passes() {
            let (mut req, etag, date) = init_request();
            let if_match = IfMatch::Items(vec![EntityTag::strong("".to_owned())]);
            req.headers_mut().set(if_match);
            assert!(is_precondition_failed(
                &req,
                &ETag(etag),
                &LastModified(date.into())
            ));
        }

        #[test]
        fn with_if_match_passes() {
            let (mut req, etag, date) = init_request();
            let if_match = IfMatch::Items(vec![EntityTag::strong("hello".to_owned())]);
            let if_none_match = IfNoneMatch::Items(vec![EntityTag::strong("world".to_owned())]);
            req.headers_mut().set(if_match);
            req.headers_mut().set(if_none_match);
            // OK with GET HEAD methods
            assert!(!is_precondition_failed(
                &req,
                &ETag(etag.to_owned()),
                &LastModified(date.into())
            ));
            // Failed with method other than GET HEAD
            req.set_method(Method::Post);
            assert!(is_precondition_failed(
                &req,
                &ETag(etag.to_owned()),
                &LastModified(date.into())
            ));
        }

        #[test]
        fn failed_with_if_unmodified_since_not_passes() {
            let (mut req, etag, date) = init_request();
            let past = date - Duration::from_secs(1);
            let if_unmodified_since = IfUnmodifiedSince(past.into());
            req.headers_mut().set(if_unmodified_since);
            assert!(is_precondition_failed(
                &req,
                &ETag(etag),
                &LastModified(date.into())
            ));
        }

        #[test]
        fn with_if_unmodified_since_passes() {
            let (mut req, etag, date) = init_request();
            let if_unmodified_since = IfUnmodifiedSince(date.into());
            let if_none_match = IfNoneMatch::Items(vec![EntityTag::strong("nonematch".to_owned())]);
            req.headers_mut().set(if_unmodified_since);
            req.headers_mut().set(if_none_match);
            // OK with GET HEAD methods
            assert!(!is_precondition_failed(
                &req,
                &ETag(etag.to_owned()),
                &LastModified(date.into())
            ));
            // Failed with method other than GET HEAD
            req.set_method(Method::Post);
            assert!(is_precondition_failed(
                &req,
                &ETag(etag.to_owned()),
                &LastModified(date.into())
            ));
        }
    }
}
