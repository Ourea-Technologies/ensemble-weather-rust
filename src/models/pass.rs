//! The place summary and the mountain pass forecast of current conditions.

use serde::{Deserialize, Serialize};

use crate::enums::PassStatusLevel;

/// A short plain-language summary of what is coming at a place, written by the server from
/// templates filled with served values (never free text from a model).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PlaceSummary {
    /// One sentence: the most important thing at the place, by priority (an NWS alert,
    /// precipitation starting or stopping soon, snow coming, wind or a large temperature
    /// change, then the day's headline).
    pub short: String,
    /// One to three sentences for the location page, each from a different kind of event.
    #[serde(default)]
    pub long: Vec<String>,
    /// The event behind each sentence of `long`, in the same order (`events[i]` wrote
    /// `long[i]`), so a client can style a sentence (an alert chip, a snow icon) without
    /// parsing text.
    #[serde(default)]
    pub events: Vec<SummaryEvent>,
    /// The served elevation the sentences describe: its display name where the catalog has one,
    /// else its name.
    pub elevation_name: Option<String>,
    /// The height the sentences describe, in the response's length units (m, ft).
    pub elevation: f64,
    /// The init of the forecast run the sentences were written from.
    pub run: String,
    /// When the sentences were written (the request's now); the "updated" stamp.
    pub generated_at: String,
}

/// "Is the pass getting weather?" at the pass's own point and elevation: forecast quantities
/// only, in the response's units.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PassForecast {
    /// The pass's elevation, in the response's length units (m, ft).
    pub elevation: f64,
    /// The pass's elevation in metres, whatever `units` is.
    pub elevation_m: f64,
    /// The first hour of `hours` (the hour containing now); the 24 h and 48 h windows open
    /// here.
    pub from: String,
    /// The snowfall of the next 24 hours.
    pub snow_24h: PassSnow24h,
    /// The chances of the hazards a driver cares about.
    pub chances: PassChances,
    /// Each spell of snow at the pass in the next 48 h, in time order; empty when none.
    #[serde(default)]
    pub snow_windows: Vec<PassSnowWindow>,
    /// The next 48 hours at the pass, from `from`, one per hour.
    #[serde(default)]
    pub hours: Vec<PassHour>,
    /// The best and worst travel windows.
    pub windows: PassTravelWindows,
    /// The road agency's restrictions, when it publishes them.
    pub official_status: Option<PassOfficialStatus>,
    /// The state DOT road-weather stations nearest the pass, nearest first: at most 3, within
    /// 25 km, each with its newest reading in the last 3 h.
    #[serde(default)]
    pub road_sensors: Vec<PassRoadSensor>,
    /// The DOT's own cameras at the pass, in the order the pass's curation lists them.
    #[serde(default)]
    pub cameras: Vec<PassCamera>,
}

/// One event the summary is built from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SummaryEvent {
    /// What the sentence is about.
    pub kind: String,
    /// The phrasing used, `<template>.<variant>` (`snow_coming.v1.3`), stable across releases
    /// so sentences can be counted, reviewed and later translated.
    pub template: String,
    /// The hour the event begins (precipitation starting or stopping, an alert's onset, a pass
    /// event).
    pub at: Option<String>,
    /// When the event ends, where the sentence says so (an alert's end, a snow window's end).
    pub until: Option<String>,
    /// The `alerts[].id` an `alert` event states.
    pub alert_id: Option<String>,
    /// The served numbers the sentence states, unrounded, in the response's units.
    pub values: Option<SummaryValues>,
}

