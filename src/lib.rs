//! Official Rust SDK for the [Ourea](https://oureawx.com) probabilistic
//! weather API.
//!
//! Every forecast value is a distribution ([`Quantiles`]) computed by the
//! server from the full ensemble. Ask the server for the number you need: a
//! [`Chance`] for a probability, a [`Total`] for a window total. The SDK never
//! sums, averages or interpolates quantiles itself, because only the server
//! has every member.
//!
//! # Quickstart
//!
//! No signup and no key: an anonymous forecast for Mt. Hood Meadows.
//!
//! ```no_run
//! use ensemble_weather::{Chance, Client, Threshold, Variable};
//!
//! #[tokio::main(flavor = "current_thread")]
//! async fn main() -> Result<(), ensemble_weather::Error> {
//!     let client = Client::new()?;
//!     let forecast = client
//!         .forecast("or/mt-hood-meadows")
//!         .chance(Chance::at_least(Variable::Snowfall, Threshold::inches(6.0)))
//!         .await?;
//!
//!     for hour in forecast.series.timeseries.iter().take(24) {
//!         if let Some(range) = hour.ensemble.temperature.range80() {
//!             println!("{}  {range:.1}", hour.valid_time);
//!         }
//!     }
//!     if let Some(p) = forecast.probabilities[0].value {
//!         println!("chance of 6 in of snow in 24 h: {:.0}%", p * 100.0);
//!     }
//!     Ok(())
//! }
//! ```
//!
//! # The client
//!
//! [`Client`] is async (Tokio) and cheap to clone. Configure it with
//! [`Client::builder`]; the API key defaults to `OUREA_API_KEY`, and with no
//! key calls are anonymous (Free tier). Each operation is a method that
//! returns a request builder: chain its options, then `.await` it.
//! [`blocking::Client`] (feature `blocking`) offers the same calls without
//! async.
//!
//! The SDK sends one canonical URL per request (sorted parameters, defaults
//! omitted) so requests share edge-cache entries; sends search, catalog
//! details and attribution without the key for the same reason; follows
//! same-host redirects and drops the key on any other host; retries
//! connection errors, 429, 502, 503 and 504 with full-jitter backoff
//! (honouring `Retry-After` on 429, 503 and 504); and
//! revalidates repeated requests with a local ETag cache.
//!
//! # Models
//!
//! Every response type is at the crate root. Fields the API may omit are
//! `Option` (absent, never 0) or an empty `Vec`; unknown fields are ignored
//! and unknown enum values keep their raw string, so a newer API never breaks
//! an older SDK. Times are RFC 3339 strings and dates `YYYY-MM-DD` strings,
//! exactly as the API sends them.
//!
//! Field names are the API's JSON names, except where one repeats a word of
//! the type that holds it (BEHAVIOR.md 3.0, one table for every Ourea SDK):
//! [`Forecast::series`] is the JSON `forecast`, [`Current::now`] is
//! `current`, [`ProbabilityResult::value`] is `probability`, list rows are
//! `items`, and so on. Each such field's docs start with its JSON name. The
//! models decode the JSON names and serialise the SDK names (which they
//! decode too). The GeoJSON layers ([`AlertOutlines`], [`AvalancheZones`])
//! keep every key as the API sends it.
//!
//! # Errors
//!
//! Every call returns [`Result<T, Error>`](Result). [`Error`] has one variant
//! per category (authentication, permission, not found, invalid request,
//! outside coverage, rate limited, unavailable, connection, usage). Errors
//! never contain the API key.
//!
//! # Documentation
//!
//! Guides: <https://docs.oureawx.com>, and this SDK's at
//! <https://docs.oureawx.com/rust>.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![cfg_attr(docsrs, feature(doc_cfg))]

#[cfg(feature = "blocking")]
#[cfg_attr(docsrs, doc(cfg(feature = "blocking")))]
pub mod blocking;
mod cache;
mod client;
mod enums;
mod error;
mod meta;
mod models;
mod paging;
mod probability;
mod query;
mod requests;
mod transport;
mod units;

#[cfg(test)]
mod tests;

pub use cache::{Cache, CacheEntry, DEFAULT_CACHE_BYTES, DEFAULT_CACHE_ENTRIES, MemoryCache};
pub use client::{
    Client, ClientBuilder, DEFAULT_BASE_URL, DEFAULT_MAX_RETRIES, DEFAULT_TIMEOUT, Deprecation,
    ENV_API_KEY, ENV_BASE_URL, VERSION,
};
pub use enums::{
    AlertCertainty, AlertClass, AlertSeverity, AlertUrgency, AstronomicalSeason, ClampedOption,
    Condition, ConditionIcon, CurrentSource, ElevationSource, ExploreMetric, ExplorePoints,
    FreezingLevelKind, GapFlowStatus, InversionKind, LocationType, MembersView, MoonPhase,
    NowcastSource, NullReason, Order, PassStatusLevel, PrecipKind, SectionKind, SnowCharacter,
    SourceKind, StationNetwork, StationStatus, StormOrientation, TemplateVariant, WindStatus,
};
pub use error::{
    ApiError, Category, ConnectionError, ConnectionErrorKind, Error, ErrorCode, NearestLocation,
    Result, UsageError,
};
pub use meta::{Headers, RateLimit, ResponseMeta};
pub use models::*;
pub use paging::Items;
pub use probability::{Aggregation, Chance, MAX_WINDOW_HOURS, Op, Total, Variable, Window};
pub use requests::{
    AlertOutlinesRequest, AlertsRequest, AttributionRequest, AvalancheZonesRequest,
    CatalogLocationRequest, CatalogLocationsRequest, CatalogStationRequest, CatalogStationsRequest,
    CompositionRequest, CurrentRequest, ExploreRequest, ForecastRequest, LocationKey,
    LocationsRequest, Members, NearbyPointRequest, NearbyRequest, Point, RunsRequest,
    SearchRequest, SeasonRequest, SnowTrendRequest, SnowpackPlumeRequest, SnowpackRequest,
};
pub use transport::MAX_RESPONSE_BYTES;
pub use units::{Threshold, Unit, Units};
