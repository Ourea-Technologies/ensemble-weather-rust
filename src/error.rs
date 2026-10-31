//! The error type every call returns.

use std::fmt;
use std::time::Duration;

use crate::enums::string_enum;
use crate::meta::RateLimit;

/// A `Result` whose error is [`Error`].
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Everything that can go wrong with a call, one variant per category of
/// BEHAVIOR 8.
///
/// The API's own failures carry an [`ApiError`] with the HTTP status, the
/// API's [`ErrorCode`] (kept verbatim, even when unknown) and message, the
/// request ID, the code's docs link and, on a 429, 503 or 504, the
/// `Retry-After` delay. Errors never contain the API key.
///
/// ```
/// use ensemble_weather::{Category, Client, Error, ErrorCode, Point};
///
/// # async fn run(client: Client) {
/// match client.get_point_forecast(Point::new(45.0, 10.0)).await {
///     Ok(forecast) => println!("{}", forecast.location.name),
///     Err(Error::OutsideCoverage(e)) => println!("outside the forecast domain: {}", e.message()),
///     Err(Error::RateLimited(e)) => println!("retry after {:?}", e.retry_after()),
///     Err(e) if e.category() == Category::Permission => println!("needs a Pro key: {e}"),
///     Err(e) => eprintln!("{e}"),
/// }
/// # }
/// ```
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// The API key is missing or invalid (401).
    #[error("authentication failed: {0}")]
    Authentication(Box<ApiError>),
    /// The key's plan does not allow the request (403), such as `PRO_REQUIRED`.
    #[error("permission denied: {0}")]
    Permission(Box<ApiError>),
    /// The location or resource does not exist (404, 410).
    #[error("not found: {0}")]
    NotFound(Box<ApiError>),
    /// The server rejected the request's parameters (400).
    #[error("invalid request: {0}")]
    InvalidRequest(Box<ApiError>),
    /// The coordinates are outside the forecast domain (`OUTSIDE_COVERAGE`,
    /// a 404).
    #[error("outside coverage: {0}")]
    OutsideCoverage(Box<ApiError>),
    /// A rate limit or quota was exceeded (429). See [`ApiError::retry_after`].
    #[error("rate limited: {0}")]
    RateLimited(Box<ApiError>),
    /// The server could not answer (5xx), or its answer was unusable.
    #[error("unavailable: {0}")]
    Unavailable(Box<ApiError>),
    /// No response arrived: DNS, TLS, a reset or a timeout.
    #[error("connection failed: {0}")]
    Connection(#[source] Box<ConnectionError>),
    /// The SDK refused the call before sending it, such as an invalid
    /// [`Chance`](crate::Chance).
    #[error("invalid usage: {0}")]
    Usage(UsageError),
}

/// The category of an [`Error`], with BEHAVIOR 8's snake_case names.
///
/// ```
/// use ensemble_weather::Category;
/// assert_eq!(Category::NotFound.as_str(), "not_found");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Category {
    /// See [`Error::Authentication`].
    Authentication,
    /// See [`Error::Permission`].
    Permission,
    /// See [`Error::NotFound`].
    NotFound,
    /// See [`Error::InvalidRequest`].
    InvalidRequest,
    /// See [`Error::OutsideCoverage`].
    OutsideCoverage,
    /// See [`Error::RateLimited`].
    RateLimited,
    /// See [`Error::Unavailable`].
    Unavailable,
    /// See [`Error::Connection`].
    Connection,
    /// See [`Error::Usage`].
    Usage,
}

impl Category {
    /// The category's snake_case name, such as `"rate_limited"`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Authentication => "authentication",
            Self::Permission => "permission",
            Self::NotFound => "not_found",
            Self::InvalidRequest => "invalid_request",
            Self::OutsideCoverage => "outside_coverage",
            Self::RateLimited => "rate_limited",
            Self::Unavailable => "unavailable",
            Self::Connection => "connection",
            Self::Usage => "usage",
        }
    }
}