/// The next 24 h of snow at the pass: exactly the default forecast's own
/// `snow_outlook.next_24h` and `snow_outlook.quantiles.next_24h` (unpinned, the nowcast on, the
/// same cycle), so the pass page, the passes board and search state one number.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PassSnow24h {
    /// `snow_outlook.next_24h` (mm, in); null where the forecast does not reach 24 h.
    pub total: Option<f64>,
    /// `snow_outlook.quantiles.next_24h.p10`; null without members covering the window.
    pub p10: Option<f64>,
    /// `snow_outlook.quantiles.next_24h.p90`.
    pub p90: Option<f64>,
    /// The start of the first snow spell (`snow_windows[0].start`) when it begins inside the 24
    /// h; null otherwise.
    pub starts_at: Option<String>,
}

/// Chances at the pass over the next `window_hours` hours, each the share of the forecast's
/// members meeting the event (the `?prob=` machinery), except the gust chance (see its
/// `method`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PassChances {
    /// The window's length, 24.
    pub window_hours: u32,
    /// The window's start (RFC 3339).
    pub from: String,
    /// The window's end (RFC 3339).
    pub to: String,
    /// Snowfall total over the window of 1 in (25.4 mm) or more: `prob=snowfall:ge:<1 in>:24h`.
    pub p_snow_ge_1in: PassChance,
    /// Snowfall total over the window of 6 in (152.4 mm) or more.
    pub p_snow_ge_6in: PassChance,
    /// Some hour of the window in which the member is at or below freezing (`threshold`: 0 C,
    /// 32 F) with precipitation of at least `precipitation_threshold` (0.254 mm, 0.01 in) in
    /// the member's value ending at that hour: a wet road at or below freezing.
    pub p_wet_below_freezing: PassChance,
    /// A gust over 40 mph (17.9 m/s; threshold fixed, does not change with units) in the
    /// window.
    pub p_gust_gt_40mph: PassChance,
}

/// A window of snowfall at the pass.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PassSnowWindow {
    /// When the first snowing hour begins (its `valid_time` minus 1 h).
    pub start: String,
    /// When the last snowing hour ends (its `valid_time`).
    pub end: String,
    /// The sum of the hourly snowfall means over the spell (mm, in), as `snow_outlook` sums its
    /// windows.
    pub total: f64,
    /// The p10 of the members' own totals over the spell (`?totals=` machinery); omitted
    /// without enough members.
    pub p10: Option<f64>,
    /// The p90 of the members' own totals over the spell.
    pub p90: Option<f64>,
    /// The share of members whose own total over the spell is 1 in (25.4 mm) or more; threshold
    /// fixed, does not change with units.
    pub p_ge_1in: Option<f64>,
}

/// One forecast hour at the pass.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PassHour {
    /// The hour's `valid_time`; amounts are over the hour ending then.
    pub time: String,
    /// `ensemble.temperature.mean`.
    pub temperature: f64,
    /// `ensemble.precipitation.mean` (mm, in).
    pub precipitation: f64,
    /// `ensemble.precipitation.probability`, 0-1.
    pub precipitation_probability: Option<f64>,
    /// The hour's served `precipitation_type.type`; omitted on a dry hour.
    pub precipitation_type: Option<String>,
    /// `ensemble.snowfall.mean` (mm, in); 0 without snow.
    pub snowfall: f64,
    /// `ensemble.snow_level.mean` (m, ft); omitted where not served.
    pub snow_level: Option<f64>,
    /// (JSON `snow_level_above_pass`) `snow_level` minus the pass's elevation (m, ft): above 0 the snow level is above the
    /// pass (rain at the pass when wet), below 0 it is under it.
    #[serde(
        rename(deserialize = "snow_level_above_pass"),
        alias = "snow_level_margin"
    )]
    pub snow_level_margin: Option<f64>,
    /// The hour is at or below freezing (temperature mean at or below 0 C, 32 F) and wet:
    /// precipitation of at least 0.254 mm (0.01 in) in this hour or one of the two before it.
    pub freezing_on_wet_road: bool,
    /// `ensemble.wind_speed.mean`.
    pub wind_speed: f64,
    /// `ensemble.wind_gust.mean`; omitted where not served.
    pub wind_gust: Option<f64>,
    /// `wind_gust` is over 40 mph (17.9 m/s); threshold fixed, does not change with units.
    pub gust_over_40mph: bool,
    /// Gusts of 25 mph (11.2 m/s) or more at or below freezing with loose snow to move: snow
    /// falling this hour, 1 in (25.4 mm) or more fallen in the 24 served hours before it, or
    /// snow on the ground at the pass (`snow_on_ground`).
    pub blowing_snow: bool,
}

