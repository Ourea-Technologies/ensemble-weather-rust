//! One request builder per API operation.
//!
//! Every builder is created by a [`Client`] method, takes optional settings
//! through chained setters, and is sent by awaiting it (or with `.send()`).
//! With the `blocking` feature the same builders, made by
//! [`blocking::Client`](crate::blocking::Client), are sent with `.send()`.

use std::fmt;
use std::future::{Future, IntoFuture};
use std::pin::Pin;

use crate::client::{Client, Operation};
use crate::enums::{LocationType, StationNetwork, StationStatus, TemplateVariant};
use crate::error::{Error, Result};
use crate::models::{
    Alerts, Attribution, CatalogLocation, CatalogLocationDetail, CatalogLocationPage,
    CatalogStation, CatalogStationDetail, CatalogStationPage, Current, Forecast, LocationRef,
    LocationSummary, LocationsPage, NearbyLocations, NearbyPoint, Runs, SearchResults,
};
use crate::paging::Items;
use crate::probability::{Chance, Total, build_chances, build_totals, map_chances, map_totals};
use crate::query::Query;
use crate::units::Units;

/// Every operation the SDK exposes, by OpenAPI `operationId`.
pub(crate) mod ops {
    use crate::client::Operation;

    const fn keyed(id: &'static str) -> Operation {
        Operation { id, open: false }
    }
    const fn open(id: &'static str) -> Operation {
        Operation { id, open: true }
    }

    pub(crate) const LOCATION_FORECAST: Operation = keyed("getLocationForecast");
    pub(crate) const POINT_FORECAST: Operation = keyed("getPointForecast");
    pub(crate) const CURRENT: Operation = keyed("getCurrentConditions");
    pub(crate) const POINT_CURRENT: Operation = keyed("getCurrentByCoordinates");
    pub(crate) const ALERTS: Operation = keyed("getLocationAlerts");
    pub(crate) const POINT_ALERTS: Operation = keyed("getAlertsByCoordinates");
    pub(crate) const NEARBY: Operation = keyed("getNearbyLocations");
    pub(crate) const POINT_NEARBY: Operation = keyed("getNearbyByCoordinates");
    pub(crate) const SEARCH: Operation = open("searchLocations");
    pub(crate) const ALL_LOCATIONS: Operation = keyed("getAllLocations");
    pub(crate) const STATE_LOCATIONS: Operation = keyed("getLocations");
    pub(crate) const CATALOG_LOCATIONS: Operation = keyed("listCatalogLocations");
    pub(crate) const CATALOG_LOCATION: Operation = open("getCatalogLocation");
    pub(crate) const CATALOG_STATIONS: Operation = keyed("listCatalogStations");
    pub(crate) const CATALOG_STATION: Operation = open("getCatalogStation");
    pub(crate) const RUNS: Operation = keyed("getAvailableRuns");
    pub(crate) const ATTRIBUTION: Operation = open("getAttribution");

    /// For the spec-drift test.
    #[cfg(test)]
    pub(crate) const ALL: &[Operation] = &[
        LOCATION_FORECAST,
        POINT_FORECAST,
        CURRENT,
        POINT_CURRENT,
        ALERTS,
        POINT_ALERTS,
        NEARBY,
        POINT_NEARBY,
        SEARCH,
        ALL_LOCATIONS,
        STATE_LOCATIONS,
        CATALOG_LOCATIONS,
        CATALOG_LOCATION,
        CATALOG_STATIONS,
        CATALOG_STATION,
        RUNS,
        ATTRIBUTION,
    ];
}

/// A named location: the pair (state, slug).
///
/// Convert from `"state/slug"` (split on its one `/`), from a
/// `(state, slug)` pair, or from a [`LocationRef`]. A two-letter state code
/// is lowercased; a state name is passed through (the server redirects it).
///
/// ```
/// use ensemble_weather::LocationKey;
/// let a = LocationKey::from("OR/mt-hood-meadows");
/// let b = LocationKey::from(("or", "mt-hood-meadows"));
/// assert_eq!(a, b);
/// assert_eq!(a.to_string(), "or/mt-hood-meadows");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LocationKey(Result<(String, String), String>);

impl LocationKey {
    /// A key from its two parts.
    #[must_use]
    pub fn new(state: impl Into<String>, slug: impl Into<String>) -> Self {
        let (state, slug) = (state.into(), slug.into());
        if state.is_empty() || slug.is_empty() {
            return Self(Err(format!("{state}/{slug}")));
        }
        let state = if state.len() == 2 {
            state.to_ascii_lowercase()
        } else {
            state
        };
        Self(Ok((state, slug)))
    }

    fn parts(&self, operation: &'static str) -> Result<(&str, &str)> {
        match &self.0 {
            Ok((state, slug)) => Ok((state, slug)),
            Err(raw) => Err(Error::usage(
                operation,
                format!(
                    "location {raw:?} must be \"state/slug\", for example \"or/mt-hood-meadows\""
                ),
            )),
        }
    }
}

impl From<&str> for LocationKey {
    fn from(s: &str) -> Self {
        match s.split_once('/') {
            Some((state, slug)) if !slug.contains('/') => Self::new(state, slug),
            _ => Self(Err(s.to_owned())),
        }
    }
}

impl From<String> for LocationKey {
    fn from(s: String) -> Self {
        Self::from(s.as_str())
    }
}

impl From<&String> for LocationKey {
    fn from(s: &String) -> Self {
        Self::from(s.as_str())
    }
}

impl<S: Into<String>, T: Into<String>> From<(S, T)> for LocationKey {
    fn from((state, slug): (S, T)) -> Self {
        Self::new(state, slug)
    }
}

impl From<&LocationRef> for LocationKey {
    fn from(l: &LocationRef) -> Self {
        // A coordinate request's point has no slug: the call is then a usage error.
        Self::new(l.state.clone(), l.slug.clone().unwrap_or_default())
    }
}

impl fmt::Display for LocationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            Ok((state, slug)) => write!(f, "{state}/{slug}"),
            Err(raw) => f.write_str(raw),
        }
    }
}