impl fmt::Display for Category {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Error {
    /// The error's category.
    ///
    /// ```
    /// # use ensemble_weather::{Category, Error};
    /// fn is_retryable_later(e: &Error) -> bool {
    ///     matches!(e.category(), Category::RateLimited | Category::Unavailable | Category::Connection)
    /// }
    /// ```
    #[must_use]
    pub fn category(&self) -> Category {
        match self {
            Self::Authentication(_) => Category::Authentication,
            Self::Permission(_) => Category::Permission,
            Self::NotFound(_) => Category::NotFound,
            Self::InvalidRequest(_) => Category::InvalidRequest,
            Self::OutsideCoverage(_) => Category::OutsideCoverage,
            Self::RateLimited(_) => Category::RateLimited,
            Self::Unavailable(_) => Category::Unavailable,
            Self::Connection(_) => Category::Connection,
            Self::Usage(_) => Category::Usage,
        }
    }

    /// The API's error details, when a response arrived.
    ///
    /// ```
    /// # use ensemble_weather::Error;
    /// fn request_id(e: &Error) -> Option<&str> {
    ///     e.api_error().and_then(|api| api.request_id())
    /// }
    /// ```
    #[must_use]
    pub fn api_error(&self) -> Option<&ApiError> {
        match self {
            Self::Authentication(e)
            | Self::Permission(e)
            | Self::NotFound(e)
            | Self::InvalidRequest(e)
            | Self::OutsideCoverage(e)
            | Self::RateLimited(e)
            | Self::Unavailable(e) => Some(e.as_ref()),
            Self::Connection(_) | Self::Usage(_) => None,
        }
    }

    /// The HTTP status, when a response arrived.
    ///
    /// ```
    /// # use ensemble_weather::Error;
    /// fn is_gone(e: &Error) -> bool { e.status() == Some(410) }
    /// ```
    #[must_use]
    pub fn status(&self) -> Option<u16> {
        self.api_error().map(ApiError::status)
    }

    /// The API's error code, when the response carried one.
    ///
    /// ```
    /// # use ensemble_weather::{Error, ErrorCode};
    /// fn needs_pro(e: &Error) -> bool { e.code() == Some(&ErrorCode::ProRequired) }
    /// ```
    #[must_use]
    pub fn code(&self) -> Option<&ErrorCode> {
        self.api_error().and_then(ApiError::code)
    }

    /// The server's `Retry-After` on a 429, 503 or 504.
    ///
    /// ```
    /// # use ensemble_weather::Error;
    /// # use std::time::Duration;
    /// fn wait(e: &Error) -> Duration { e.retry_after().unwrap_or(Duration::from_secs(1)) }
    /// ```
    #[must_use]
    pub fn retry_after(&self) -> Option<Duration> {
        self.api_error().and_then(ApiError::retry_after)
    }

    pub(crate) fn usage(operation: &'static str, message: impl Into<String>) -> Self {
        Self::Usage(UsageError {
            operation,
            message: message.into(),
        })
    }

    /// A 2xx response the SDK could not use (bad JSON, a missing answer).
    pub(crate) fn unanswered(operation: &'static str, message: impl Into<String>) -> Self {
        Self::Unavailable(Box::new(ApiError {
            operation,
            status: 200,
            code: None,
            message: message.into(),
            request_id: None,
            doc_url: None,
            param: None,
            nearest_location: None,
            retry_after: None,
            rate_limit: None,
        }))
    }

