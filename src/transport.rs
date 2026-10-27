//! The HTTP layer: one GET, no redirect following (the client follows
//! redirects itself, so it can drop the key on another host).

use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use crate::error::ConnectionErrorKind;
use crate::meta::Headers;

/// A response body over this size is an error, never an unbounded allocation.
pub const MAX_RESPONSE_BYTES: usize = 64 << 20;

/// One request: a GET of `url` with `headers`.
#[derive(Debug, Clone)]
pub(crate) struct HttpRequest {
    pub(crate) url: String,
    pub(crate) headers: Vec<(&'static str, String)>,
}

/// One response, with its whole (bounded) body.
#[derive(Debug, Clone)]
pub(crate) struct HttpResponse {
    pub(crate) status: u16,
    pub(crate) headers: Headers,
    pub(crate) body: Vec<u8>,
}

/// Why a request got no response.
#[derive(Debug)]
pub(crate) struct TransportError {
    pub(crate) kind: ConnectionErrorKind,
    pub(crate) message: String,
    pub(crate) source: Option<Box<dyn std::error::Error + Send + Sync>>,
}

pub(crate) type SendFuture<'a> =
    Pin<Box<dyn Future<Output = Result<HttpResponse, TransportError>> + Send + 'a>>;

/// Sends one GET. The production transport is [`ReqwestTransport`]; the
/// tests use a scripted one.
pub(crate) trait Transport: Send + Sync + std::fmt::Debug {
    fn send(&self, request: HttpRequest) -> SendFuture<'_>;
}

/// The production transport: reqwest over rustls, with TLS verification
/// always on and redirects left to the client.
#[derive(Debug)]
pub(crate) struct ReqwestTransport {
    client: reqwest::Client,
}

impl ReqwestTransport {
    pub(crate) fn new(timeout: Duration) -> Result<Self, TransportError> {
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(timeout)
            .build()
            .map_err(|e| TransportError {
                kind: ConnectionErrorKind::Other,
                message: format!("could not build the HTTP client: {e}"),
                source: Some(Box::new(e)),
            })?;
        Ok(Self { client })
    }
}

fn classify(e: &reqwest::Error) -> ConnectionErrorKind {
    if e.is_timeout() {
        return ConnectionErrorKind::Timeout;
    }
    // A failed certificate check surfaces as a rustls error in the source
    // chain; it is never retried (BEHAVIOR 9).
    let mut source: Option<&(dyn std::error::Error + 'static)> = Some(e);
    while let Some(err) = source {
        let text = err.to_string().to_ascii_lowercase();
        if text.contains("certificate") || text.contains("invalidcertificate") {
            return ConnectionErrorKind::Tls;
        }
        source = err.source();
    }
    ConnectionErrorKind::Other
}

fn transport_error(e: reqwest::Error) -> TransportError {
    TransportError {
        kind: classify(&e),
        message: e.to_string(),
        source: Some(Box::new(e)),
    }
}

impl Transport for ReqwestTransport {
    fn send(&self, request: HttpRequest) -> SendFuture<'_> {
        Box::pin(async move {
            let mut builder = self.client.get(&request.url);
            for (name, value) in &request.headers {
                builder = builder.header(*name, value);
            }
            let mut response = builder.send().await.map_err(transport_error)?;
            let status = response.status().as_u16();
            let headers = Headers::new(
                response
                    .headers()
                    .iter()
                    .filter_map(|(k, v)| Some((k.as_str().to_owned(), v.to_str().ok()?.to_owned())))
                    .collect(),
            );
            let too_large = || TransportError {
                kind: ConnectionErrorKind::BodyTooLarge,
                message: format!("the response body exceeds {MAX_RESPONSE_BYTES} bytes"),
                source: None,
            };
            if response
                .content_length()
                .is_some_and(|n| n > MAX_RESPONSE_BYTES as u64)
            {
                return Err(too_large());
            }
            let mut body = Vec::new();
            while let Some(chunk) = response.chunk().await.map_err(transport_error)? {
                if body.len() + chunk.len() > MAX_RESPONSE_BYTES {
                    return Err(too_large());
                }
                body.extend_from_slice(&chunk);
            }
            Ok(HttpResponse {
                status,
                headers,
                body,
            })
        })
    }
}