/// A coordinate in decimal degrees (WGS84, west negative), sent exactly as
/// given.
///
/// ```
/// use ensemble_weather::Point;
/// let summit = Point::new(45.3735, -121.6959).elevation_m(3_429.0);
/// assert_eq!(summit.elevation_m, Some(3_429.0));
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct Point {
    /// Latitude.
    pub lat: f64,
    /// Longitude.
    pub lon: f64,
    /// Elevation in metres, whatever the units. `None` lets the server use
    /// its 10 m terrain model. Only forecasts and current conditions take it.
    pub elevation_m: Option<f64>,
}

impl Point {
    /// The point at `lat`, `lon`.
    #[must_use]
    pub fn new(lat: f64, lon: f64) -> Self {
        Self {
            lat,
            lon,
            elevation_m: None,
        }
    }

    /// Sets the elevation, metres.
    #[must_use]
    pub fn elevation_m(mut self, metres: f64) -> Self {
        self.elevation_m = Some(metres);
        self
    }

    fn query(&self, q: &mut Query, with_elevation: bool) {
        q.num("lat", Some(self.lat), None);
        q.num("lon", Some(self.lon), None);
        if with_elevation {
            q.num("elevation_m", self.elevation_m, None);
        }
    }
}

#[derive(Debug, Clone)]
enum Target {
    Location(LocationKey),
    Point(Point),
}

type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Implements `send` (and `IntoFuture` for the async client, `send` for the
/// blocking one) on a request builder.
macro_rules! sendable {
    ($req:ident => $out:ty, $exec:ident) => {
        impl<'a> $req<'a, Client> {
            /// Sends the request; awaiting the builder does the same.
            ///
            /// # Errors
            ///
            /// Any [`Error`]: the API's, a connection failure, or a usage
            /// error found before sending.
            pub async fn send(self) -> Result<$out> {
                self.client.$exec(self.params).await
            }
        }

        impl<'a> IntoFuture for $req<'a, Client> {
            type Output = Result<$out>;
            type IntoFuture = BoxFuture<'a, Result<$out>>;

            fn into_future(self) -> Self::IntoFuture {
                Box::pin(self.send())
            }
        }

        #[cfg(feature = "blocking")]
        impl $req<'_, crate::blocking::Client> {
            /// Sends the request and waits for the answer.
            ///
            /// # Errors
            ///
            /// Any [`Error`]: the API's, a connection failure, or a usage
            /// error found before sending.
            pub fn send(self) -> Result<$out> {
                self.client.block_on(self.client.inner.$exec(self.params))
            }
        }
    };
}

/// Implements `items()` (a stream for the async client, an iterator for the
/// blocking one) on a paged request builder.
macro_rules! pageable {
    ($req:ident => $item:ty, $page:ident) => {
        impl $req<'_, Client> {
            /// Every item from this page on, following `next_cursor` until it
            /// is `None`, as a [`Stream`](futures_core::Stream).
            pub fn items(self) -> Items<$item> {
                let client = self.client.clone();
                let params = self.params;
                Items::new(params.cursor.clone(), move |cursor| {
                    let client = client.clone();
                    let mut params = params.clone();
                    params.cursor = cursor;
                    Box::pin(async move { client.$page(params).await })
                })
            }
        }

        #[cfg(feature = "blocking")]
        impl $req<'_, crate::blocking::Client> {
            /// Every item from this page on, following `next_cursor` until it
            /// is `None`.
            pub fn items(self) -> crate::blocking::Items<$item> {
                let blocking = self.client.clone();
                let request = $req {
                    client: &blocking.inner,
                    params: self.params,
                };
                crate::blocking::Items::new(blocking.clone(), request.items())
            }
        }
    };
}

// ---------------------------------------------------------------- forecast

#[derive(Debug, Clone)]
pub(crate) struct ForecastParams {
    target: Target,
    units: Option<Units>,
    max_hours: Option<u32>,
    max_days: Option<u32>,
    include_past: Option<bool>,
    models: Option<bool>,
    members: Option<bool>,
    daily: Option<bool>,
    hourly: Option<bool>,
    nowcast: Option<bool>,
    elevations: Vec<String>,
    run: Option<String>,
    sources: Vec<String>,
    exclude_sources: Vec<String>,
    chances: Vec<Chance>,
    totals: Vec<Total>,
    diagnostics: Vec<String>,
    include: Vec<String>,
    feels_like: Vec<String>,
}

/// A forecast request, from [`Client::forecast`] or [`Client::get_point_forecast`].
///
/// ```no_run
/// use ensemble_weather::{Chance, Client, Threshold, Total, Units, Variable, Window};
///
/// # async fn run() -> Result<(), ensemble_weather::Error> {
/// let client = Client::new()?;
/// let forecast = client
///     .forecast("or/mt-hood-meadows")
///     .units(Units::Imperial)
///     .max_hours(72)
///     .chance(Chance::at_least(Variable::Snowfall, Threshold::inches(6.0)))
///     .total(Total::new(Variable::Snowfall).window(Window::hours(72)))
///     .await?;
/// # let _ = forecast;
/// # Ok(()) }
/// ```
#[derive(Debug, Clone)]
#[must_use = "a request does nothing until it is awaited or sent"]
pub struct ForecastRequest<'a, C = Client> {
    client: &'a C,
    params: ForecastParams,
}

fn strings<I: IntoIterator<Item = S>, S: Into<String>>(items: I) -> Vec<String> {
    items.into_iter().map(Into::into).collect()
}