    pub(crate) fn from_api(error: ApiError) -> Self {
        let error = Box::new(error);
        let category =
            error
                .code
                .as_ref()
                .and_then(ErrorCode::category)
                .unwrap_or(match error.status {
                    401 => Category::Authentication,
                    403 => Category::Permission,
                    404 | 410 => Category::NotFound,
                    429 => Category::RateLimited,
                    s if s >= 500 => Category::Unavailable,
                    _ => Category::InvalidRequest,
                });
        match category {
            Category::Authentication => Self::Authentication(error),
            Category::Permission => Self::Permission(error),
            Category::NotFound => Self::NotFound(error),
            Category::OutsideCoverage => Self::OutsideCoverage(error),
            Category::RateLimited => Self::RateLimited(error),
            Category::Unavailable => Self::Unavailable(error),
            _ => Self::InvalidRequest(error),
        }
    }
}

/// An error response from the API.
#[derive(Debug, Clone)]
pub struct ApiError {
    pub(crate) operation: &'static str,
    pub(crate) status: u16,
    pub(crate) code: Option<ErrorCode>,
    pub(crate) message: String,
    pub(crate) request_id: Option<String>,
    pub(crate) doc_url: Option<String>,
    pub(crate) param: Option<String>,
    pub(crate) nearest_location: Option<NearestLocation>,
    pub(crate) retry_after: Option<Duration>,
    pub(crate) rate_limit: Option<RateLimit>,
}

impl ApiError {
    /// The API operation that failed, such as `"getLocationForecast"`.
    #[must_use]
    pub fn operation(&self) -> &str {
        self.operation
    }

    /// The HTTP status.
    #[must_use]
    pub fn status(&self) -> u16 {
        self.status
    }

    /// The API's error code (the body's `error` field), verbatim; `None` when
    /// the body was not a JSON error.
    #[must_use]
    pub fn code(&self) -> Option<&ErrorCode> {
        self.code.as_ref()
    }

    /// The human-readable explanation.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// The request's ID: the error body's `request_id`, else the response's
    /// `X-Request-Id`. Quote it to support.
    #[must_use]
    pub fn request_id(&self) -> Option<&str> {
        self.request_id.as_deref()
    }

    /// The code's explanation in the API docs (cause, fix, whether to retry),
    /// when the body links one.
    #[must_use]
    pub fn doc_url(&self) -> Option<&str> {
        self.doc_url.as_deref()
    }

    /// The query parameter a 400 is about, when it is about one (such as
    /// `"max_hours"`).
    #[must_use]
    pub fn param(&self) -> Option<&str> {
        self.param.as_deref()
    }

    /// On a `PRO_REQUIRED` from a coordinate route, the nearby named location
    /// that answers without a key; `None` when none qualifies.
    #[must_use]
    pub fn nearest_location(&self) -> Option<&NearestLocation> {
        self.nearest_location.as_ref()
    }

    /// The server's `Retry-After` on a 429, 503 or 504.
    #[must_use]
    pub fn retry_after(&self) -> Option<Duration> {
        self.retry_after
    }

    /// The response's rate-limit state, when it carried one.
    #[must_use]
    pub fn rate_limit(&self) -> Option<&RateLimit> {
        self.rate_limit.as_ref()
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: HTTP {}", self.operation, self.status)?;
        if let Some(code) = &self.code {
            write!(f, " {code}")?;
        }
        if !self.message.is_empty() {
            write!(f, ": {}", self.message)?;
        }
        if let Some(id) = &self.request_id {
            write!(f, " (request ID {id})")?;
        }
        Ok(())
    }
}

/// The named location a `PRO_REQUIRED` error suggests for a coordinate
/// request: within 10 km of the point and 300 m of its elevation.
#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
#[non_exhaustive]
pub struct NearestLocation {
    /// The location's slug.
    pub slug: String,
    /// The location route's path on this API, such as
    /// `"/api/v1/locations/or/mt-hood-meadows"` (with `/current` or `/alerts`
    /// for those routes).
    pub url: String,
    /// The distance from the point, km.
    pub distance_km: f64,
    /// The location's elevation, metres.
    pub elevation_m: f64,
}

/// A call refused before it was sent.
#[derive(Debug, Clone)]
pub struct UsageError {
    operation: &'static str,
    message: String,
}

impl UsageError {
    /// The operation that was refused.
    #[must_use]
    pub fn operation(&self) -> &str {
        self.operation
    }

    /// What was wrong.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for UsageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.operation, self.message)
    }
}

