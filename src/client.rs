//! The client, its builder, and the request engine: canonical URLs, keyless
//! open routes, redirects, retries, the ETag cache and deprecation warnings.

use std::collections::HashSet;
use std::collections::hash_map::RandomState;
use std::fmt;
use std::hash::{BuildHasher, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::time::{Duration, SystemTime};

use serde::de::DeserializeOwned;
use url::Url;

use crate::cache::{Cache, CacheEntry, MemoryCache, cache_key, parse_cache_control};
use crate::error::{
    ApiError, ConnectionError, ConnectionErrorKind, Error, ErrorCode, NearestLocation, Result,
};
use crate::meta::{Headers, RateLimit, ResponseMeta, WithMeta};
use crate::query::{Query, escape};
use crate::transport::{HttpRequest, HttpResponse, ReqwestTransport, Transport, TransportError};
use crate::units::Units;

/// The API's address.
pub const DEFAULT_BASE_URL: &str = "https://api.oureawx.com";
/// The default bound on each attempt.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
/// The default number of retries (3 attempts in all).
pub const DEFAULT_MAX_RETRIES: u32 = 2;
/// The environment variable the API key is read from.
pub const ENV_API_KEY: &str = "OUREA_API_KEY";
/// The environment variable that overrides the base URL (for proxies and
/// local test servers).
pub const ENV_BASE_URL: &str = "OUREA_BASE_URL";

const MAX_REDIRECTS: usize = 5;
const MAX_RETRY_AFTER: Duration = Duration::from_secs(60);

/// The SDK's version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// An API operation: its OpenAPI `operationId` and whether it is an open
/// route (`security: []`), sent without the key by default (BEHAVIOR 3.2).
#[derive(Debug, Clone, Copy)]
pub(crate) struct Operation {
    pub(crate) id: &'static str,
    pub(crate) open: bool,
}

/// The API key, which never prints.
#[derive(Clone, PartialEq, Eq)]
struct ApiKey(String);

impl fmt::Debug for ApiKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[redacted]")
    }
}

/// A deprecation notice from the API (BEHAVIOR 11), reported once per
/// operation per process.
///
/// ```
/// let client = ensemble_weather::Client::builder()
///     .on_deprecation(|notice| eprintln!("upgrade soon: {notice}"))
///     .build()?;
/// # let _ = client;
/// # Ok::<(), ensemble_weather::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Deprecation {
    /// The deprecated operation, such as `"getLocationForecast"`.
    pub operation: &'static str,
    /// The `Deprecation` header, when present.
    pub deprecation: Option<String>,
    /// The `Sunset` header, when present.
    pub sunset: Option<String>,
    /// The `Link rel="deprecation"` target, when present.
    pub link: Option<String>,
}

impl fmt::Display for Deprecation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ensemble-weather: {} is deprecated", self.operation)?;
        if let Some(sunset) = &self.sunset {
            write!(f, " (sunset {sunset})")?;
        }
        if let Some(link) = &self.link {
            write!(f, "; see {link}")?;
        }
        Ok(())
    }
}

type DeprecationHandler = Arc<dyn Fn(&Deprecation) + Send + Sync>;

#[derive(Clone)]
pub(crate) struct Config {
    api_key: Option<ApiKey>,
    base: Url,
    base_str: String,
    pub(crate) units: Units,
    timeout: Duration,
    max_retries: u32,
    cache: Option<Arc<dyn Cache>>,
    key_on_open_routes: bool,
    user_agent: String,
    on_deprecation: DeprecationHandler,
    transport: Arc<dyn Transport>,
    /// A value in [0, 1] for full-jitter backoff (a test seam).
    pub(crate) jitter: Arc<dyn Fn() -> f64 + Send + Sync>,
}