impl<C> ForecastRequest<'_, C> {
    /// Overrides the client's unit system for this request.
    pub fn units(mut self, units: Units) -> Self {
        self.params.units = Some(units);
        self
    }

    /// The number of hours past the first (6..=360; default 24), clamped to
    /// the tier's outlook (Free 240, Pro 360).
    pub fn max_hours(mut self, hours: u32) -> Self {
        self.params.max_hours = Some(hours);
        self
    }

    /// The number of daily rows (1..=15; default the full horizon).
    pub fn max_days(mut self, days: u32) -> Self {
        self.params.max_days = Some(days);
        self
    }

    /// Serves the hourly series from the model run's init (default false).
    pub fn include_past(mut self, yes: bool) -> Self {
        self.params.include_past = Some(yes);
        self
    }

    /// Adds per-model breakdowns (Pro; default false).
    pub fn models(mut self, yes: bool) -> Self {
        self.params.models = Some(yes);
        self
    }

    /// Adds per-member values (Free: the plume view; default false).
    pub fn members(mut self, yes: bool) -> Self {
        self.params.members = Some(yes);
        self
    }

    /// Includes the daily summaries (default true).
    pub fn daily(mut self, yes: bool) -> Self {
        self.params.daily = Some(yes);
        self
    }

    /// Includes the hourly series (default true).
    pub fn hourly(mut self, yes: bool) -> Self {
        self.params.hourly = Some(yes);
        self
    }

    /// Blends the live nowcast layer (default true).
    pub fn nowcast(mut self, yes: bool) -> Self {
        self.params.nowcast = Some(yes);
        self
    }

    /// Adds elevation forecasts: `["all"]` or elevation names. Location
    /// forecasts only; for a point, use [`Point::elevation_m`].
    pub fn elevations<I: IntoIterator<Item = S>, S: Into<String>>(mut self, names: I) -> Self {
        self.params.elevations = strings(names);
        self
    }

    /// Pins a historical model run by its init time, RFC 3339 (Pro).
    pub fn run(mut self, initialized_at: impl Into<String>) -> Self {
        self.params.run = Some(initialized_at.into());
        self
    }

    /// Selects the observation sources of the nowcast.
    pub fn sources<I: IntoIterator<Item = S>, S: Into<String>>(mut self, sources: I) -> Self {
        self.params.sources = strings(sources);
        self
    }

    /// Excludes observation sources from the nowcast.
    pub fn exclude_sources<I: IntoIterator<Item = S>, S: Into<String>>(
        mut self,
        sources: I,
    ) -> Self {
        self.params.exclude_sources = strings(sources);
        self
    }

    /// Adds a probability term, answered in
    /// [`Forecast::probabilities`] in the order added.
    pub fn chance(mut self, chance: Chance) -> Self {
        self.params.chances.push(chance);
        self
    }

    /// Adds several probability terms.
    pub fn chances(mut self, chances: impl IntoIterator<Item = Chance>) -> Self {
        self.params.chances.extend(chances);
        self
    }

    /// Adds a window total, answered in [`Forecast::window_totals`] in the
    /// order added.
    pub fn total(mut self, total: Total) -> Self {
        self.params.totals.push(total);
        self
    }

    /// Adds several window totals.
    pub fn totals(mut self, totals: impl IntoIterator<Item = Total>) -> Self {
        self.params.totals.extend(totals);
        self
    }

    /// Opts in to serve-path diagnostics: `"provenance"`.
    pub fn diagnostics<I: IntoIterator<Item = S>, S: Into<String>>(mut self, names: I) -> Self {
        self.params.diagnostics = strings(names);
        self
    }

    /// Opts in to extra forecast variables: `"mslp"` and `"skin_temperature"`
    /// (`ensemble.mslp`, `ensemble.skin_temperature`).
    pub fn include<I: IntoIterator<Item = S>, S: Into<String>>(mut self, variables: I) -> Self {
        self.params.include = strings(variables);
        self
    }

    /// Selects the feels-like indices (default `["nws"]`, the only one).
    pub fn feels_like<I: IntoIterator<Item = S>, S: Into<String>>(mut self, indices: I) -> Self {
        self.params.feels_like = strings(indices);
        self
    }
}

sendable!(ForecastRequest => Forecast, exec_forecast);

// ----------------------------------------------------------------- current

#[derive(Debug, Clone)]
pub(crate) struct CurrentParams {
    target: Target,
    units: Option<Units>,
    sources: Vec<String>,
    exclude_sources: Vec<String>,
    include_nowcast_hours: bool,
}

/// A current-conditions request, from [`Client::current`] or
/// [`Client::get_point_current`].
///
/// ```no_run
/// # async fn run(client: ensemble_weather::Client) -> Result<(), ensemble_weather::Error> {
/// let now = client.current("or/mt-hood-meadows").include_nowcast_hours(true).await?;
/// # let _ = now;
/// # Ok(()) }
/// ```
#[derive(Debug, Clone)]
#[must_use = "a request does nothing until it is awaited or sent"]
pub struct CurrentRequest<'a, C = Client> {
    client: &'a C,
    params: CurrentParams,
}

impl<C> CurrentRequest<'_, C> {
    /// Overrides the client's unit system for this request.
    pub fn units(mut self, units: Units) -> Self {
        self.params.units = Some(units);
        self
    }

    /// Selects observation sources.
    pub fn sources<I: IntoIterator<Item = S>, S: Into<String>>(mut self, sources: I) -> Self {
        self.params.sources = strings(sources);
        self
    }

    /// Excludes observation sources.
    pub fn exclude_sources<I: IntoIterator<Item = S>, S: Into<String>>(
        mut self,
        sources: I,
    ) -> Self {
        self.params.exclude_sources = strings(sources);
        self
    }

    /// Adds the observation-blended hours ([`Current::nowcast_hours`]).
    pub fn include_nowcast_hours(mut self, yes: bool) -> Self {
        self.params.include_nowcast_hours = yes;
        self
    }
}

sendable!(CurrentRequest => Current, exec_current);

// ------------------------------------------------------------------ alerts

#[derive(Debug, Clone)]
pub(crate) struct AlertsParams {
    target: Target,
}

/// An alerts request, from [`Client::alerts`] or [`Client::get_point_alerts`].
///
/// ```no_run
/// # async fn run(client: ensemble_weather::Client) -> Result<(), ensemble_weather::Error> {
/// for alert in client.alerts("or/mt-hood-meadows").await?.alerts {
///     println!("{}", alert.headline);
/// }
/// # Ok(()) }
/// ```
#[derive(Debug, Clone)]
#[must_use = "a request does nothing until it is awaited or sent"]
pub struct AlertsRequest<'a, C = Client> {
    client: &'a C,
    params: AlertsParams,
}

sendable!(AlertsRequest => Alerts, exec_alerts);

// ------------------------------------------------------------------ nearby

#[derive(Debug, Clone)]
pub(crate) struct NearbyParams {
    target: Target,
    units: Option<Units>,
    limit: Option<u32>,
    radius_km: Option<f64>,
}