/// The quietest and the busiest stretch of weather at the pass in the next `horizon_hours`, by
/// one fixed rule on the hours above: an hour has weather when it is snowing (`snowfall` at
/// least 0.04 in, 1 mm), `freezing_on_wet_road`, or `gust_over_40mph`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PassTravelWindows {
    /// The horizon's length in hours, 24.
    pub horizon_hours: u32,
    /// Null when every hour has weather.
    pub least_weather: Option<PassTravelWindow>,
    /// Null when no hour has weather.
    pub most_weather: Option<PassTravelWindow>,
}

/// The DOT's own status for the road over the pass -- chain controls and closures as the DOT
/// publishes them, its wording verbatim.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PassOfficialStatus {
    /// The DOT feed.
    pub source: String,
    /// Who to credit, as the client shows it.
    pub attribution: String,
    /// The DOT's own page for this road, to link out to.
    pub url: String,
    /// How fresh the DOT's feed behind these segments was at Ourea's last read: the time the
    /// DOT wrote the file Ourea last read (Caltrans rewrites it every minute; Ourea reads it
    /// every 5), and the older of two when the segments span two DOT districts.
    pub checked_at: String,
    /// `checked_at` is more than 30 minutes before `as_of`.
    pub stale: bool,
    /// The level.
    pub level: PassStatusLevel,
    /// The DOT's chain-control and closure points on the road over the pass, each direction, in
    /// the pass's curated order.
    #[serde(default)]
    pub segments: Vec<PassStatusSegment>,
}

/// One state DOT road-weather station's newest reading, in the response's units.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PassRoadSensor {
    /// The station's id in its network.
    pub station_id: String,
    /// The station's registry name, else its id.
    pub name: String,
    /// The feed: `rwis` (Caltrans CWWP2 RWIS) or `wsdot`.
    pub source: String,
    /// Who measured it, as the client credits it.
    pub attribution: String,
    /// Straight-line distance from the pass, km.
    pub distance_km: f64,
    /// The station's registry elevation in the response's length units (m, ft).
    pub elevation: Option<f64>,
    /// The station's registry elevation in metres.
    pub elevation_m: Option<f64>,
    /// The reading's time (RFC 3339).
    pub observed_at: String,
    /// `observed_at` is more than 60 minutes before `as_of`.
    pub stale: bool,
    /// The pavement state the sensor reports (NTCIP 1204 `essSurfaceStatus`, as
    /// the current-conditions sensors serve it: `dry`, `trace_moisture`, `wet`,
    /// `chemically_wet`, `ice_warning`, `ice_watch`, `snow_warning`,
    /// `snow_watch`, `absorption`, `dew`, `frost`, `absorption_at_dewpoint`).
    pub pavement_condition: Option<String>,
    /// Pavement surface temperature: °C, °F (imperial) or K (kelvin).
    pub pavement_temp: Option<f64>,
    /// As `pavement_temp`.
    pub subsurface_temp: Option<f64>,
    /// Air temperature, as `pavement_temp`.
    pub temperature: Option<f64>,
    /// m/s or mph (imperial).
    pub wind_speed: Option<f64>,
    /// As `wind_speed`.
    pub wind_gust: Option<f64>,
    /// Degrees clockwise from north.
    pub wind_direction: Option<f64>,
    /// m, or statute miles (imperial).
    pub visibility: Option<f64>,
}

