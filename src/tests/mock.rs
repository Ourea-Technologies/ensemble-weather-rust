//! A scripted transport: answers requests from a queue and records them, with
//! the (paused) Tokio clock's time of each.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use tokio::time::Instant;

use crate::error::ConnectionErrorKind;
use crate::meta::Headers;
use crate::transport::{HttpRequest, HttpResponse, SendFuture, Transport, TransportError};

#[derive(Debug, Default)]
pub(crate) struct MockTransport {
    responses: Mutex<VecDeque<std::result::Result<HttpResponse, ConnectionErrorKind>>>,
    requests: Mutex<Vec<(Instant, HttpRequest)>>,
}

impl MockTransport {
    pub(crate) fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub(crate) fn push(&self, status: u16, headers: &[(&str, &str)], body: impl Into<Vec<u8>>) {
        let headers = Headers::new(
            headers
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                .collect(),
        );
        self.responses.lock().unwrap().push_back(Ok(HttpResponse {
            status,
            headers,
            body: body.into(),
        }));
    }

    pub(crate) fn push_json(&self, body: &str) {
        self.push(200, &[("Content-Type", "application/json")], body);
    }

    pub(crate) fn push_failure(&self, kind: ConnectionErrorKind) {
        self.responses.lock().unwrap().push_back(Err(kind));
    }

    pub(crate) fn requests(&self) -> Vec<(Instant, HttpRequest)> {
        self.requests.lock().unwrap().clone()
    }

    pub(crate) fn urls(&self) -> Vec<String> {
        self.requests().into_iter().map(|(_, r)| r.url).collect()
    }
}

impl HttpRequest {
    pub(crate) fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

impl Transport for MockTransport {
    fn send(&self, request: HttpRequest) -> SendFuture<'_> {
        self.requests
            .lock()
            .unwrap()
            .push((Instant::now(), request));
        let next = self.responses.lock().unwrap().pop_front();
        Box::pin(async move {
            match next {
                Some(Ok(response)) => Ok(response),
                Some(Err(kind)) => Err(TransportError {
                    kind,
                    message: format!("scripted {kind:?}"),
                    source: None,
                }),
                None => Err(TransportError {
                    kind: ConnectionErrorKind::Redirect, // never retried
                    message: "the test scripted no response for this request".to_owned(),
                    source: None,
                }),
            }
        })
    }
}