/// A nearby-locations request around a named location, from [`Client::nearby`].
///
/// ```no_run
/// # async fn run(client: ensemble_weather::Client) -> Result<(), ensemble_weather::Error> {
/// let near = client.nearby("or/mt-hood-meadows").limit(5).radius_km(30.0).await?;
/// # let _ = near;
/// # Ok(()) }
/// ```
#[derive(Debug, Clone)]
#[must_use = "a request does nothing until it is awaited or sent"]
pub struct NearbyRequest<'a, C = Client> {
    client: &'a C,
    params: NearbyParams,
}

/// A nearby-locations request around a point, from [`Client::get_point_nearby`].
///
/// ```no_run
/// # async fn run(client: ensemble_weather::Client) -> Result<(), ensemble_weather::Error> {
/// use ensemble_weather::Point;
/// let near = client.get_point_nearby(Point::new(45.33, -121.66)).limit(5).await?;
/// # let _ = near;
/// # Ok(()) }
/// ```
#[derive(Debug, Clone)]
#[must_use = "a request does nothing until it is awaited or sent"]
pub struct NearbyPointRequest<'a, C = Client> {
    client: &'a C,
    params: NearbyParams,
}

macro_rules! nearby_setters {
    ($req:ident) => {
        impl<C> $req<'_, C> {
            /// Sets the unit of the rows' elevations.
            pub fn units(mut self, units: Units) -> Self {
                self.params.units = Some(units);
                self
            }

            /// The number of neighbours (1..=20; default 3).
            pub fn limit(mut self, limit: u32) -> Self {
                self.params.limit = Some(limit);
                self
            }

            /// The search radius, km (up to 100; default 50).
            pub fn radius_km(mut self, km: f64) -> Self {
                self.params.radius_km = Some(km);
                self
            }
        }
    };
}
nearby_setters!(NearbyRequest);
nearby_setters!(NearbyPointRequest);
sendable!(NearbyRequest => NearbyLocations, exec_nearby);
sendable!(NearbyPointRequest => NearbyPoint, exec_point_nearby);

// ------------------------------------------------------------------ search

#[derive(Debug, Clone)]
pub(crate) struct SearchParams {
    text: String,
    state: Option<String>,
    county: Option<String>,
    kind: Option<LocationType>,
    limit: Option<u32>,
}

/// A location search, from [`Client::search`]. Sent without the API key, so
/// results are shared at the edge cache.
///
/// ```no_run
/// # async fn run(client: ensemble_weather::Client) -> Result<(), ensemble_weather::Error> {
/// use ensemble_weather::LocationType;
/// let hits = client.search("hood").kind(LocationType::SkiArea).limit(5).await?;
/// for hit in hits.items {
///     println!("{} {}", hit.key, hit.location.name);
/// }
/// # Ok(()) }
/// ```
#[derive(Debug, Clone)]
#[must_use = "a request does nothing until it is awaited or sent"]
pub struct SearchRequest<'a, C = Client> {
    client: &'a C,
    params: SearchParams,
}

impl<C> SearchRequest<'_, C> {
    /// Filters by two-letter code or state name.
    pub fn state(mut self, state: impl Into<String>) -> Self {
        self.params.state = Some(state.into());
        self
    }

    /// Filters by county.
    pub fn county(mut self, county: impl Into<String>) -> Self {
        self.params.county = Some(county.into());
        self
    }

    /// Filters by location type.
    pub fn kind(mut self, kind: LocationType) -> Self {
        self.params.kind = Some(kind);
        self
    }

    /// The maximum number of hits (1..=100; default 20).
    pub fn limit(mut self, limit: u32) -> Self {
        self.params.limit = Some(limit);
        self
    }
}

sendable!(SearchRequest => SearchResults, exec_search);

// --------------------------------------------------------------- locations

#[derive(Debug, Clone)]
pub(crate) struct LocationsParams {
    state: Option<String>,
    kind: Option<LocationType>,
    units: Option<Units>,
    limit: Option<u32>,
    cursor: Option<String>,
}

/// A page of named locations, from [`Client::locations`]. `.items()` walks
/// every page.
///
/// ```no_run
/// # async fn run(client: ensemble_weather::Client) -> Result<(), ensemble_weather::Error> {
/// let mut all = client.locations().state("or").limit(500).items();
/// while let Some(location) = all.try_next().await? {
///     println!("{} ({:?})", location.location.name, location.location.slug);
/// }
/// # Ok(()) }
/// ```
#[derive(Debug, Clone)]
#[must_use = "a request does nothing until it is awaited or sent"]
pub struct LocationsRequest<'a, C = Client> {
    client: &'a C,
    params: LocationsParams,
}

impl<C> LocationsRequest<'_, C> {
    /// Limits the list to one state (default: every state).
    pub fn state(mut self, state: impl Into<String>) -> Self {
        self.params.state = Some(state.into());
        self
    }

    /// Filters by location type.
    pub fn kind(mut self, kind: LocationType) -> Self {
        self.params.kind = Some(kind);
        self
    }

    /// Sets the unit of the rows' elevations.
    pub fn units(mut self, units: Units) -> Self {
        self.params.units = Some(units);
        self
    }

    /// The page size (1..=5000).
    pub fn limit(mut self, limit: u32) -> Self {
        self.params.limit = Some(limit);
        self
    }

    /// Starts at this cursor (a previous page's `next_cursor`).
    pub fn cursor(mut self, cursor: impl Into<String>) -> Self {
        self.params.cursor = Some(cursor.into());
        self
    }
}

sendable!(LocationsRequest => LocationsPage, exec_locations);
pageable!(LocationsRequest => LocationSummary, page_locations);

// ----------------------------------------------------------------- catalog

#[derive(Debug, Clone, Default)]
pub(crate) struct CatalogLocationsParams {
    admin1: Option<String>,
    country: Option<String>,
    kind: Option<LocationType>,
    template: Option<TemplateVariant>,
    limit: Option<u32>,
    cursor: Option<String>,
}

