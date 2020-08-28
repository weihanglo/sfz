// Copyright (c) 2018 Weihang Lo
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Response factory functions.
//!

use headers::{ContentLength, HeaderMapExt};
use hyper::StatusCode;

use crate::server::Response;

/// Generate 304 NotModified response.
pub fn not_modified(mut res: Response) -> Response {
    *res.status_mut() = StatusCode::NOT_MODIFIED;
    res
}

/// Generate 403 Forbidden response.
pub fn forbidden(res: Response) -> Response {
    prepare_response(res, StatusCode::FORBIDDEN, "403 Forbidden")
}

/// Generate 404 NotFound response.
pub fn not_found(res: Response) -> Response {
    prepare_response(res, StatusCode::NOT_FOUND, "404 Not Found")
}

/// Generate 412 PreconditionFailed response.
pub fn precondition_failed(res: Response) -> Response {
    prepare_response(
        res,
        StatusCode::PRECONDITION_FAILED,
        "412 Precondition Failed",
    )
}

/// Generate 500 InternalServerError response.
pub fn internal_server_error(res: Response) -> Response {
    prepare_response(
        res,
        StatusCode::INTERNAL_SERVER_ERROR,
        "500 Internal Server Error",
    )
}

fn prepare_response(mut res: Response, code: StatusCode, body: &'static str) -> Response {
    *res.status_mut() = code;
    *res.body_mut() = body.into();
    res.headers_mut()
        .typed_insert(ContentLength(body.len() as u64));
    res
}

#[cfg(test)]
mod t {
    use super::*;

    #[test]
    fn response_304() {
        let res = not_modified(Response::default());
        assert_eq!(res.status(), StatusCode::NOT_MODIFIED);
    }

    #[test]
    fn response_403() {
        let res = forbidden(Response::default());
        assert_eq!(res.status(), StatusCode::FORBIDDEN);
    }

    #[test]
    fn response_404() {
        let res = not_found(Response::default());
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn response_412() {
        let res = precondition_failed(Response::default());
        assert_eq!(res.status(), StatusCode::PRECONDITION_FAILED);
    }

    #[test]
    fn response_500() {
        let res = internal_server_error(Response::default());
        assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }
}