/// The Ourea API client.
///
/// Cheap to clone (clones share the connection pool and the response cache);
/// create one and reuse it. Every call is async and runs on Tokio; enable the
/// `blocking` feature for [`blocking::Client`](crate::blocking::Client).
///
/// ```no_run
/// use ensemble_weather::{Client, Units};
///
/// # async fn run() -> Result<(), ensemble_weather::Error> {
/// // Anonymous (Free tier), or the key in OUREA_API_KEY:
/// let client = Client::new()?;
/// // Or configured:
/// let client = Client::builder().api_key("ow_...").units(Units::Imperial).build()?;
/// let forecast = client.forecast("or/mt-hood-meadows").await?;
/// # let _ = forecast;
/// # Ok(()) }
/// ```
#[derive(Clone)]
pub struct Client {
    pub(crate) config: Arc<Config>,
}

impl fmt::Debug for Client {
    /// Describes the client without its API key.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Client")
            .field("base_url", &self.config.base_str)
            .field(
                "api_key",
                &self.config.api_key.as_ref().map(|_| "[redacted]"),
            )
            .field("units", &self.config.units)
            .finish_non_exhaustive()
    }
}

enum KeySetting {
    FromEnv,
    Key(String),
    Anonymous,
}

enum CacheSetting {
    Default,
    Custom(Arc<dyn Cache>),
    Off,
}

/// Configures a [`Client`]. Start with [`Client::builder`].
///
/// ```
/// use std::time::Duration;
/// use ensemble_weather::{Client, Units};
///
/// let client = Client::builder()
///     .api_key("ow_...")
///     .units(Units::Imperial)
///     .timeout(Duration::from_secs(10))
///     .max_retries(4)
///     .user_agent("my-app/1.2")
///     .build()?;
/// # let _ = client;
/// # Ok::<(), ensemble_weather::Error>(())
/// ```
#[must_use]
pub struct ClientBuilder {
    api_key: KeySetting,
    base_url: Option<String>,
    units: Units,
    timeout: Duration,
    max_retries: u32,
    cache: CacheSetting,
    key_on_open_routes: bool,
    app_token: Option<String>,
    on_deprecation: Option<DeprecationHandler>,
    transport: Option<Arc<dyn Transport>>,
    jitter: Option<Arc<dyn Fn() -> f64 + Send + Sync>>,
}

impl fmt::Debug for ClientBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClientBuilder")
            .field("base_url", &self.base_url)
            .field(
                "api_key",
                &matches!(self.api_key, KeySetting::Key(_)).then_some("[redacted]"),
            )
            .field("units", &self.units)
            .finish_non_exhaustive()
    }
}

impl Default for ClientBuilder {
    fn default() -> Self {
        Self {
            api_key: KeySetting::FromEnv,
            base_url: None,
            units: Units::Metric,
            timeout: DEFAULT_TIMEOUT,
            max_retries: DEFAULT_MAX_RETRIES,
            cache: CacheSetting::Default,
            key_on_open_routes: false,
            app_token: None,
            on_deprecation: None,
            transport: None,
            jitter: None,
        }
    }
}