/// Why no response arrived.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ConnectionErrorKind {
    /// An attempt took longer than the client's timeout.
    Timeout,
    /// The server's TLS certificate failed verification (never retried).
    Tls,
    /// The response body exceeded 64 MiB (never retried).
    BodyTooLarge,
    /// More than 5 redirects, or an invalid one (never retried).
    Redirect,
    /// Anything else: DNS, a refused or reset connection.
    Other,
}

/// A failure before any response arrived.
#[derive(Debug, thiserror::Error)]
#[error("{operation}: {message}")]
pub struct ConnectionError {
    pub(crate) operation: &'static str,
    pub(crate) kind: ConnectionErrorKind,
    pub(crate) message: String,
    #[source]
    pub(crate) source: Option<Box<dyn std::error::Error + Send + Sync>>,
}

impl ConnectionError {
    /// The operation that failed.
    #[must_use]
    pub fn operation(&self) -> &str {
        self.operation
    }

    /// Why no response arrived.
    #[must_use]
    pub fn kind(&self) -> ConnectionErrorKind {
        self.kind
    }
}

string_enum! {
    /// The API's machine-readable error code: the `error` field of an error
    /// response, one of the public spec's codes. A code the SDK does not know
    /// is kept verbatim in [`ErrorCode::Unknown`] and categorised by HTTP
    /// status.
    ///
    /// ```
    /// use ensemble_weather::ErrorCode;
    /// assert_eq!(ErrorCode::from("OUTSIDE_COVERAGE"), ErrorCode::OutsideCoverage);
    /// assert_eq!(ErrorCode::from("A_CODE_ADDED_LATER").as_str(), "A_CODE_ADDED_LATER");
    /// ```
    pub enum ErrorCode {
        /// The API key is not valid.
        InvalidApiKey => "INVALID_API_KEY",
        /// Credentials are required.
        Unauthorized => "UNAUTHORIZED",
        /// The request needs a Pro key.
        ProRequired => "PRO_REQUIRED",
        /// The named location does not exist.
        LocationNotFound => "LOCATION_NOT_FOUND",
        /// The resource does not exist.
        NotFound => "NOT_FOUND",
        /// The request is malformed.
        BadRequest => "BAD_REQUEST",
        /// The coordinates are invalid.
        InvalidCoordinates => "INVALID_COORDINATES",
        /// The elevation is invalid.
        InvalidElevation => "INVALID_ELEVATION",
        /// The request needs an elevation.
        ElevationRequired => "ELEVATION_REQUIRED",
        /// The coordinates are outside the forecast domain.
        OutsideCoverage => "OUTSIDE_COVERAGE",
        /// A rate limit or quota was exceeded.
        TooManyRequests => "TOO_MANY_REQUESTS",
        /// No forecast is available right now.
        ForecastUnavailable => "FORECAST_UNAVAILABLE",
        /// The server timed out.
        Timeout => "TIMEOUT",
        /// An internal server error.
        InternalError => "INTERNAL_ERROR",
        /// A database error.
        DatabaseError => "DATABASE_ERROR",
        /// The server could not check credentials right now.
        AuthUnavailable => "AUTH_UNAVAILABLE",
    }
}

impl ErrorCode {
    /// The category this code maps to, or `None` for an unknown code (which
    /// is categorised by HTTP status).
    #[must_use]
    pub fn category(&self) -> Option<Category> {
        Some(match self {
            Self::InvalidApiKey | Self::Unauthorized => Category::Authentication,
            Self::ProRequired => Category::Permission,
            Self::LocationNotFound | Self::NotFound => Category::NotFound,
            Self::BadRequest
            | Self::InvalidCoordinates
            | Self::InvalidElevation
            | Self::ElevationRequired => Category::InvalidRequest,
            Self::OutsideCoverage => Category::OutsideCoverage,
            Self::TooManyRequests => Category::RateLimited,
            Self::ForecastUnavailable
            | Self::Timeout
            | Self::InternalError
            | Self::DatabaseError
            | Self::AuthUnavailable => Category::Unavailable,
            Self::Unknown(_) => return None,
        })
    }
}
