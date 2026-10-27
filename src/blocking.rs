//! A synchronous client (the `blocking` feature).
//!
//! [`blocking::Client`](Client) runs the async [`Client`](crate::Client) on
//! its own single-threaded Tokio runtime, so it behaves identically: same
//! builders, same errors, same cache. Do not use it from inside an async
//! runtime; use the async client there.
//!
//! ```no_run
//! use ensemble_weather::blocking::Client;
//! use ensemble_weather::{Chance, Threshold, Variable};
//!
//! let client = Client::new()?;
//! let forecast = client
//!     .forecast("or/mt-hood-meadows")
//!     .chance(Chance::at_least(Variable::Snowfall, Threshold::inches(6.0)))
//!     .send()?;
//! println!("{}", forecast.location.name);
//! # Ok::<(), ensemble_weather::Error>(())
//! ```

use std::fmt;
use std::future::Future;
use std::sync::Arc;

use crate::error::{ConnectionError, ConnectionErrorKind, Error, Result};
use crate::paging;

/// The synchronous Ourea API client. Cheap to clone.
///
/// ```no_run
/// let client = ensemble_weather::blocking::Client::builder().units(ensemble_weather::Units::Imperial).build()?;
/// let alerts = client.alerts("or/mt-hood-meadows").send()?;
/// # let _ = alerts;
/// # Ok::<(), ensemble_weather::Error>(())
/// ```
#[derive(Clone)]
pub struct Client {
    pub(crate) inner: crate::Client,
    runtime: Arc<tokio::runtime::Runtime>,
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("blocking::Client")
            .field(&self.inner)
            .finish()
    }
}

/// Configures a [`blocking::Client`](Client): the same settings as
/// [`ClientBuilder`](crate::ClientBuilder).
///
/// ```
/// let client = ensemble_weather::blocking::Client::builder().anonymous().build()?;
/// # let _ = client;
/// # Ok::<(), ensemble_weather::Error>(())
/// ```
#[derive(Debug)]
#[must_use]
pub struct ClientBuilder(crate::ClientBuilder);

impl ClientBuilder {
    /// See [`crate::ClientBuilder::api_key`].
    pub fn api_key(self, key: impl Into<String>) -> Self {
        Self(self.0.api_key(key))
    }

    /// See [`crate::ClientBuilder::anonymous`].
    pub fn anonymous(self) -> Self {
        Self(self.0.anonymous())
    }

    /// See [`crate::ClientBuilder::base_url`].
    pub fn base_url(self, url: impl Into<String>) -> Self {
        Self(self.0.base_url(url))
    }

    /// See [`crate::ClientBuilder::units`].
    pub fn units(self, units: crate::Units) -> Self {
        Self(self.0.units(units))
    }

    /// See [`crate::ClientBuilder::timeout`].
    pub fn timeout(self, timeout: std::time::Duration) -> Self {
        Self(self.0.timeout(timeout))
    }

    /// See [`crate::ClientBuilder::max_retries`].
    pub fn max_retries(self, retries: u32) -> Self {
        Self(self.0.max_retries(retries))
    }

    /// See [`crate::ClientBuilder::cache`].
    pub fn cache(self, cache: impl crate::Cache + 'static) -> Self {
        Self(self.0.cache(cache))
    }

    /// See [`crate::ClientBuilder::no_cache`].
    pub fn no_cache(self) -> Self {
        Self(self.0.no_cache())
    }

    /// See [`crate::ClientBuilder::send_key_on_open_routes`].
    pub fn send_key_on_open_routes(self, yes: bool) -> Self {
        Self(self.0.send_key_on_open_routes(yes))
    }

    /// See [`crate::ClientBuilder::user_agent`].
    pub fn user_agent(self, product: impl Into<String>) -> Self {
        Self(self.0.user_agent(product))
    }

    /// See [`crate::ClientBuilder::on_deprecation`].
    pub fn on_deprecation(
        self,
        handler: impl Fn(&crate::Deprecation) + Send + Sync + 'static,
    ) -> Self {
        Self(self.0.on_deprecation(handler))
    }

    /// Builds the client and its runtime.
    ///
    /// # Errors
    ///
    /// As [`crate::ClientBuilder::build`], or [`Error::Connection`] when the
    /// runtime cannot start.
    pub fn build(self) -> Result<Client> {
        Client::from_async(self.0.build()?)
    }
}

impl Client {
    /// A client with the defaults (see [`crate::Client::new`]).
    ///
    /// # Errors
    ///
    /// As [`ClientBuilder::build`].
    pub fn new() -> Result<Self> {
        Self::builder().build()
    }

    /// A builder for a configured client.
    pub fn builder() -> ClientBuilder {
        ClientBuilder(crate::Client::builder())
    }

    /// Wraps an async client, sharing its cache and connection pool.
    ///
    /// # Errors
    ///
    /// [`Error::Connection`] when the runtime cannot start.
    pub fn from_async(inner: crate::Client) -> Result<Self> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| {
                Error::Connection(Box::new(ConnectionError {
                    operation: "client",
                    kind: ConnectionErrorKind::Other,
                    message: format!("could not start the runtime: {e}"),
                    source: Some(Box::new(e)),
                }))
            })?;
        Ok(Self {
            inner,
            runtime: Arc::new(runtime),
        })
    }

    /// A copy of this client that sends `key` instead (see
    /// [`crate::Client::with_api_key`]).
    #[must_use]
    pub fn with_api_key(&self, key: impl Into<String>) -> Self {
        Self {
            inner: self.inner.with_api_key(key),
            runtime: self.runtime.clone(),
        }
    }

    pub(crate) fn block_on<F: Future>(&self, future: F) -> F::Output {
        self.runtime.block_on(future)
    }
}

/// Every item of a paged list, fetched page by page as you iterate.
///
/// ```no_run
/// let client = ensemble_weather::blocking::Client::new()?;
/// for location in client.catalog_locations().items() {
///     println!("{}", location?.location.name);
/// }
/// # Ok::<(), ensemble_weather::Error>(())
/// ```
#[must_use = "an iterator does nothing until iterated"]
#[derive(Debug)]
pub struct Items<T> {
    client: Client,
    stream: paging::Items<T>,
}

impl<T> Items<T> {
    pub(crate) fn new(client: Client, stream: paging::Items<T>) -> Self {
        Self { client, stream }
    }
}

impl<T: Unpin> Iterator for Items<T> {
    type Item = Result<T>;

    fn next(&mut self) -> Option<Self::Item> {
        let stream = &mut self.stream;
        self.client.block_on(stream.try_next()).transpose()
    }
}
