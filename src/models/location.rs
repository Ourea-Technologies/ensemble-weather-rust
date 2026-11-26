//! Locations, current conditions, alerts, search and nearby.

use serde::{Deserialize, Serialize};

use crate::enums::{
    AlertCertainty, AlertSeverity, AlertUrgency, Condition, ConditionIcon, CurrentSource,
    LocationType, NowcastSource,
};
use crate::meta::{ResponseMeta, with_meta};
use crate::models::{ExploreValue, PassForecast, PlaceSummary, TimeSeriesPoint};
use crate::units::Units;

/// A named location, or the point of a coordinate request.
///
/// ```
/// # use ensemble_weather::LocationRef;
/// fn key(location: &LocationRef) -> Option<String> {
///     location.key() // Some("or/mt-hood-meadows"), as the location methods take it
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct LocationRef {
    /// The location's slug, unique within its state; `None` for a coordinate
    /// request that matched no named location.
    pub slug: Option<String>,
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
    /// The kind of location; `None` when `slug` is.
    #[serde(rename = "type")]
    pub kind: Option<LocationType>,
    /// The IANA timezone, such as `"America/Los_Angeles"`.
    pub timezone: String,
}

impl LocationRef {
    /// The `"state/slug"` form the location methods take; `None` for a
    /// coordinate request's point.
    #[must_use]
    pub fn key(&self) -> Option<String> {
        self.slug
            .as_ref()
            .map(|slug| format!("{}/{slug}", self.state))
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
    /// The CAP severity.
    pub severity: AlertSeverity,
    /// The CAP urgency.
    pub urgency: AlertUrgency,
    /// The CAP certainty.
    pub certainty: AlertCertainty,
    /// The headline.
    pub headline: String,
    /// When it begins (RFC 3339).
    pub onset: Option<String>,
    /// When it expires (RFC 3339).
    pub expires: String,
    /// When the hazard ends (RFC 3339).
    pub ends: Option<String>,
    /// When NWS issued this message (RFC 3339).
    pub sent: Option<String>,
    /// The CAP message type: `"alert"`, `"update"` or `"cancel"`. Not a sign
    /// of a new event; dedupe on `event_key`.
    pub message_type: Option<String>,
    /// The IDs of the messages this one replaces.
    #[serde(default)]
    pub references: Vec<String>,
    /// The issuing office, such as `"NWS Portland OR"`.
    pub sender_name: Option<String>,
    /// The places this message covers, as NWS words them.
    pub area_desc: Option<String>,
    /// The full NWS text, unedited.
    pub description: Option<String>,
    /// What NWS says to do, unedited.
    pub instruction: Option<String>,
    /// The NWS forecast page for this location.
    pub source_url: Option<String>,
    /// The stable identity of the event across its messages: dedupe on this
    /// to notify once per event. Opaque.
    pub event_key: Option<String>,
    /// The VTEC action, lower case (`"can"`, `"exp"` and `"upg"` end the
    /// event).
    pub vtec_action: Option<String>,
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
    /// (JSON `current`) Right now; `None` when no served hour is near the
    /// request time.
    #[serde(rename(deserialize = "current"), alias = "now")]
    pub now: Option<CurrentConditions>,
    /// The place summary, written by the server from served values only.
    pub summary: Option<PlaceSummary>,
    /// The mountain pass forecast (mountain passes only).
    pub pass: Option<PassForecast>,
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
    /// (JSON `condition`) The condition code.
    #[serde(rename(deserialize = "condition"), alias = "code")]
    pub code: Condition,
    /// (JSON `condition_text`) The condition, for display.
    #[serde(rename(deserialize = "condition_text"), alias = "text")]
    pub text: String,
    /// (JSON `condition_icon`) The icon hint.
    #[serde(rename(deserialize = "condition_icon"), alias = "icon")]
    pub icon: ConditionIcon,
    /// Whether precipitation was measured in the hour in progress (see
    /// [`TimeSeriesPoint::measured`]).
    pub measured: bool,
    /// Cloud cover, percent (0-100), of the hour in progress.
    pub cloud_cover: Option<f64>,
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
    pub precipitation_probability: Option<f64>,
    /// Where the values came from.
    pub source: CurrentSource,
    /// The station that anchored the hour, when one did.
    pub station: Option<CurrentStation>,
}

/// The surface station that anchored the current hour.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CurrentStation {
    /// (JSON `station_id`) The station's ID in its network, as in
    /// [`SensorObservation::station_id`].
    #[serde(rename(deserialize = "station_id"), alias = "id")]
    pub id: String,
    /// The station's registry name, else its ID.
    pub name: String,
    /// Its observation time (RFC 3339).
    pub observation_time: String,
    /// Whether the reading was calm (no wind, so no bearing: the served
    /// direction is then the forecast's, not the station's).
    pub calm: bool,
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
    /// (JSON `observation_time`) The reading's time (RFC 3339).
    #[serde(rename(deserialize = "observation_time"), alias = "observed_at")]
    pub observed_at: String,
    /// The distance from the location, km.
    pub distance_km: f64,
    /// The sensor's elevation minus the location's, in the length unit
    /// (positive above).
    pub elevation_diff: Option<f64>,
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
    pub precipitation_rate: Option<f64>,
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
    /// (JSON `alert_count`) The number of alerts.
    #[serde(rename(deserialize = "alert_count"), alias = "count")]
    pub count: u32,
    /// When the alerts were fetched (RFC 3339).
    pub as_of: String,
    /// (JSON `alerts`) The alerts.
    #[serde(default, rename(deserialize = "alerts"), alias = "items")]
    pub items: Vec<ActiveAlert>,
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
    pub elevations: Vec<ElevationSummary>,
}