impl ClientBuilder {
    /// Sets the API key, sent as `X-API-Key`. Without it the client reads
    /// `OUREA_API_KEY`; with no key at all, calls are anonymous (the Free
    /// tier), which is valid.
    pub fn api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = KeySetting::Key(key.into());
        self
    }

    /// Makes every call anonymous, even when `OUREA_API_KEY` is set.
    pub fn anonymous(mut self) -> Self {
        self.api_key = KeySetting::Anonymous;
        self
    }

    /// Sets the API's base URL (default [`DEFAULT_BASE_URL`], or
    /// `OUREA_BASE_URL` when set). A trailing slash is ignored and a path
    /// prefix is kept: `https://proxy.example/ourea/` calls
    /// `https://proxy.example/ourea/api/v1/...`.
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = Some(url.into());
        self
    }

    /// Sets the unit system of every request (default [`Units::Metric`]).
    /// A request's own `units` overrides it.
    pub fn units(mut self, units: Units) -> Self {
        self.units = units;
        self
    }

    /// Bounds each attempt (default 30 s). The whole call is bounded by the
    /// attempts [`max_retries`](Self::max_retries) allows.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Sets how many times a retryable failure is retried (default 2, so 3
    /// attempts in all). 0 disables retrying.
    pub fn max_retries(mut self, retries: u32) -> Self {
        self.max_retries = retries;
        self
    }

    /// Replaces the response cache (default: a [`MemoryCache`] of 256 entries
    /// and 32 MiB).
    pub fn cache(mut self, cache: impl Cache + 'static) -> Self {
        self.cache = CacheSetting::Custom(Arc::new(cache));
        self
    }

    /// Turns the response cache off: no `If-None-Match` is ever sent.
    pub fn no_cache(mut self) -> Self {
        self.cache = CacheSetting::Off;
        self
    }

    /// Sends the API key on the routes that need no credentials too (search,
    /// catalog details, attribution).
    ///
    /// By default those are sent without the key so every caller shares one
    /// edge-cache entry; turn this on when every request must be attributed
    /// to the key.
    pub fn send_key_on_open_routes(mut self, yes: bool) -> Self {
        self.key_on_open_routes = yes;
        self
    }

    /// Appends your application's product token, such as `"my-app/1.2"`, to
    /// the SDK's User-Agent. It must not contain user data.
    pub fn user_agent(mut self, product: impl Into<String>) -> Self {
        self.app_token = Some(product.into().trim().to_owned());
        self
    }

    /// Receives deprecation notices (once per operation per process). The
    /// default writes one line to standard error.
    pub fn on_deprecation(
        mut self,
        handler: impl Fn(&Deprecation) + Send + Sync + 'static,
    ) -> Self {
        self.on_deprecation = Some(Arc::new(handler));
        self
    }

    #[cfg(test)]
    pub(crate) fn transport(mut self, transport: Arc<dyn Transport>) -> Self {
        self.transport = Some(transport);
        self
    }

    #[cfg(test)]
    pub(crate) fn jitter(mut self, jitter: impl Fn() -> f64 + Send + Sync + 'static) -> Self {
        self.jitter = Some(Arc::new(jitter));
        self
    }

    /// Builds the client.
    ///
    /// # Errors
    ///
    /// [`Error::Usage`] for an invalid base URL, or [`Error::Connection`]
    /// when the HTTP stack cannot be initialised.
    pub fn build(self) -> Result<Client> {
        self.build_with_env(&|name| std::env::var(name).ok())
    }

    /// Builds the client, reading the environment through `env`.
    pub(crate) fn build_with_env(self, env: &dyn Fn(&str) -> Option<String>) -> Result<Client> {
        let env = |name: &str| env(name).filter(|v| !v.trim().is_empty());
        let api_key = match self.api_key {
            KeySetting::Key(key) => Some(key),
            KeySetting::Anonymous => None,
            KeySetting::FromEnv => env(ENV_API_KEY),
        }
        .filter(|k| !k.is_empty())
        .map(ApiKey);
        let base_raw = self
            .base_url
            .or_else(|| env(ENV_BASE_URL))
            .unwrap_or_else(|| DEFAULT_BASE_URL.to_owned());
        let (base, base_str) = parse_base_url(&base_raw)?;
        let transport: Arc<dyn Transport> = match self.transport {
            Some(t) => t,
            None => Arc::new(
                ReqwestTransport::new(self.timeout).map_err(|e| connection_error("client", e))?,
            ),
        };
        let cache = match self.cache {
            CacheSetting::Default => Some(Arc::new(MemoryCache::default()) as Arc<dyn Cache>),
            CacheSetting::Custom(cache) => Some(cache),
            CacheSetting::Off => None,
        };
        let mut user_agent = format!(
            "ensemble-weather-rust/{VERSION} (rustc/{})",
            env!("ENSEMBLE_WEATHER_RUSTC_VERSION")
        );
        if let Some(app) = self.app_token.filter(|a| !a.is_empty()) {
            user_agent.push(' ');
            user_agent.push_str(&app);
        }
        Ok(Client {
            config: Arc::new(Config {
                api_key,
                base,
                base_str,
                units: self.units,
                timeout: self.timeout,
                max_retries: self.max_retries,
                cache,
                key_on_open_routes: self.key_on_open_routes,
                user_agent,
                on_deprecation: self
                    .on_deprecation
                    .unwrap_or_else(|| Arc::new(|d| eprintln!("warning: {d}"))),
                transport,
                jitter: self.jitter.unwrap_or_else(|| Arc::new(random_unit)),
            }),
        })
    }
}