/// A page of the location catalog, from [`Client::catalog_locations`].
/// `.items()` walks every page.
///
/// ```no_run
/// # async fn run(client: ensemble_weather::Client) -> Result<(), ensemble_weather::Error> {
/// let page = client.catalog_locations().admin1("US-OR").limit(50).await?;
/// println!("{} records, more: {}", page.items.len(), page.next_cursor.is_some());
/// # Ok(()) }
/// ```
#[derive(Debug, Clone)]
#[must_use = "a request does nothing until it is awaited or sent"]
pub struct CatalogLocationsRequest<'a, C = Client> {
    client: &'a C,
    params: CatalogLocationsParams,
}

impl<C> CatalogLocationsRequest<'_, C> {
    /// An ISO 3166-2 code, such as `"US-OR"`.
    pub fn admin1(mut self, code: impl Into<String>) -> Self {
        self.params.admin1 = Some(code.into());
        self
    }

    /// An ISO 3166-1 alpha-2 code, such as `"US"`.
    pub fn country(mut self, code: impl Into<String>) -> Self {
        self.params.country = Some(code.into());
        self
    }

    /// A location type, such as [`LocationType::SkiArea`].
    pub fn kind(mut self, kind: impl Into<LocationType>) -> Self {
        self.params.kind = Some(kind.into());
        self
    }

    /// A template variant, such as [`TemplateVariant::Mountain`].
    pub fn template(mut self, variant: impl Into<TemplateVariant>) -> Self {
        self.params.template = Some(variant.into());
        self
    }

    /// The page size (1..=100; default 20).
    pub fn limit(mut self, limit: u32) -> Self {
        self.params.limit = Some(limit);
        self
    }

    /// Starts at this cursor (a previous page's `next_cursor`).
    pub fn cursor(mut self, cursor: impl Into<String>) -> Self {
        self.params.cursor = Some(cursor.into());
        self
    }
}

sendable!(CatalogLocationsRequest => CatalogLocationPage, exec_catalog_locations);
pageable!(CatalogLocationsRequest => CatalogLocation, page_catalog_locations);

#[derive(Debug, Clone, Default)]
pub(crate) struct CatalogStationsParams {
    admin1: Option<String>,
    country: Option<String>,
    networks: Vec<StationNetwork>,
    capabilities: Vec<String>,
    status: Option<StationStatus>,
    limit: Option<u32>,
    cursor: Option<String>,
}

/// A page of the station catalog, from [`Client::catalog_stations`].
/// `.items()` walks every page.
///
/// ```no_run
/// # async fn run(client: ensemble_weather::Client) -> Result<(), ensemble_weather::Error> {
/// use ensemble_weather::{StationNetwork, StationStatus};
/// let page = client
///     .catalog_stations()
///     .networks([StationNetwork::Snotel])
///     .status(StationStatus::Active)
///     .capabilities(["snow_depth"])
///     .await?;
/// # let _ = page;
/// # Ok(()) }
/// ```
#[derive(Debug, Clone)]
#[must_use = "a request does nothing until it is awaited or sent"]
pub struct CatalogStationsRequest<'a, C = Client> {
    client: &'a C,
    params: CatalogStationsParams,
}

impl<C> CatalogStationsRequest<'_, C> {
    /// An ISO 3166-2 code, such as `"US-OR"`.
    pub fn admin1(mut self, code: impl Into<String>) -> Self {
        self.params.admin1 = Some(code.into());
        self
    }

    /// An ISO 3166-1 alpha-2 code, such as `"US"`.
    pub fn country(mut self, code: impl Into<String>) -> Self {
        self.params.country = Some(code.into());
        self
    }

    /// Filters by network, such as [`StationNetwork::Snotel`] (any of them).
    pub fn networks<I: IntoIterator<Item = S>, S: Into<StationNetwork>>(
        mut self,
        networks: I,
    ) -> Self {
        self.params.networks = networks.into_iter().map(Into::into).collect();
        self
    }

    /// Capabilities a station must all have.
    pub fn capabilities<I: IntoIterator<Item = S>, S: Into<String>>(
        mut self,
        capabilities: I,
    ) -> Self {
        self.params.capabilities = strings(capabilities);
        self
    }

    /// A live-feed status, such as [`StationStatus::Active`].
    pub fn status(mut self, status: impl Into<StationStatus>) -> Self {
        self.params.status = Some(status.into());
        self
    }

    /// The page size (1..=100; default 20).
    pub fn limit(mut self, limit: u32) -> Self {
        self.params.limit = Some(limit);
        self
    }

    /// Starts at this cursor (a previous page's `next_cursor`).
    pub fn cursor(mut self, cursor: impl Into<String>) -> Self {
        self.params.cursor = Some(cursor.into());
        self
    }
}

sendable!(CatalogStationsRequest => CatalogStationPage, exec_catalog_stations);
pageable!(CatalogStationsRequest => CatalogStation, page_catalog_stations);

#[derive(Debug, Clone)]
pub(crate) struct CatalogDetailParams {
    slug: String,
    include: Vec<String>,
    locale: Option<String>,
}

/// A catalog record request, from [`Client::catalog_location`]. Sent without
/// the API key, so the response is shared at the edge cache.
///
/// ```no_run
/// # async fn run(client: ensemble_weather::Client) -> Result<(), ensemble_weather::Error> {
/// let record = client.catalog_location("mt-hood-meadows").include(["content"]).locale("es").await?;
/// # let _ = record;
/// # Ok(()) }
/// ```
#[derive(Debug, Clone)]
#[must_use = "a request does nothing until it is awaited or sent"]
pub struct CatalogLocationRequest<'a, C = Client> {
    client: &'a C,
    params: CatalogDetailParams,
}

/// A station record request, from [`Client::catalog_station`]. Sent without
/// the API key, so the response is shared at the edge cache.
///
/// ```no_run
/// # async fn run(client: ensemble_weather::Client) -> Result<(), ensemble_weather::Error> {
/// let station = client.catalog_station("mt-hood-test-site-snotel").await?;
/// # let _ = station;
/// # Ok(()) }
/// ```
#[derive(Debug, Clone)]
#[must_use = "a request does nothing until it is awaited or sent"]
pub struct CatalogStationRequest<'a, C = Client> {
    client: &'a C,
    params: CatalogDetailParams,
}

