//! Locations, current conditions, alerts, search and nearby.

use serde::{Deserialize, Serialize};

use crate::enums::{Condition, CurrentSource, LocationType, NowcastSource};
use crate::meta::{ResponseMeta, with_meta};
use crate::models::TimeSeriesPoint;
use crate::units::Units;

/// A named location.
///
/// ```
/// # use ensemble_weather::LocationRef;
/// fn key(location: &LocationRef) -> String {
///     location.key() // "or/mt-hood-meadows", as the location methods take it
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct LocationRef {
    /// The location's slug, unique within its state.
    pub slug: String,
    /// The state, as a lowercase USPS code.
    pub state: String,
    /// The display name.
    pub name: String,
    /// Latitude, decimal degrees.
    pub lat: f64,
    /// Longitude, decimal degrees (west negative).
    pub lon: f64,
    /// Elevation in metres, whatever the request's units.
    pub elevation_m: f64,
    /// The kind of location.
    #[serde(rename = "type")]
    pub kind: LocationType,
    /// The IANA timezone, such as `"America/Los_Angeles"`.
    pub timezone: String,
}

impl LocationRef {
    /// The `"state/slug"` form the location methods take.
    #[must_use]
    pub fn key(&self) -> String {
        format!("{}/{}", self.state, self.slug)
    }
}

/// The data sources a response drew on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ResponseAttribution {
    /// The source IDs.
    #[serde(default)]
    pub sources: Vec<String>,
    /// The line to carry with redistributed output.
    pub credit: String,
    /// Where the full attribution lives.
    pub url: String,
}

/// A weather alert in effect.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ActiveAlert {
    /// The alert's ID.
    pub id: String,
    /// The event, such as `"Winter Storm Warning"`.
    pub event: String,
    /// The severity.
    pub severity: String,
    /// The urgency.
    pub urgency: String,
    /// The headline.
    pub headline: String,
    /// When it begins (RFC 3339).
    pub onset: Option<String>,
    /// When it expires (RFC 3339).
    pub expires: String,
    /// When the hazard ends (RFC 3339).
    pub ends: Option<String>,
}

/// Current conditions and active alerts at a location or point.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Current {
    /// How the response was obtained.
    #[serde(skip)]
    pub meta: ResponseMeta,
    /// The location.
    pub location: LocationRef,
    /// The alerts in effect.
    #[serde(default)]
    pub alerts: Vec<ActiveAlert>,
    /// When the alerts were fetched (RFC 3339).
    pub alerts_as_of: String,
    /// The number of sensors.
    pub sensor_count: u32,
    /// The latest readings of nearby sensors.
    #[serde(default)]
    pub sensors: Vec<SensorObservation>,
    /// Right now; `None` when no served hour is near the request time.
    pub current: Option<CurrentConditions>,
    /// When the response was built (RFC 3339).
    pub as_of: String,
    /// The served model run's init time (RFC 3339).
    pub model_run_initialized_at: Option<String>,
    /// The observation-blended hours (with `include_nowcast_hours`).
    pub nowcast_hours: Option<Vec<TimeSeriesPoint>>,
    /// The unit system of every value.
    pub units: Units,
    /// The data sources.
    pub attribution: ResponseAttribution,
    /// Live inputs this response was built without.
    #[serde(default)]
    pub nowcast_unavailable: Vec<NowcastSource>,
}

/// "Right now": a view of one served forecast hour, in the request's units.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CurrentConditions {
    /// The hour (RFC 3339).
    pub time: String,
    /// Temperature.
    pub temperature: f64,
    /// Feels-like temperature.
    pub feels_like: Option<f64>,
    /// The condition code.
    pub condition: Condition,
    /// The condition, for display.
    pub condition_text: String,
    /// The icon name.
    pub condition_icon: String,
    /// Wind speed.
    pub wind_speed: f64,
    /// Wind gust.
    pub wind_gust: Option<f64>,
    /// Wind direction, degrees.
    pub wind_direction: f64,
    /// Relative humidity, percent.
    pub relative_humidity: Option<f64>,
    /// Dewpoint.
    pub dewpoint: Option<f64>,
    /// The chance of precipitation this hour, 0..1.
    pub precip_probability: Option<f64>,
    /// Where the values came from.
    pub source: CurrentSource,
    /// The station that anchored the hour, when one did.
    pub station: Option<CurrentStation>,
}

/// The surface station that anchored the current hour.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CurrentStation {
    /// The station's name, such as `"KPDX"`.
    pub name: String,
    /// Its observation time (RFC 3339).
    pub observation_time: String,
}