fn parse_base_url(raw: &str) -> Result<(Url, String)> {
    let trimmed = raw.trim().trim_end_matches('/');
    let invalid = |why: &str| Error::usage("client", format!("invalid base URL {raw:?}: {why}"));
    let url = Url::parse(trimmed).map_err(|e| invalid(&e.to_string()))?;
    if !matches!(url.scheme(), "https" | "http") || url.host_str().is_none() {
        return Err(invalid("want http(s)://host[/prefix]"));
    }
    if url.query().is_some()
        || url.fragment().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(invalid("no query, fragment or credentials allowed"));
    }
    let base_str = url.as_str().trim_end_matches('/').to_owned();
    Ok((url, base_str))
}

/// A random value in [0, 1), from the standard library's randomly keyed hasher.
fn random_unit() -> f64 {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let mut hasher = RandomState::new().build_hasher();
    hasher.write_u64(COUNTER.fetch_add(1, Ordering::Relaxed));
    #[allow(clippy::cast_precision_loss)] // 53 bits fit an f64 exactly
    let unit = (hasher.finish() >> 11) as f64 / (1u64 << 53) as f64;
    unit
}

fn connection_error(operation: &'static str, e: TransportError) -> Error {
    Error::Connection(Box::new(ConnectionError {
        operation,
        kind: e.kind,
        message: e.message,
        source: e.source,
    }))
}

/// The operations already warned about, so each is reported once per process.
fn warned() -> &'static Mutex<HashSet<&'static str>> {
    static WARNED: OnceLock<Mutex<HashSet<&'static str>>> = OnceLock::new();
    WARNED.get_or_init(|| Mutex::new(HashSet::new()))
}

/// The target of a `Link` with `rel="deprecation"`.
fn deprecation_link(headers: &Headers) -> Option<String> {
    for value in headers.get_all("Link") {
        for link in value.split(',') {
            let mut parts = link.split(';');
            let target = parts.next()?.trim();
            let is_deprecation = parts.any(|p| {
                let (name, val) = p.trim().split_once('=').unwrap_or((p.trim(), ""));
                name.eq_ignore_ascii_case("rel")
                    && val.trim_matches('"').eq_ignore_ascii_case("deprecation")
            });
            if is_deprecation {
                return Some(
                    target
                        .trim_start_matches('<')
                        .trim_end_matches('>')
                        .to_owned(),
                );
            }
        }
    }
    None
}

/// Reads `Retry-After`: delta-seconds or an IMF-fixdate HTTP-date.
fn parse_retry_after(value: &str, now: SystemTime) -> Option<Duration> {
    let value = value.trim();
    if let Ok(secs) = value.parse::<u64>() {
        return Some(Duration::from_secs(secs));
    }
    let at = parse_http_date(value)?;
    Some(at.duration_since(now).unwrap_or(Duration::ZERO))
}