macro_rules! detail_setters {
    ($req:ident) => {
        impl<C> $req<'_, C> {
            /// Opts in to extras: `"content"` and `"template"`.
            pub fn include<I: IntoIterator<Item = S>, S: Into<String>>(
                mut self,
                extras: I,
            ) -> Self {
                self.params.include = strings(extras);
                self
            }

            /// The BCP-47 locale for content (default `"en"`).
            pub fn locale(mut self, locale: impl Into<String>) -> Self {
                self.params.locale = Some(locale.into());
                self
            }
        }
    };
}
detail_setters!(CatalogLocationRequest);
detail_setters!(CatalogStationRequest);
sendable!(CatalogLocationRequest => CatalogLocationDetail, exec_catalog_location);
sendable!(CatalogStationRequest => CatalogStationDetail, exec_catalog_station);

// ------------------------------------------------------ runs, attribution

#[derive(Debug, Clone, Default)]
pub(crate) struct LimitParams {
    limit: Option<u32>,
}

/// An available-runs request, from [`Client::runs`].
///
/// ```no_run
/// # async fn run(client: ensemble_weather::Client) -> Result<(), ensemble_weather::Error> {
/// let runs = client.runs().limit(3).await?;
/// # let _ = runs;
/// # Ok(()) }
/// ```
#[derive(Debug, Clone)]
#[must_use = "a request does nothing until it is awaited or sent"]
pub struct RunsRequest<'a, C = Client> {
    client: &'a C,
    params: LimitParams,
}

macro_rules! limit_setter {
    ($req:ident, $max:literal, $default:literal) => {
        impl<C> $req<'_, C> {
            #[doc = concat!("The maximum number (up to ", $max, "; default ", $default, ").")]
            pub fn limit(mut self, limit: u32) -> Self {
                self.params.limit = Some(limit);
                self
            }
        }
    };
}
limit_setter!(RunsRequest, 100, 10);
sendable!(RunsRequest => Runs, exec_runs);

#[derive(Debug, Clone, Default)]
pub(crate) struct NoParams;

/// An attribution request, from [`Client::attribution`]. Sent without the
/// API key.
///
/// ```no_run
/// # async fn run(client: ensemble_weather::Client) -> Result<(), ensemble_weather::Error> {
/// for provider in client.attribution().await?.providers {
///     println!("{}: {}", provider.name, provider.credit);
/// }
/// # Ok(()) }
/// ```
#[derive(Debug, Clone)]
#[must_use = "a request does nothing until it is awaited or sent"]
pub struct AttributionRequest<'a, C = Client> {
    client: &'a C,
    params: NoParams,
}

sendable!(AttributionRequest => Attribution, exec_attribution);

// ---------------------------------------------------------- constructors

fn forecast_params(target: Target) -> ForecastParams {
    ForecastParams {
        target,
        units: None,
        max_hours: None,
        max_days: None,
        include_past: None,
        models: None,
        members: None,
        daily: None,
        hourly: None,
        nowcast: None,
        elevations: Vec::new(),
        run: None,
        sources: Vec::new(),
        exclude_sources: Vec::new(),
        chances: Vec::new(),
        totals: Vec::new(),
        diagnostics: Vec::new(),
        include: Vec::new(),
        feels_like: Vec::new(),
    }
}

fn current_params(target: Target) -> CurrentParams {
    CurrentParams {
        target,
        units: None,
        sources: Vec::new(),
        exclude_sources: Vec::new(),
        include_nowcast_hours: false,
    }
}

fn nearby_params(target: Target) -> NearbyParams {
    NearbyParams {
        target,
        units: None,
        limit: None,
        radius_km: None,
    }
}

/// The request constructors, shared by [`Client`] and the blocking client.
macro_rules! constructors {
    ($client:ty) => {
        impl $client {
            /// The forecast for a named location (`"or/mt-hood-meadows"`). It
            /// needs no API key.
            pub fn forecast(&self, location: impl Into<LocationKey>) -> ForecastRequest<'_, Self> {
                ForecastRequest {
                    client: self,
                    params: forecast_params(Target::Location(location.into())),
                }
            }

            /// The forecast at a point. It needs a Pro API key.
            pub fn get_point_forecast(&self, point: Point) -> ForecastRequest<'_, Self> {
                ForecastRequest {
                    client: self,
                    params: forecast_params(Target::Point(point)),
                }
            }

            /// Current conditions and active alerts at a named location.
            pub fn current(&self, location: impl Into<LocationKey>) -> CurrentRequest<'_, Self> {
                CurrentRequest {
                    client: self,
                    params: current_params(Target::Location(location.into())),
                }
            }

            /// Current conditions at a point. It needs an API key.
            pub fn get_point_current(&self, point: Point) -> CurrentRequest<'_, Self> {
                CurrentRequest {
                    client: self,
                    params: current_params(Target::Point(point)),
                }
            }

            /// The weather alerts in effect at a named location.
            pub fn alerts(&self, location: impl Into<LocationKey>) -> AlertsRequest<'_, Self> {
                AlertsRequest {
                    client: self,
                    params: AlertsParams {
                        target: Target::Location(location.into()),
                    },
                }
            }

            /// The weather alerts in effect at a point (no elevation). It needs an API key.
            pub fn get_point_alerts(&self, point: Point) -> AlertsRequest<'_, Self> {
                AlertsRequest {
                    client: self,
                    params: AlertsParams {
                        target: Target::Point(point),
                    },
                }
            }

            /// The locations near a named location.
            pub fn nearby(&self, location: impl Into<LocationKey>) -> NearbyRequest<'_, Self> {
                NearbyRequest {
                    client: self,
                    params: nearby_params(Target::Location(location.into())),
                }
            }

            /// The locations near a point (no elevation).
            pub fn get_point_nearby(&self, point: Point) -> NearbyPointRequest<'_, Self> {
                NearbyPointRequest {
                    client: self,
                    params: nearby_params(Target::Point(point)),
                }
            }

            /// Finds locations by free text (at least 2 characters). Sent
            /// without the API key, so results are shared at the edge cache.
            pub fn search(&self, text: impl Into<String>) -> SearchRequest<'_, Self> {
                SearchRequest {
                    client: self,
                    params: SearchParams {
                        text: text.into(),
                        state: None,
                        county: None,
                        kind: None,
                        limit: None,
                    },
                }
            }

            /// One page of named locations (every state's, or one state's
            /// with `.state()`).
            pub fn locations(&self) -> LocationsRequest<'_, Self> {
                LocationsRequest {
                    client: self,
                    params: LocationsParams {
                        state: None,
                        kind: None,
                        units: None,
                        limit: None,
                        cursor: None,
                    },
                }
            }

            /// One page of the location catalog.
            pub fn catalog_locations(&self) -> CatalogLocationsRequest<'_, Self> {
                CatalogLocationsRequest {
                    client: self,
                    params: CatalogLocationsParams::default(),
                }
            }

            /// A location's full catalog record. Sent without the API key.
            pub fn catalog_location(
                &self,
                slug: impl Into<String>,
            ) -> CatalogLocationRequest<'_, Self> {
                CatalogLocationRequest {
                    client: self,
                    params: CatalogDetailParams {
                        slug: slug.into(),
                        include: Vec::new(),
                        locale: None,
                    },
                }
            }

            /// One page of the station catalog.
            pub fn catalog_stations(&self) -> CatalogStationsRequest<'_, Self> {
                CatalogStationsRequest {
                    client: self,
                    params: CatalogStationsParams::default(),
                }
            }

            /// A station's full catalog record. Sent without the API key.
            pub fn catalog_station(
                &self,
                slug: impl Into<String>,
            ) -> CatalogStationRequest<'_, Self> {
                CatalogStationRequest {
                    client: self,
                    params: CatalogDetailParams {
                        slug: slug.into(),
                        include: Vec::new(),
                        locale: None,
                    },
                }
            }

            /// The available model runs, newest first.
            pub fn runs(&self) -> RunsRequest<'_, Self> {
                RunsRequest {
                    client: self,
                    params: LimitParams::default(),
                }
            }

            /// Every data source with its licence, terms and required
            /// notices. Sent without the API key.
            pub fn attribution(&self) -> AttributionRequest<'_, Self> {
                AttributionRequest {
                    client: self,
                    params: NoParams,
                }
            }
        }
    };
}