/// One DOT camera, a still image the DOT refreshes about once a minute.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PassCamera {
    /// The id.
    pub id: String,
    /// The DOT's name for the camera.
    pub title: String,
    /// The DOT's current still image.
    pub image_url: String,
    /// The DOT's own page for the camera, to link out to.
    pub url: String,
    /// The image's time (its Last-Modified) when Ourea last checked it, every 15 minutes; the
    /// image at `image_url` may be newer.
    pub captured_at: Option<String>,
    /// `captured_at` is unknown or more than 60 minutes before `as_of`: the camera may be
    /// frozen.
    pub stale: bool,
    /// The source.
    pub source: String,
    /// The attribution.
    pub attribution: String,
}

/// Each number a summary sentence states, as served.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SummaryValues {
    /// The window's headline snowfall total (mm, in): for `snow_coming` the forecast's
    /// `snow_outlook.<snowfall_window>`; for `pass_snow_starts` the window's
    /// `pass.snow_windows[].total`.
    pub snowfall: Option<f64>,
    /// The window's served p10 (`snow_outlook.quantiles.<window>.p10`, or the pass window's
    /// `p10`).
    pub snowfall_p10: Option<f64>,
    /// The window's served p90.
    pub snowfall_p90: Option<f64>,
    /// Which window `snowfall` totals.
    pub snowfall_window: Option<String>,
    /// A day's served high (`daily[].high.mean`) a `day_headline` or `temperature_change`
    /// states.
    pub temperature: Option<f64>,
    /// The served hourly gust mean (`ensemble.wind_gust.mean`) a `wind` sentence states.
    pub wind_gust: Option<f64>,
    /// The served chance (0-1) behind the sentence's likelihood word.
    pub precipitation_probability: Option<f64>,
}

/// The chance of one hazard at the pass.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PassChance {
    /// 0-1 (0.001); null when it cannot be answered (`null_reason`).
    pub probability: Option<f64>,
    /// The event's threshold, in the response's units.
    pub threshold: f64,
    /// For `p_wet_below_freezing`, the precipitation that makes an hour wet, in the response's
    /// units.
    pub precipitation_threshold: Option<f64>,
    /// The members that voted (0 for `hourly_gust_distribution`).
    pub members: u32,
    /// How the chance was computed.
    pub method: String,
    /// Why `probability` is `None`.
    pub null_reason: Option<String>,
}

/// A travel window.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PassTravelWindow {
    /// When the run's first hour begins.
    pub start: String,
    /// When its last hour ends.
    pub end: String,
    /// The sum of the hourly snowfall means over the run (mm, in).
    pub snowfall: f64,
    /// Hours of freezing on a wet road in the window.
    pub freezing_on_wet_road_hours: u32,
    /// The highest hourly gust mean in the run; omitted where no gust is served.
    pub max_wind_gust: Option<f64>,
}

/// One DOT chain-control or closure point, as the DOT publishes it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PassStatusSegment {
    /// The DOT's own id for the point.
    pub id: String,
    /// The route.
    pub route: String,
    /// The direction the point controls, as the DOT names it.
    pub direction: String,
    /// The DOT's name for the point (its trailing field padding removed).
    pub location: String,
    /// (JSON `status`) The DOT's status code, verbatim.
    #[serde(rename(deserialize = "status"), alias = "state")]
    pub state: String,
    /// The level.
    pub level: PassStatusLevel,
    /// (JSON `status_text`) The DOT's own description of the status, verbatim.
    #[serde(rename(deserialize = "status_text"), alias = "text")]
    pub text: String,
    /// When the DOT set this status.
    pub updated_at: String,
    /// Latitude.
    pub lat: f64,
    /// Longitude.
    pub lon: f64,
    /// The DOT's own report of the road surface there, verbatim, where it gives one beside the
    /// restriction (WSDOT, ODOT).
    pub road_condition: Option<String>,
    /// The DOT's own report of the weather there, verbatim (WSDOT, ODOT).
    pub weather_condition: Option<String>,
}