/// Parses an IMF-fixdate such as `Wed, 01 Dec 2027 00:00:00 GMT`.
fn parse_http_date(s: &str) -> Option<SystemTime> {
    let (_, rest) = s.split_once(", ")?;
    let mut it = rest.split_whitespace();
    let day: i64 = it.next()?.parse().ok()?;
    let month = match it.next()? {
        "Jan" => 1,
        "Feb" => 2,
        "Mar" => 3,
        "Apr" => 4,
        "May" => 5,
        "Jun" => 6,
        "Jul" => 7,
        "Aug" => 8,
        "Sep" => 9,
        "Oct" => 10,
        "Nov" => 11,
        "Dec" => 12,
        _ => return None,
    };
    let year: i64 = it.next()?.parse().ok()?;
    let mut hms = it.next()?.split(':').map(str::parse::<i64>);
    let (h, m, sec) = (hms.next()?.ok()?, hms.next()?.ok()?, hms.next()?.ok()?);
    if it.next()? != "GMT" {
        return None;
    }
    // Days from the civil date (Howard Hinnant's algorithm).
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    let secs = u64::try_from(days * 86_400 + h * 3_600 + m * 60 + sec).ok()?;
    Some(SystemTime::UNIX_EPOCH + Duration::from_secs(secs))
}

#[derive(serde::Deserialize)]
struct ErrorBody {
    error: String,
    #[serde(default)]
    message: String,
    request_id: Option<String>,
    doc_url: Option<String>,
    param: Option<String>,
    nearest_location: Option<NearestLocation>,
}

fn is_redirect(status: u16) -> bool {
    matches!(status, 301 | 302 | 307 | 308)
}

fn same_origin(a: &Url, b: &Url) -> bool {
    a.scheme().eq_ignore_ascii_case(b.scheme())
        && a.host_str().map(str::to_ascii_lowercase) == b.host_str().map(str::to_ascii_lowercase)
        && a.port_or_known_default() == b.port_or_known_default()
}

impl Client {
    /// A client with the defaults: the key from `OUREA_API_KEY` (anonymous
    /// without it), metric units, the in-memory cache.
    ///
    /// # Errors
    ///
    /// As [`ClientBuilder::build`].
    ///
    /// ```
    /// let client = ensemble_weather::Client::new()?;
    /// # let _ = client;
    /// # Ok::<(), ensemble_weather::Error>(())
    /// ```
    pub fn new() -> Result<Self> {
        Self::builder().build()
    }

    /// A builder for a configured client.
    ///
    /// ```
    /// use ensemble_weather::{Client, Units};
    /// let client = Client::builder().units(Units::Imperial).build()?;
    /// # let _ = client;
    /// # Ok::<(), ensemble_weather::Error>(())
    /// ```
    pub fn builder() -> ClientBuilder {
        ClientBuilder::default()
    }

    /// A copy of this client that sends `key` instead, sharing this client's
    /// cache and connection pool: for a per-customer key.
    ///
    /// ```
    /// # let client = ensemble_weather::Client::new()?;
    /// let customer = client.with_api_key("ow_customer_key");
    /// # let _ = customer;
    /// # Ok::<(), ensemble_weather::Error>(())
    /// ```
    #[must_use]
    pub fn with_api_key(&self, key: impl Into<String>) -> Self {
        let mut config = (*self.config).clone();
        config.api_key = Some(ApiKey(key.into())).filter(|k| !k.0.is_empty());
        Self {
            config: Arc::new(config),
        }
    }

    /// Whether this client has an API key.
    ///
    /// ```
    /// let client = ensemble_weather::Client::builder().anonymous().build()?;
    /// assert!(!client.has_api_key());
    /// # Ok::<(), ensemble_weather::Error>(())
    /// ```
    #[must_use]
    pub fn has_api_key(&self) -> bool {
        self.config.api_key.is_some()
    }

    /// The unit system requests use unless they set their own.
    ///
    /// ```
    /// let client = ensemble_weather::Client::new()?;
    /// assert_eq!(client.units(), &ensemble_weather::Units::Metric);
    /// # Ok::<(), ensemble_weather::Error>(())
    /// ```
    #[must_use]
    pub fn units(&self) -> &Units {
        &self.config.units
    }