constructors!(Client);
#[cfg(feature = "blocking")]
constructors!(crate::blocking::Client);

// ---------------------------------------------------------------- execution

impl Client {
    fn units_or(&self, units: Option<Units>) -> Units {
        units.unwrap_or_else(|| self.config.units.clone())
    }

    fn units_query(q: &mut Query, units: &Units) {
        q.opt_default("units", Some(units.as_str()), "metric");
    }

    pub(crate) async fn exec_forecast(&self, p: ForecastParams) -> Result<Forecast> {
        let (op, segments): (Operation, Vec<String>) = match &p.target {
            Target::Location(key) => {
                let (state, slug) = key.parts(ops::LOCATION_FORECAST.id)?;
                (
                    ops::LOCATION_FORECAST,
                    vec!["locations".into(), state.into(), slug.into()],
                )
            }
            Target::Point(_) => (ops::POINT_FORECAST, vec!["forecast".into()]),
        };
        let units = self.units_or(p.units);
        let chances = build_chances(op.id, &p.chances, &units)?;
        let totals = build_totals(op.id, &p.totals)?;
        let mut q = Query::default();
        Self::units_query(&mut q, &units);
        q.int("max_hours", p.max_hours, Some(24));
        q.int("max_days", p.max_days, None);
        q.flag("include_past", p.include_past, false);
        q.flag("models", p.models, false);
        q.flag("members", p.members, false);
        q.flag("daily", p.daily, true);
        q.flag("hourly", p.hourly, true);
        q.flag("nowcast", p.nowcast, true);
        q.opt("run", p.run.as_deref());
        q.list("sources", &p.sources);
        q.list("exclude_sources", &p.exclude_sources);
        q.literal("prob", chances.value.as_deref());
        q.literal("totals", totals.value.as_deref());
        q.list("diagnostics", &p.diagnostics);
        q.sorted_list("include", &p.include);
        if p.feels_like != ["nws"] {
            q.list("feels_like", &p.feels_like);
        }
        match &p.target {
            Target::Location(_) => q.list("elevations", &p.elevations),
            Target::Point(point) => {
                if !p.elevations.is_empty() {
                    return Err(Error::usage(
                        op.id,
                        "elevations apply only to location forecasts; set Point::elevation_m instead",
                    ));
                }
                point.query(&mut q, true);
            }
        }
        let segments: Vec<&str> = segments.iter().map(String::as_str).collect();
        let mut forecast: Forecast = self.get(op, &segments, &q).await?;
        let served_units = forecast.units.clone();
        forecast.probabilities = map_chances(
            op.id,
            &chances,
            std::mem::take(&mut forecast.probabilities),
            &served_units,
        )?;
        forecast.window_totals = map_totals(
            op.id,
            &totals,
            std::mem::take(&mut forecast.window_totals),
            &served_units,
        )?;
        Ok(forecast)
    }

    pub(crate) async fn exec_current(&self, p: CurrentParams) -> Result<Current> {
        let mut q = Query::default();
        Self::units_query(&mut q, &self.units_or(p.units));
        q.list("sources", &p.sources);
        q.list("exclude_sources", &p.exclude_sources);
        if p.include_nowcast_hours {
            q.set("include", "nowcast_hours");
        }
        match &p.target {
            Target::Location(key) => {
                let (state, slug) = key.parts(ops::CURRENT.id)?;
                self.get(ops::CURRENT, &["locations", state, slug, "current"], &q)
                    .await
            }
            Target::Point(point) => {
                point.query(&mut q, true);
                self.get(ops::POINT_CURRENT, &["current"], &q).await
            }
        }
    }

    pub(crate) async fn exec_alerts(&self, p: AlertsParams) -> Result<Alerts> {
        match &p.target {
            Target::Location(key) => {
                let (state, slug) = key.parts(ops::ALERTS.id)?;
                self.get(
                    ops::ALERTS,
                    &["locations", state, slug, "alerts"],
                    &Query::default(),
                )
                .await
            }
            Target::Point(point) => {
                if point.elevation_m.is_some() {
                    return Err(Error::usage(
                        ops::POINT_ALERTS.id,
                        "alerts take no elevation",
                    ));
                }
                let mut q = Query::default();
                point.query(&mut q, false);
                self.get(ops::POINT_ALERTS, &["alerts"], &q).await
            }
        }
    }

