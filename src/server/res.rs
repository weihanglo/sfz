// Copyright (c) 2018 Weihang Lo
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Response factory functions.
//!

use hyper::StatusCode;
use hyper::header::ContentLength;
use hyper::server::Response;

/// Generate 304 NotModified response.
pub fn not_modified(res: Response) -> Response {
    res.with_status(StatusCode::NotModified)
}

/// Generate 403 Forbidden response.
pub fn forbidden(res: Response) -> Response {
    let body = "403 Forbidden";
    res.with_status(StatusCode::Forbidden)
        .with_header(ContentLength(body.len() as u64))
        .with_body(body)
}

/// Generate 404 NotFound response.
pub fn not_found(res: Response) -> Response {
    let body = "404 Not Found";
    res.with_status(StatusCode::NotFound)
        .with_header(ContentLength(body.len() as u64))
        .with_body(body)
}

/// Generate 412 PreconditionFailed response.
pub fn precondition_failed(res: Response) -> Response {
    let body = "412 Precondition Failed";
    res.with_status(StatusCode::PreconditionFailed)
        .with_header(ContentLength(body.len() as u64))
        .with_body(body)
}

/// Generate 500 InternalServerError response.
pub fn internal_server_error(res: Response) -> Response {
    let body = "500 Internal Server Error";
    res.with_status(StatusCode::InternalServerError)
        .with_header(ContentLength(body.len() as u64))
        .with_body(body)
}

#[cfg(test)]
mod t {
    use super::*;

    #[test]
    fn response_304() {
        let res = not_modified(Response::new());
        assert_eq!(res.status(), StatusCode::NotModified);
        assert!(res.body().is_empty());
    }

    #[test]
    fn response_403() {
        let res = forbidden(Response::new());
        assert_eq!(res.status(), StatusCode::Forbidden);
        assert!(!res.body().is_empty());
    }

    #[test]
    fn response_404() {
        let res = not_found(Response::new());
        assert_eq!(res.status(), StatusCode::NotFound);
        assert!(!res.body().is_empty());
    }

    #[test]
    fn response_412() {
        let res = precondition_failed(Response::new());
        assert_eq!(res.status(), StatusCode::PreconditionFailed);
        assert!(!res.body().is_empty());
    }

    #[test]
    fn response_500() {
        let res = internal_server_error(Response::new());
        assert_eq!(res.status(), StatusCode::InternalServerError);
        assert!(!res.body().is_empty());
    }
}