/// One of a location's elevation points, in the request's length unit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ElevationSummary {
    /// The elevation's ID: the name to pass to
    /// [`ForecastRequest::elevations`](crate::ForecastRequest::elevations),
    /// such as `"summit"`.
    pub name: String,
    /// What the place is called, for display (`"K-1 Lodge"`).
    pub display_name: Option<String>,
    /// (JSON `elevation`) The elevation.
    #[serde(rename(deserialize = "elevation"), alias = "height")]
    pub height: f64,
    /// Whether this is the primary (`Some(true)` on the primary only).
    pub default: Option<bool>,
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
    /// (JSON `location`) The location searched around.
    #[serde(rename(deserialize = "location"), alias = "origin")]
    pub origin: LocationSummary,
    /// The search radius, km.
    pub radius_km: f64,
    /// The number of neighbours.
    pub count: u32,
    /// (JSON `locations`) The neighbours, nearest first.
    #[serde(default, rename(deserialize = "locations"), alias = "items")]
    pub items: Vec<NearbyLocation>,
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
    /// (JSON `locations`) The neighbours, nearest first.
    #[serde(default, rename(deserialize = "locations"), alias = "items")]
    pub items: Vec<NearbyLocation>,
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
    /// (JSON `locations`) The locations.
    #[serde(default, rename(deserialize = "locations"), alias = "items")]
    pub items: Vec<LocationSummary>,
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
    /// The hit's unique key, such as `"or-mt-hood-meadows"` (a dash, not a
    /// slash: for a location method, use `location.key()`).
    pub key: String,
    /// The location.
    #[serde(flatten)]
    pub location: LocationRef,
    /// The county.
    pub county: String,
    /// With a metric [`SearchRequest::sort`](crate::SearchRequest::sort),
    /// the place's value as [`Client::explore`](crate::Client::explore)
    /// ranks it; `None` where it has none.
    pub explore: Option<ExploreValue>,
}

with_meta!(
    Current,
    Alerts,
    NearbyLocations,
    NearbyPoint,
    LocationsPage,
    SearchResults
);