    fn nearby_query(&self, p: &NearbyParams) -> Query {
        let mut q = Query::default();
        Self::units_query(&mut q, &self.units_or(p.units.clone()));
        q.int("limit", p.limit, Some(3));
        q.num("radius_km", p.radius_km, Some(50.0));
        q
    }

    pub(crate) async fn exec_nearby(&self, p: NearbyParams) -> Result<NearbyLocations> {
        let q = self.nearby_query(&p);
        let Target::Location(key) = &p.target else {
            return Err(Error::usage(
                ops::NEARBY.id,
                "nearby needs a named location",
            ));
        };
        let (state, slug) = key.parts(ops::NEARBY.id)?;
        self.get(ops::NEARBY, &["locations", state, slug, "nearby"], &q)
            .await
    }

    pub(crate) async fn exec_point_nearby(&self, p: NearbyParams) -> Result<NearbyPoint> {
        let mut q = self.nearby_query(&p);
        let Target::Point(point) = &p.target else {
            return Err(Error::usage(
                ops::POINT_NEARBY.id,
                "get_point_nearby needs a point",
            ));
        };
        if point.elevation_m.is_some() {
            return Err(Error::usage(
                ops::POINT_NEARBY.id,
                "nearby takes no elevation",
            ));
        }
        point.query(&mut q, false);
        self.get(ops::POINT_NEARBY, &["nearby"], &q).await
    }

    pub(crate) async fn exec_search(&self, p: SearchParams) -> Result<SearchResults> {
        let mut q = Query::default();
        q.set("q", p.text);
        q.opt("state", p.state.as_deref());
        q.opt("county", p.county.as_deref());
        q.opt("type", p.kind.as_ref().map(LocationType::as_str));
        q.int("limit", p.limit, Some(20));
        self.get(ops::SEARCH, &["search", "locations"], &q).await
    }

    pub(crate) async fn exec_locations(&self, p: LocationsParams) -> Result<LocationsPage> {
        let mut q = Query::default();
        q.opt("type", p.kind.as_ref().map(LocationType::as_str));
        Self::units_query(&mut q, &self.units_or(p.units));
        q.int("limit", p.limit, None);
        q.opt("cursor", p.cursor.as_deref());
        match p.state.as_deref().filter(|s| !s.is_empty()) {
            None => self.get(ops::ALL_LOCATIONS, &["locations"], &q).await,
            Some(state) => {
                let state = if state.len() == 2 {
                    state.to_ascii_lowercase()
                } else {
                    state.to_owned()
                };
                self.get(ops::STATE_LOCATIONS, &["locations", &state], &q)
                    .await
            }
        }
    }

    pub(crate) async fn page_locations(
        &self,
        p: LocationsParams,
    ) -> Result<(Vec<LocationSummary>, Option<String>)> {
        let page = self.exec_locations(p).await?;
        Ok((page.locations, page.next_cursor))
    }

    pub(crate) async fn exec_catalog_locations(
        &self,
        p: CatalogLocationsParams,
    ) -> Result<CatalogLocationPage> {
        let mut q = Query::default();
        q.opt("admin1", p.admin1.as_deref());
        q.opt("country", p.country.as_deref());
        q.opt("type", p.kind.as_ref().map(LocationType::as_str));
        q.opt("template", p.template.as_ref().map(TemplateVariant::as_str));
        q.int("limit", p.limit, Some(20));
        q.opt("cursor", p.cursor.as_deref());
        self.get(ops::CATALOG_LOCATIONS, &["catalog", "locations"], &q)
            .await
    }

    pub(crate) async fn page_catalog_locations(
        &self,
        p: CatalogLocationsParams,
    ) -> Result<(Vec<CatalogLocation>, Option<String>)> {
        let page = self.exec_catalog_locations(p).await?;
        Ok((page.items, page.next_cursor))
    }

    pub(crate) async fn exec_catalog_stations(
        &self,
        p: CatalogStationsParams,
    ) -> Result<CatalogStationPage> {
        let mut q = Query::default();
        q.opt("admin1", p.admin1.as_deref());
        q.opt("country", p.country.as_deref());
        let networks: Vec<String> = p.networks.iter().map(|n| n.as_str().to_owned()).collect();
        q.sorted_list("network", &networks);
        q.sorted_list("capability", &p.capabilities);
        q.opt("status", p.status.as_ref().map(StationStatus::as_str));
        q.int("limit", p.limit, Some(20));
        q.opt("cursor", p.cursor.as_deref());
        self.get(ops::CATALOG_STATIONS, &["catalog", "stations"], &q)
            .await
    }

    pub(crate) async fn page_catalog_stations(
        &self,
        p: CatalogStationsParams,
    ) -> Result<(Vec<CatalogStation>, Option<String>)> {
        let page = self.exec_catalog_stations(p).await?;
        Ok((page.items, page.next_cursor))
    }

    fn detail_query(p: &CatalogDetailParams) -> Query {
        let mut q = Query::default();
        q.list("include", &p.include);
        q.opt_default("locale", p.locale.as_deref(), "en");
        q
    }

    pub(crate) async fn exec_catalog_location(
        &self,
        p: CatalogDetailParams,
    ) -> Result<CatalogLocationDetail> {
        self.get(
            ops::CATALOG_LOCATION,
            &["catalog", "locations", &p.slug],
            &Self::detail_query(&p),
        )
        .await
    }

    pub(crate) async fn exec_catalog_station(
        &self,
        p: CatalogDetailParams,
    ) -> Result<CatalogStationDetail> {
        self.get(
            ops::CATALOG_STATION,
            &["catalog", "stations", &p.slug],
            &Self::detail_query(&p),
        )
        .await
    }

    pub(crate) async fn exec_runs(&self, p: LimitParams) -> Result<Runs> {
        let mut q = Query::default();
        q.int("limit", p.limit, Some(10));
        self.get(ops::RUNS, &["runs"], &q).await
    }

    pub(crate) async fn exec_attribution(&self, _: NoParams) -> Result<Attribution> {
        self.get(ops::ATTRIBUTION, &["attribution"], &Query::default())
            .await
    }
}