    pub(crate) fn build_url(&self, segments: &[&str], query: &Query) -> String {
        let mut url = self.config.base_str.clone();
        url.push_str("/api/v1");
        for segment in segments {
            url.push('/');
            url.push_str(&escape(segment, b""));
        }
        let qs = query.encode();
        if !qs.is_empty() {
            url.push('?');
            url.push_str(&qs);
        }
        url
    }

    /// Performs one operation and decodes its JSON body into `T`.
    pub(crate) async fn get<T: DeserializeOwned + WithMeta>(
        &self,
        op: Operation,
        segments: &[&str],
        query: &Query,
    ) -> Result<T> {
        let config = &self.config;
        let url = self.build_url(segments, query);
        let key = if op.open && !config.key_on_open_routes {
            None
        } else {
            config.api_key.as_ref()
        }
        .map(|k| k.0.as_str());
        let ck = cache_key(&url, key);
        let cached = config.cache.as_ref().and_then(|c| c.get(&ck));
        if let Some(entry) = &cached {
            let fresh = entry.max_age > Duration::ZERO
                && SystemTime::now()
                    .duration_since(entry.stored_at)
                    .is_ok_and(|age| age < entry.max_age);
            if fresh {
                let headers = Headers::new(entry.headers.clone());
                let meta = ResponseMeta {
                    rate_limit: RateLimit::from_headers(&headers),
                    headers,
                    ..Default::default()
                };
                return decode(op.id, &entry.body, meta);
            }
        }
        let etag = cached.as_ref().and_then(|e| e.etag.clone());

        let response = self.retrying(op.id, &url, key, etag.as_deref()).await?;
        self.warn_deprecation(op.id, &response.headers);
        let mut meta = ResponseMeta {
            rate_limit: RateLimit::from_headers(&response.headers),
            request_id: response.headers.get("X-Request-Id").map(str::to_owned),
            headers: response.headers.clone(),
            revalidated: false,
        };

        if response.status == 304 {
            let (Some(cache), Some(entry)) = (config.cache.as_ref(), cached) else {
                let mut e = ApiError::bare(
                    op.id,
                    304,
                    "304 Not Modified for a request the client has no copy of",
                );
                e.request_id = meta.request_id;
                return Err(Error::Unavailable(Box::new(e)));
            };
            let headers = Headers::new(entry.headers.clone()).merged(&response.headers);
            let cc = parse_cache_control(headers.get("Cache-Control"));
            let etag = response
                .headers
                .get("ETag")
                .map(str::to_owned)
                .or(entry.etag);
            cache.set(
                &ck,
                CacheEntry::new(
                    etag,
                    entry.body.clone(),
                    headers.pairs().to_vec(),
                    SystemTime::now(),
                    cc.max_age,
                ),
            );
            meta.headers = headers;
            meta.revalidated = true;
            return decode(op.id, &entry.body, meta);
        }

        if let Some(cache) = &config.cache {
            let cc = parse_cache_control(response.headers.get("Cache-Control"));
            let etag = response.headers.get("ETag").map(str::to_owned);
            if !cc.no_store && (etag.is_some() || cc.max_age > Duration::ZERO) {
                let entry = CacheEntry::new(
                    etag,
                    response.body.clone().into(),
                    response.headers.pairs().to_vec(),
                    SystemTime::now(),
                    cc.max_age,
                );
                cache.set(&ck, entry);
            } else if cached.is_some() {
                cache.remove(&ck);
            }
        }
        decode(op.id, &response.body, meta)
    }