/// One sensor's latest reading, in the request's units.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SensorObservation {
    /// The station's ID.
    pub station_id: String,
    /// The station's name.
    pub name: String,
    /// The observation network.
    pub source: String,
    /// The reading's time (RFC 3339).
    pub observation_time: String,
    /// Temperature.
    pub temperature: Option<f64>,
    /// Dewpoint.
    pub dewpoint: Option<f64>,
    /// Relative humidity, percent.
    pub relative_humidity: Option<f64>,
    /// Wind speed.
    pub wind_speed: Option<f64>,
    /// Wind gust.
    pub wind_gust: Option<f64>,
    /// Wind direction, degrees.
    pub wind_direction: Option<f64>,
    /// Pressure.
    pub pressure: Option<f64>,
    /// Precipitation.
    pub precipitation: Option<f64>,
    /// Precipitation rate.
    pub precip_rate: Option<f64>,
    /// Radar reflectivity, dBZ.
    pub reflectivity: Option<f64>,
    /// Pavement temperature.
    pub pavement_temp: Option<f64>,
    /// Subsurface temperature.
    pub subsurface_temp: Option<f64>,
    /// Visibility.
    pub visibility: Option<f64>,
    /// Cloud cover, percent.
    pub cloud_cover: Option<f64>,
    /// Cloud ceiling.
    pub ceiling_height: Option<f64>,
    /// Snow depth.
    pub snow_depth: Option<f64>,
    /// Snow water equivalent.
    pub swe: Option<f64>,
    /// SWE as a percent of the median.
    pub swe_percent_of_median: Option<f64>,
    /// Snowfall.
    pub snowfall: Option<f64>,
    /// Solar radiation.
    pub solar_radiation: Option<f64>,
    /// The pavement condition.
    pub pavement_condition: Option<String>,
}

/// The alerts in effect at a location or point.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Alerts {
    /// How the response was obtained.
    #[serde(skip)]
    pub meta: ResponseMeta,
    /// The location.
    pub location: LocationRef,
    /// The number of alerts.
    pub alert_count: u32,
    /// When the alerts were fetched (RFC 3339).
    pub as_of: String,
    /// The alerts.
    #[serde(default)]
    pub alerts: Vec<ActiveAlert>,
    /// The data sources.
    pub attribution: ResponseAttribution,
}

/// A named location in a list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct LocationSummary {
    /// The location.
    #[serde(flatten)]
    pub location: LocationRef,
    /// The county.
    pub county: String,
    /// A short description.
    pub description: Option<String>,
    /// The location's forecast elevations, the primary first.
    #[serde(default)]
    pub elevations: Vec<NamedElevation>,
}

/// One of a location's forecast elevations, in the request's length unit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct NamedElevation {
    /// The name, such as `"summit"`.
    pub name: String,
    /// The elevation.
    pub elevation: f64,
    /// Whether this is the primary (the elevation forecasts are served at).
    #[serde(default)]
    pub default: bool,
}

/// A location near another, with its distance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct NearbyLocation {
    /// The location.
    #[serde(flatten)]
    pub location: LocationSummary,
    /// The distance, km.
    pub distance_km: f64,
}

/// The locations near a named location.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct NearbyLocations {
    /// How the response was obtained.
    #[serde(skip)]
    pub meta: ResponseMeta,
    /// The location searched around.
    pub location: LocationSummary,
    /// The search radius, km.
    pub radius_km: f64,
    /// The number of neighbours.
    pub count: u32,
    /// The neighbours, nearest first.
    #[serde(default)]
    pub locations: Vec<NearbyLocation>,
}

/// The locations near a point.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct NearbyPoint {
    /// How the response was obtained.
    #[serde(skip)]
    pub meta: ResponseMeta,
    /// The latitude searched around.
    pub lat: f64,
    /// The longitude searched around.
    pub lon: f64,
    /// The search radius, km.
    pub radius_km: f64,
    /// The number of neighbours.
    pub count: u32,
    /// The neighbours, nearest first.
    #[serde(default)]
    pub locations: Vec<NearbyLocation>,
}

/// One page of a state's (or every state's) named locations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct LocationsPage {
    /// How the response was obtained.
    #[serde(skip)]
    pub meta: ResponseMeta,
    /// The state, or empty for every state.
    pub state: String,
    /// The number of locations on this page.
    pub count: u32,
    /// The locations.
    #[serde(default)]
    pub locations: Vec<LocationSummary>,
    /// The cursor of the next page; `None` on the last page.
    pub next_cursor: Option<String>,
}

/// Location search hits.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SearchResults {
    /// How the response was obtained.
    #[serde(skip)]
    pub meta: ResponseMeta,
    /// The hits, best first.
    #[serde(default)]
    pub items: Vec<SearchHit>,
    /// The number of hits.
    pub count: u32,
}

/// One location search hit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SearchHit {
    /// The `"state/slug"` key, as the location methods take it.
    pub key: String,
    /// The location.
    #[serde(flatten)]
    pub location: LocationRef,
    /// The county.
    pub county: String,
}

with_meta!(
    Current,
    Alerts,
    NearbyLocations,
    NearbyPoint,
    LocationsPage,
    SearchResults
);
