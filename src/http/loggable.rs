// Copyright (c) 2018 Weihang Lo
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::{
    net::{IpAddr, SocketAddr},
    pin::Pin,
    sync::atomic::{AtomicUsize, Ordering},
    task::{Context, Poll},
};

use chrono::Local;
use futures::ready;
use hyper::{body::HttpBody, Method, Uri, Version};

use crate::server::{Request, Response};

#[derive(Default)]
pub struct Log {
    pub remote_addr: Option<IpAddr>,
    pub method: Method,
    pub uri: Uri,
    pub status: u16,
    pub version: Version,
    pub user_agent: String,
}

impl Log {
    pub fn new(remote_addr: SocketAddr, req: &Request, res: &Response) -> Self {
        let user_agent = req
            .headers()
            .get(hyper::header::USER_AGENT)
            .map(|s| s.to_str().ok().unwrap_or_default())
            .unwrap_or("-");
        Self {
            remote_addr: Some(remote_addr.ip()),
            method: req.method().clone(),
            uri: req.uri().clone(),
            status: res.status().as_u16(),
            version: req.version(),
            user_agent: user_agent.to_string(),
        }
    }
}

#[derive(Default)]
pub struct LoggableBody<B> {
    pub inner: B,
    pub bytes_sent: AtomicUsize,
    pub content_length: Option<u64>,
    pub log: Option<Log>,
}

impl<'a, B> From<&'a str> for LoggableBody<B>
where
    B: HttpBody + From<&'a str>,
{
    fn from(s: &'a str) -> Self {
        Self {
            inner: s.into(),
            bytes_sent: Default::default(),
            content_length: None,
            log: None,
        }
    }
}

impl<B> LoggableBody<B>
where
    B: HttpBody,
{
    pub fn new(log: Option<Log>, inner: B) -> Self {
        Self {
            inner,
            bytes_sent: Default::default(),
            content_length: None,
            log,
        }
    }

    pub fn with_content_length(log: Option<Log>, inner: B, content_length: u64) -> Self {
        Self {
            inner,
            bytes_sent: Default::default(),
            content_length: Some(content_length),
            log,
        }
    }
}

impl<B> HttpBody for LoggableBody<B>
where
    B: HttpBody + Unpin,
    B::Data: std::fmt::Debug + bytes::Buf,
{
    type Data = B::Data;

    type Error = B::Error;

    fn poll_data(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Self::Data, Self::Error>>> {
        use bytes::Buf as _;

        let polled = ready!(Pin::new(&mut self.inner).poll_data(cx));
        if let Some(Ok(ref b)) = polled {
            self.bytes_sent.fetch_add(b.remaining(), Ordering::AcqRel);
        }

        let has_data = if polled.is_none() {
            false
        } else if let Some(l) = self.content_length {
            // If Content-Length header is set when body is not compressed,
            // hyper will not call poll_data to get None,
            // thus we cannot track when we should print the log message.
            // We need to track whether the entire body was sent.
            self.bytes_sent.load(Ordering::Acquire) < l as usize
        } else {
            true
        };

        if has_data {
            return Poll::Ready(polled);
        }

        if let Some(ref l) = self.log {
            let ip = match l.remote_addr {
                None => "-".to_string(),
                Some(ip) => ip.to_string(),
            };
            let local_time = Local::now().format("%d/%b/%Y %H:%M:%S %z");
            let method = &l.method;
            let uri = &l.uri;
            let version = l.version;
            let status = l.status;
            let bytes_sent = self.bytes_sent.load(Ordering::Acquire);
            let user_agent = &l.user_agent;
            println!(
                r#"{ip} - - [{local_time}] "{method} {uri} {version:?}" {status} {bytes_sent} "-" "{user_agent}" "-""#
            );
        }

        Poll::Ready(polled)
    }

    fn poll_trailers(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Option<headers::HeaderMap>, Self::Error>> {
        Pin::new(&mut self.inner).poll_trailers(cx)
    }
}