    /// Sends the request, retrying connection errors (except TLS
    /// verification failures), 429, 502, 503 and 504 up to `max_retries`
    /// times. Returns the final 2xx or 304 response.
    async fn retrying(
        &self,
        op: &'static str,
        url: &str,
        key: Option<&str>,
        etag: Option<&str>,
    ) -> Result<HttpResponse> {
        let max = self.config.max_retries;
        let mut attempt = 0;
        loop {
            match self.follow(url, key, etag).await {
                Err(e) => {
                    let retryable = matches!(
                        e.kind,
                        ConnectionErrorKind::Timeout | ConnectionErrorKind::Other
                    );
                    if !retryable || attempt >= max {
                        return Err(connection_error(op, e));
                    }
                    tokio::time::sleep(self.backoff(attempt + 1)).await;
                }
                Ok(resp) if (200..300).contains(&resp.status) || resp.status == 304 => {
                    return Ok(resp);
                }
                Ok(resp) => {
                    let err = self.response_error(op, &resp, key);
                    if !matches!(resp.status, 429 | 502 | 503 | 504) || attempt >= max {
                        return Err(Error::from_api(err));
                    }
                    let mut delay = self.backoff(attempt + 1);
                    if let Some(retry_after) = err.retry_after {
                        if retry_after > MAX_RETRY_AFTER {
                            return Err(Error::from_api(err)); // not waited out; the caller decides
                        }
                        delay = retry_after;
                    }
                    tokio::time::sleep(delay).await;
                }
            }
            attempt += 1;
        }
    }

    /// Full jitter: a random delay in `[0, min(8 s, 0.5 s × 2^(n-1))]`.
    fn backoff(&self, n: u32) -> Duration {
        let ceiling = (0.5 * f64::from(1u32 << (n - 1).min(10))).min(8.0);
        Duration::from_secs_f64((self.config.jitter)().clamp(0.0, 1.0) * ceiling)
    }

    /// One attempt, following 301, 302, 307 and 308 (at most 5). The key is
    /// kept only while the target stays on the base URL's origin.
    async fn follow(
        &self,
        url: &str,
        key: Option<&str>,
        etag: Option<&str>,
    ) -> std::result::Result<HttpResponse, TransportError> {
        let attempt = async {
            let mut target = url.to_owned();
            let mut key = key;
            for _ in 0..=MAX_REDIRECTS {
                let mut headers = vec![
                    ("Accept", "application/json".to_owned()),
                    ("User-Agent", self.config.user_agent.clone()),
                ];
                if let Some(k) = key {
                    headers.push(("X-API-Key", k.to_owned()));
                }
                if let Some(tag) = etag {
                    headers.push(("If-None-Match", tag.to_owned()));
                }
                let response = self
                    .config
                    .transport
                    .send(HttpRequest {
                        url: target.clone(),
                        headers,
                    })
                    .await?;
                let location = response
                    .headers
                    .get("Location")
                    .filter(|_| is_redirect(response.status));
                let Some(location) = location else {
                    return Ok(response);
                };
                let bad = |why: String| TransportError {
                    kind: ConnectionErrorKind::Redirect,
                    message: why,
                    source: None,
                };
                let next = Url::parse(&target)
                    .and_then(|base| base.join(location))
                    .map_err(|e| bad(format!("invalid redirect Location: {e}")))?;
                if !same_origin(&next, &self.config.base) {
                    key = None;
                }
                target = next.to_string();
            }
            Err(TransportError {
                kind: ConnectionErrorKind::Redirect,
                message: format!("stopped after {MAX_REDIRECTS} redirects"),
                source: None,
            })
        };
        tokio::time::timeout(self.config.timeout, attempt)
            .await
            .unwrap_or_else(|_| {
                Err(TransportError {
                    kind: ConnectionErrorKind::Timeout,
                    message: format!("no response within {:?}", self.config.timeout),
                    source: None,
                })
            })
    }

