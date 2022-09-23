// Copyright (c) 2018 Weihang Lo
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

mod res;
mod send;
mod serve;

use crate::http::loggable::LoggableBody;

pub type Request = hyper::Request<hyper::Body>;
pub type Response = hyper::Response<LoggableBody<hyper::Body>>;

pub use self::serve::{serve, PathType};