    fn response_error(&self, op: &'static str, resp: &HttpResponse, key: Option<&str>) -> ApiError {
        let header_id = resp.headers.get("X-Request-Id").map(str::to_owned);
        let mut e = match serde_json::from_slice::<ErrorBody>(&resp.body) {
            Ok(body) if !body.error.is_empty() => ApiError {
                code: Some(ErrorCode::from(body.error)),
                message: body.message,
                request_id: body.request_id.or(header_id),
                doc_url: body.doc_url,
                param: body.param,
                nearest_location: body.nearest_location,
                ..ApiError::bare(op, resp.status, "")
            },
            _ => {
                let text = String::from_utf8_lossy(&resp.body);
                let text: String = text.trim().chars().take(200).collect();
                ApiError {
                    request_id: header_id,
                    ..ApiError::bare(op, resp.status, &text)
                }
            }
        };
        // Never echo a credential back, wherever it came from.
        for secret in [key, self.config.api_key.as_ref().map(|k| k.0.as_str())]
            .into_iter()
            .flatten()
        {
            e.message = e.message.replace(secret, "[redacted]");
        }
        // The API sends Retry-After on every 503 and 504 (BEHAVIOR 9).
        if matches!(resp.status, 429 | 503 | 504) {
            e.retry_after = resp
                .headers
                .get("Retry-After")
                .and_then(|v| parse_retry_after(v, SystemTime::now()));
        }
        e.rate_limit = RateLimit::from_headers(&resp.headers);
        e
    }

    fn warn_deprecation(&self, op: &'static str, headers: &Headers) {
        let deprecation = headers.get("Deprecation").map(str::to_owned);
        let sunset = headers.get("Sunset").map(str::to_owned);
        if deprecation.is_none() && sunset.is_none() {
            return;
        }
        if !warned()
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(op)
        {
            return;
        }
        let notice = Deprecation {
            operation: op,
            deprecation,
            sunset,
            link: deprecation_link(headers),
        };
        (self.config.on_deprecation)(&notice);
    }
}

impl ApiError {
    fn bare(operation: &'static str, status: u16, message: &str) -> Self {
        Self {
            operation,
            status,
            code: None,
            message: message.to_owned(),
            request_id: None,
            doc_url: None,
            param: None,
            nearest_location: None,
            retry_after: None,
            rate_limit: None,
        }
    }
}

fn decode<T: DeserializeOwned + WithMeta>(
    op: &'static str,
    body: &[u8],
    meta: ResponseMeta,
) -> Result<T> {
    match serde_json::from_slice::<T>(body) {
        Ok(mut value) => {
            value.set_meta(meta);
            Ok(value)
        }
        Err(e) => {
            let mut err = ApiError::bare(
                op,
                200,
                &format!("the response is not valid JSON for this operation: {e}"),
            );
            err.request_id = meta.request_id;
            Err(Error::Unavailable(Box::new(err)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_http_dates() {
        let t = parse_http_date("Wed, 01 Dec 2027 00:00:00 GMT").unwrap();
        assert_eq!(
            t.duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs(),
            1_827_619_200
        );
        assert!(parse_http_date("not a date").is_none());
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_827_619_190);
        assert_eq!(
            parse_retry_after("Wed, 01 Dec 2027 00:00:00 GMT", now),
            Some(Duration::from_secs(10))
        );
        assert_eq!(parse_retry_after("3", now), Some(Duration::from_secs(3)));
    }

    #[test]
    fn rejects_bad_base_urls() {
        for bad in ["ftp://x", "not a url", "https://x/?q=1", "https://u:p@x"] {
            assert!(parse_base_url(bad).is_err(), "{bad}");
        }
        assert_eq!(
            parse_base_url("https://proxy.example.test/ourea/")
                .unwrap()
                .1,
            "https://proxy.example.test/ourea"
        );
    }

    #[test]
    fn debug_never_prints_the_key() {
        let client = Client::builder()
            .api_key("ow_secret_value")
            .build()
            .unwrap();
        let text = format!(
            "{client:?} {:?}",
            Client::builder().api_key("ow_secret_value")
        );
        assert!(!text.contains("ow_secret_value"), "{text}");
    }

    #[test]
    fn jitter_is_in_unit_range() {
        for _ in 0..1000 {
            let v = random_unit();
            assert!((0.0..1.0).contains(&v));
        }
    }
}
