//! Forecasts: the hourly series, daily summaries, probabilities and totals.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::enums::{
    AstronomicalSeason, ClampedOption, Condition, ConditionIcon, ElevationSource,
    FreezingLevelKind, GapFlowStatus, InversionKind, LocationType, MembersView, MoonPhase,
    NowcastSource, NullReason, PrecipKind, SnowCharacter, StormOrientation, WindStatus,
};
use crate::meta::{ResponseMeta, with_meta};
use crate::models::{CurrentConditions, ElevationSummary, MemberPlume, ResponseAttribution};
use crate::probability::{Aggregation, Op, Variable};
use crate::units::{Unit, Units};

/// The API's one distribution shape: an ensemble statistic in the request's
/// units, with percentiles over the members.
///
/// Every field is optional; `None` is absent, never 0. The SDK never sums,
/// averages or interpolates quantiles: ask the server instead, with a
/// [`Chance`](crate::Chance) for a probability or a [`Total`](crate::Total)
/// for a window total.
///
/// Wind direction is not a `Quantiles`: it is a [`CircularStats`].
///
/// ```
/// # use ensemble_weather::Quantiles;
/// fn show(q: &Quantiles) {
///     if let Some(r) = q.range80() {
///         println!("80% range {r}"); // p10–p90 as the server sent it
///     }
///     if let Some(m) = q.median() {
///         println!("median {m}"); // p50: the median, not "the forecast"
///     }
/// }
/// ```
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Quantiles {
    /// The ensemble mean.
    pub mean: Option<f64>,
    /// The 10th percentile.
    pub p10: Option<f64>,
    /// The 25th percentile.
    pub p25: Option<f64>,
    /// The 50th percentile (the median).
    pub p50: Option<f64>,
    /// The 75th percentile.
    pub p75: Option<f64>,
    /// The 90th percentile.
    pub p90: Option<f64>,
    /// The lowest member.
    pub min: Option<f64>,
    /// The highest member.
    pub max: Option<f64>,
    /// The chance of occurrence, 0..1 (hourly precipitation and snowfall only).
    pub probability: Option<f64>,
}

impl Quantiles {
    /// The ensemble median (p50). It is the median, not "the forecast": the
    /// forecast is the whole distribution.
    #[must_use]
    pub fn median(&self) -> Option<f64> {
        self.p50
    }

    /// The central 80 % range, p10 to p90, as the server sent it; `None` when
    /// either end is absent.
    ///
    /// Only the served pairs (p10–p90 and p25–p75) have accessors: any other
    /// range would need interpolation, which the SDK never does.
    #[must_use]
    pub fn range80(&self) -> Option<Interval> {
        Some(Interval {
            low: self.p10?,
            high: self.p90?,
        })
    }

    /// The central 50 % range, p25 to p75, as the server sent it; `None` when
    /// either end is absent.
    #[must_use]
    pub fn range50(&self) -> Option<Interval> {
        Some(Interval {
            low: self.p25?,
            high: self.p75?,
        })
    }
}

/// A pair of percentiles the server served.
///
/// ```
/// # use ensemble_weather::Interval;
/// # fn show(r: Interval) {
/// println!("{r}"); // "-4–0"
/// println!("{:.1} to {:.1}", r.low, r.high);
/// # }
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct Interval {
    /// The lower percentile.
    pub low: f64,
    /// The upper percentile.
    pub high: f64,
}

impl fmt::Display for Interval {
    /// `low–high`; a precision (`{:.1}`) applies to both ends.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match f.precision() {
            Some(p) => write!(f, "{:.p$}–{:.p$}", self.low, self.high),
            None => write!(f, "{}–{}", self.low, self.high),
        }
    }
}

/// A forecast for a named location or a point.
///
/// ```no_run
/// # async fn run() -> Result<(), ensemble_weather::Error> {
/// let client = ensemble_weather::Client::new()?;
/// let forecast = client.forecast("or/mt-hood-meadows").await?;
/// for hour in &forecast.series.timeseries {
///     println!("{} {:?}", hour.valid_time, hour.ensemble.temperature.range80());
/// }
/// # Ok(()) }
/// ```
///
/// Two fields carry SDK names (BEHAVIOR.md 3.0, no stutter): `series` is the
/// JSON `forecast` and `elevations` the JSON `elevation_forecasts`. Every
/// model decodes the wire names and serialises the SDK names (which it
/// decodes too), so a serialised value reads back.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Forecast {
    /// How the response was obtained (rate limits, request ID, revalidation).
    #[serde(skip)]
    pub meta: ResponseMeta,
    /// The server's fixed thresholds behind the hourly and daily probabilities.
    pub thresholds: Option<ProbabilityThresholds>,
    /// The answers to the request's [`Chance`](crate::Chance) terms, in the
    /// caller's order (duplicates share one answer). Empty when none were
    /// asked for or the server sent none.
    #[serde(default)]
    pub probabilities: Vec<ProbabilityResult>,
    /// The answers to the request's [`Total`](crate::Total) terms, in the
    /// caller's order. Empty when none were asked for or the server sent none.
    #[serde(default)]
    pub window_totals: Vec<WindowTotal>,
    /// The options the server clamped to the caller's tier.
    pub applied: Option<AppliedOptions>,
    /// The location, with the elevation served.
    pub location: ForecastLocation,
    /// The model run behind the forecast.
    pub model_run: ModelRunInfo,
    /// The unit system of every value in this response.
    pub units: Units,
    /// (JSON `forecast`) The hourly series and daily summaries.
    #[serde(rename(deserialize = "forecast"), alias = "series")]
    pub series: ForecastData,
    /// (JSON `elevation_forecasts`) Forecasts at the location's extra
    /// elevations, when asked for.
    #[serde(
        default,
        rename(deserialize = "elevation_forecasts"),
        alias = "elevations"
    )]
    pub elevations: Vec<ElevationForecast>,
    /// Whether the live nowcast layer is baked in.
    pub nowcast_applied: bool,
    /// Live inputs this response was built without.
    #[serde(default)]
    pub nowcast_unavailable: Vec<NowcastSource>,
    /// Which per-member values are present, when any are.
    pub members_view: Option<MembersView>,
    /// The Columbia Gorge gap-flow status, for locations in the Gorge.
    pub gap_flow: Option<GapFlow>,
    /// Observation sensors near the forecast point.
    #[serde(default)]
    pub nearest_sensors: Vec<NearestSensor>,
    /// The grid point a coordinate forecast was served from.
    pub point: Option<PointInfo>,
    /// Current conditions, when the response carries them.
    pub current: Option<CurrentConditions>,
    /// The avalanche center's danger rating for the zone the point is in.
    pub avalanche: Option<AvalancheForecast>,
    /// What the snowpack model (beta) states is on the ground now.
    pub snow_on_ground: Option<SnowOnGround>,
    /// The data sources; carry `credit` with redistributed output.
    pub attribution: ResponseAttribution,
}

/// A forecast's location: the named location (or the point of a coordinate
/// request), the elevation the forecast is served at, and its other
/// elevation points.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ForecastLocation {
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
    /// The elevation the series is served at, in the request's length unit.
    pub elevation: f64,
    /// The location's elevation points, the primary first (named locations
    /// with more than one).
    #[serde(default)]
    pub elevations: Vec<ElevationSummary>,
}

impl ForecastLocation {
    /// The `"state/slug"` form the location methods take; `None` for a
    /// coordinate request's point.
    #[must_use]
    pub fn key(&self) -> Option<String> {
        self.slug
            .as_ref()
            .map(|slug| format!("{}/{slug}", self.state))
    }
}

/// The avalanche center's danger rating for the forecast zone a location's
/// point is in, from avalanche.org's public map layer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct AvalancheForecast {
    /// The avalanche center's short ID.
    pub center_id: String,
    /// The center's name.
    pub center_name: String,
    /// avalanche.org's ID for the forecast zone.
    pub zone_id: i64,
    /// The zone's name.
    pub zone_name: String,
    /// The North American Public Avalanche Danger Scale level, 1-5; present
    /// only when `status` is `rated`.
    pub danger_level: Option<u32>,
    /// The scale's English name for `danger_level`; `None` exactly when
    /// `danger_level` is.
    pub danger_label: Option<String>,
    /// `rated` for a current rating; other values say why there is none.
    pub status: String,
    /// When the center's current product took effect (RFC 3339), when stated.
    pub valid_from: Option<String>,
    /// When the center's current product expires (RFC 3339), when stated.
    pub valid_until: Option<String>,
    /// (JSON `forecast_url`) The center's full forecast for the zone.
    #[serde(rename(deserialize = "forecast_url"), alias = "url")]
    pub url: String,
    /// The center's site.
    pub center_url: String,
    /// The credit to show with the rating.
    pub attribution: String,
}

/// What the snowpack model (beta) states is on the ground at a spot at one
/// hour.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SnowOnGround {
    /// Always `true` while the product is a beta; fields may be added.
    pub beta: bool,
    /// Snow depth: cm, or in (imperial).
    pub depth: f64,
    /// `measured`: the run started from a snow-depth sensor at the spot.
    pub basis: String,
    /// The surface class at the hour (an open set; the snowpack route's
    /// `definitions.surface` has the text of each).
    pub surface: Option<String>,
    /// The hour the depth is for (RFC 3339).
    pub valid_time: String,
    /// When the snowpack run that produced it finished (RFC 3339).
    pub generated_at: String,
}

/// One probability term, answered, with its reference class: the variable,
/// comparison, threshold, window and member count.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ProbabilityResult {
    /// The canonical term the SDK sent, echoed by the server; the SDK matches
    /// each answer to the caller's [`Chance`](crate::Chance) by it.
    pub term: String,
    /// The variable.
    pub variable: Variable,
    /// The comparison.
    pub op: Op,
    /// The threshold, in the request's units at the served precision.
    pub threshold: f64,
    /// The threshold's unit (set by the SDK from the response's units).
    #[serde(skip)]
    pub threshold_unit: Option<Unit>,
    /// The window's first hour.
    pub window_start_hours: u32,
    /// The window's end hour.
    pub window_end_hours: u32,
    /// How the window's hours are combined.
    pub aggregation: Aggregation,
    /// The window's first instant (RFC 3339); `None` when not covered.
    pub from: Option<String>,
    /// The window's exclusive end (RFC 3339); `None` when not covered.
    pub to: Option<String>,
    /// (JSON `probability`) The share of members meeting the term, 0..1.
    /// `None` when the server could not answer (see `null_reason`): absent,
    /// never 0.
    #[serde(rename(deserialize = "probability"), alias = "value")]
    pub value: Option<f64>,
    /// The number of members that voted.
    pub members: u32,
    /// Why `probability` is `None`; `None` when it is set.
    pub null_reason: Option<NullReason>,
}

/// The distribution of each member's own total over a window.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct WindowTotal {
    /// The canonical term the SDK sent, echoed by the server; the SDK matches
    /// each answer to the caller's [`Total`](crate::Total) by it.
    pub term: String,
    /// The variable.
    pub variable: Variable,
    /// The quantiles' unit (set by the SDK from the response's units).
    #[serde(skip)]
    pub unit: Option<Unit>,
    /// (JSON `window_start_hours`) The window's first hour.
    #[serde(rename(deserialize = "window_start_hours"), alias = "start_hours")]
    pub start_hours: u32,
    /// (JSON `window_end_hours`) The window's end hour.
    #[serde(rename(deserialize = "window_end_hours"), alias = "end_hours")]
    pub end_hours: u32,
    /// How each member's hours were combined (`total` for precipitation and
    /// snowfall).
    pub aggregation: Aggregation,
    /// The window's first instant (RFC 3339); `None` when not covered.
    pub from: Option<String>,
    /// The window's exclusive end (RFC 3339); `None` when not covered.
    pub to: Option<String>,
    /// The distribution; `None` when the server could not answer (see
    /// `null_reason`).
    pub quantiles: Option<Quantiles>,
    /// The number of members.
    pub members: u32,
    /// Why `quantiles` is `None`; `None` when it is set.
    pub null_reason: Option<NullReason>,
    /// How the distribution was computed: `"members"` (from each member's
    /// own total) or `"ensemble_moments"`; `None` with no distribution.
    pub method: Option<String>,
}

/// The fixed thresholds behind the hourly and daily probabilities, in the
/// request's units.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[allow(clippy::struct_field_names)] // the API's names
pub struct ProbabilityThresholds {
    /// Behind each hour's precipitation probability.
    pub hourly_precipitation: f64,
    /// Behind each hour's snowfall probability.
    pub hourly_snowfall: f64,
    /// (JSON `daily_precipitation_probability`) Behind each day's
    /// `precipitation_probability`.
    #[serde(
        rename(deserialize = "daily_precipitation_probability"),
        alias = "daily_precipitation"
    )]
    pub daily_precipitation: f64,
    /// (JSON `daily_precipitation_occurrence_probability`) Behind each day's
    /// `precipitation_occurrence_probability`.
    #[serde(
        rename(deserialize = "daily_precipitation_occurrence_probability"),
        alias = "daily_precipitation_occurrence"
    )]
    pub daily_precipitation_occurrence: f64,
    /// (JSON `daily_snow_probability`) Behind each day's `snow_probability`.
    #[serde(rename(deserialize = "daily_snow_probability"), alias = "daily_snow")]
    pub daily_snow: f64,
}

/// The horizon actually served.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct AppliedOptions {
    /// The hours served.
    pub max_hours: Option<u32>,
    /// The days served.
    pub max_days: Option<u32>,
    /// The options the server reduced.
    #[serde(default)]
    pub clamped: Vec<ClampedOption>,
}

/// The Columbia Gorge gap-flow status at the first served hour.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct GapFlow {
    /// The category.
    pub status: Option<GapFlowStatus>,
    /// The pressure difference across the Gorge, hPa.
    pub pressure_diff_hpa: Option<f64>,
    /// The wind multiplier applied.
    pub wind_multiplier: Option<f64>,
    /// The flow direction.
    pub direction: Option<String>,
}

/// The grid point a coordinate forecast was served from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PointInfo {
    /// The latitude asked for.
    pub requested_lat: f64,
    /// The longitude asked for.
    pub requested_lon: f64,
    /// The grid point's latitude.
    pub lat: f64,
    /// The grid point's longitude.
    pub lon: f64,
    /// The grid resolution, degrees.
    pub resolution_deg: f64,
    /// The elevation served at, in the request's length unit.
    pub elevation: f64,
    /// Where the elevation came from.
    pub elevation_source: ElevationSource,
    /// The state the point is in.
    pub state: String,
}

/// An observation sensor near the forecast point.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct NearestSensor {
    /// The station's ID.
    pub station_id: String,
    /// The station's name.
    pub name: String,
    /// The observation network.
    pub source: String,
    /// The distance, km.
    pub distance_km: f64,
    /// The elevation difference.
    pub elevation_diff: f64,
    /// The terrain-weighted distance, km.
    pub effective_distance_km: f64,
    /// The latest observation (RFC 3339).
    pub observation_time: String,
}

/// The model run behind a forecast.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ModelRunInfo {
    /// The reference init time (RFC 3339).
    pub initialized_at: String,
    /// The ensemble members in all.
    pub total_members: u32,
    /// Each contributing model.
    #[serde(default)]
    pub models: Vec<ModelEntry>,
}

/// One model contributing to a run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ModelEntry {
    /// The model's ID, such as `"ifs_ens"`.
    pub id: String,
    /// The model's own cycle (RFC 3339).
    pub cycle: Option<String>,
    /// The model's member count.
    pub members: Option<u32>,
    /// The members shown in the plume (the model's display set), when it
    /// shows fewer than it has.
    #[serde(default)]
    pub display_members: Vec<u32>,
    /// `"borrowed_from_deterministic"` when the shown members' snowfall is
    /// the deterministic run's.
    pub display_members_snowfall: Option<String>,
}

/// The hourly series and daily summaries.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ForecastData {
    /// The hours past the first served hour.
    pub max_hours: u32,
    /// The spacing of the series, hours.
    pub interval_hours: u32,
    /// The hourly series, from the current hour (or the run's init with
    /// `include_past`). Empty with `members=plume`, which serves `plume`
    /// instead.
    #[serde(default)]
    pub timeseries: Vec<TimeSeriesPoint>,
    /// The daily summaries, one per local date.
    #[serde(default)]
    pub daily: Vec<DailyForecast>,
    /// Snow totals for common display windows.
    pub snow_outlook: Option<SnowOutlook>,
    /// Liquid-equivalent totals over the same windows as `snow_outlook`.
    pub precipitation_outlook: Option<PrecipOutlook>,
    /// The members in columns (with [`Members::Plume`](crate::Members::Plume)).
    pub plume: Option<MemberPlume>,
}

/// Liquid-equivalent totals for common display windows, as the server
/// computes them (sums of the hourly means; `None` when not covered).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PrecipOutlook {
    /// The next 24 hours.
    pub next_24h: Option<f64>,
    /// The next 48 hours.
    pub next_48h: Option<f64>,
    /// The next 72 hours.
    pub next_72h: Option<f64>,
    /// The next 7 days.
    pub next_7d: Option<f64>,
    /// The distributions of each member's own window total.
    pub quantiles: Option<SnowOutlookQuantiles>,
}

/// Snow totals for common display windows, as the server computes them.
///
/// For the distribution of a window's total, use `quantiles` or ask for a
/// [`Total`](crate::Total).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SnowOutlook {
    /// The next 24 hours.
    pub next_24h: Option<f64>,
    /// The next 48 hours.
    pub next_48h: Option<f64>,
    /// The next 72 hours.
    pub next_72h: Option<f64>,
    /// The next 7 days.
    pub next_7d: Option<f64>,
    /// Tonight, 4 pm to 9 am.
    pub overnight_4pm_9am: Option<f64>,
    /// The distributions of each member's own window total.
    pub quantiles: Option<SnowOutlookQuantiles>,
}

/// The distributions of each member's own window total.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SnowOutlookQuantiles {
    /// The next 24 hours.
    pub next_24h: Option<Quantiles>,
    /// The next 48 hours.
    pub next_48h: Option<Quantiles>,
    /// The next 72 hours.
    pub next_72h: Option<Quantiles>,
    /// The next 7 days.
    pub next_7d: Option<Quantiles>,
}

/// One forecast hour.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct TimeSeriesPoint {
    /// The hour (RFC 3339).
    pub valid_time: String,
    /// Hours since the run's init.
    pub lead_time_hours: i32,
    /// Whether the hour is at night.
    pub is_night: bool,
    /// Whether precipitation was measured in this hour: a radar accumulation
    /// was published for this clock hour and its precipitation is that.
    pub measured: bool,
    /// The condition code.
    pub condition: Condition,
    /// The condition, for display.
    pub condition_text: String,
    /// The icon hint.
    pub condition_icon: ConditionIcon,
    /// Whether gusts are notable.
    pub gust_notable: Option<bool>,
    /// The precipitation type with its probabilities.
    pub precipitation_type: Option<PrecipType>,
    /// A detected inversion or cold pool.
    pub inversion: Option<InversionInfo>,
    /// The freezing level.
    pub freezing_level: Option<FreezingLevel>,
    /// The ensemble distributions.
    pub ensemble: EnsembleStats,
    /// Post-processing diagnostics (with `diagnostics=provenance` only).
    pub provenance: Option<HourProvenance>,
    /// Per-model statistics (with `models`, Pro).
    #[serde(default)]
    pub model_means: Vec<ModelMean>,
    /// Per-member values, one entry per model (with `members`).
    #[serde(default)]
    pub members: Vec<ModelMembers>,
    /// The distributions of the members' running totals (with `members`).
    pub members_cumulative: Option<MembersCumulative>,
    /// The members' spread at this hour (with `members`).
    pub members_spread: Option<MembersSpread>,
    /// The distributions of the members' 6-hour totals ending at this hour
    /// (with `members`, at 00/06/12/18Z).
    pub members_6h: Option<MembersCumulative>,
    /// The server's running total of hourly precipitation means.
    pub cumulative_precipitation: Option<f64>,
    /// The server's running total of hourly snowfall means.
    pub cumulative_snowfall: Option<f64>,
    /// The character of the hour's snow, when it snows.
    pub snow_character: Option<SnowCharacter>,
    /// The hour's mean precipitation split by phase.
    pub precipitation_by_type: Option<PrecipitationByType>,
}

/// The members' spread at an hour for the plume's instantaneous variables.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct MembersSpread {
    /// Temperature.
    pub temperature: Option<MemberQuantiles>,
    /// Snow level.
    pub snow_level: Option<MemberQuantiles>,
    /// Snow level on the NWS (wet-bulb) scale.
    pub snow_level_wet_bulb: Option<MemberQuantiles>,
    /// Snow-to-liquid ratio over the members snowing at this hour.
    pub slr: Option<MemberQuantiles>,
    /// Wind speed.
    pub wind_speed: Option<MemberQuantiles>,
}

/// An hour's mean precipitation split by phase, in the precipitation unit;
/// the four add up to the hour's mean (to the served precision).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PrecipitationByType {
    /// Rain.
    pub rain: f64,
    /// The liquid that falls as snow (not snow depth: that is
    /// `ensemble.snowfall`).
    pub snow: f64,
    /// A rain and snow mix.
    pub rain_snow_mix: f64,
    /// Liquid that freezes on contact.
    pub freezing_rain: f64,
}

/// The post-processing rules that shaped each variable this hour. A research
/// diagnostic, not for display.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct HourProvenance {
    /// Whether the hour was densified.
    pub densified: Option<bool>,
    /// The variables the `ngr` rule touched.
    #[serde(default)]
    pub ngr: Vec<String>,
    /// The variables the `fallback` rule touched.
    #[serde(default)]
    pub fallback: Vec<String>,
    /// The variables the `borrowed` rule touched.
    #[serde(default)]
    pub borrowed: Vec<String>,
    /// The variables the `spread_floored` rule touched.
    #[serde(default)]
    pub spread_floored: Vec<String>,
    /// The variables the `floor_clamped` rule touched.
    #[serde(default)]
    pub floor_clamped: Vec<String>,
    /// The variables the `bucket_split` rule touched.
    #[serde(default)]
    pub bucket_split: Vec<String>,
    /// The variables the `distribution_parameters` rule touched.
    #[serde(default)]
    pub distribution_parameters: Vec<String>,
    /// The variables the `spread_interpolated` rule touched.
    #[serde(default)]
    pub spread_interpolated: Vec<String>,
    /// The variables the `interpolated_contributor` rule touched.
    #[serde(default)]
    pub interpolated_contributor: Vec<String>,
    /// The variables the `moment_matched_family` rule touched.
    #[serde(default)]
    pub moment_matched_family: Vec<String>,
    /// The variables the `lapse_profile` rule touched.
    #[serde(default)]
    pub lapse_profile: Vec<String>,
    /// The variables the `lapse_climatology` rule touched.
    #[serde(default)]
    pub lapse_climatology: Vec<String>,
    /// The variables the `lapse_prism` rule touched.
    #[serde(default)]
    pub lapse_prism: Vec<String>,
    /// The variables the `spread_clamped_low` rule touched.
    #[serde(default)]
    pub spread_clamped_low: Vec<String>,
    /// The variables the `spread_clamped_high` rule touched.
    #[serde(default)]
    pub spread_clamped_high: Vec<String>,
    /// The variables the `gust_dressed` rule touched.
    #[serde(default)]
    pub gust_dressed: Vec<String>,
}

/// A clockwise arc of directions, degrees.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Sector {
    /// The arc's start.
    pub from: f64,
    /// The arc's end (may be less than `from`: the arc crosses north).
    pub to: f64,
}

/// Wind direction as a circular distribution, degrees clockwise from north.
///
/// Each sector is a clockwise arc, so its `to` may be numerically less than
/// its `from` (the arc crosses north). The SDK does not "fix" this.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CircularStats {
    /// The members' resultant-vector mean direction, in `[0, 360)`.
    pub mean: f64,
    /// The members' angular spread around `mean`; `None` on a single-member
    /// hour.
    pub stddev: Option<f64>,
    /// The central 50 % of the members (25th to 75th circular percentile).
    pub sector_50: Sector,
    /// The central 90 % of the members (5th to 95th circular percentile).
    pub sector_90: Sector,
}

/// One model's member values at an hour.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ModelMembers {
    /// The model's ID.
    pub id: String,
    /// (JSON `members`) One entry per member.
    #[serde(default, rename(deserialize = "members"), alias = "items")]
    pub items: Vec<MemberForecast>,
}

/// One ensemble member's values at an hour. The plume view (Free) withholds
/// wind and SLR.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct MemberForecast {
    /// (JSON `member_id`) The member's index within its model.
    #[serde(rename(deserialize = "member_id"), alias = "id")]
    pub id: u32,
    /// This member's temperature.
    pub temperature: Option<f64>,
    /// This member's precipitation.
    pub precipitation: Option<f64>,
    /// This member's precipitation hours.
    pub precipitation_hours: Option<u32>,
    /// This member's wind speed.
    pub wind_speed: Option<f64>,
    /// This member's wind direction.
    pub wind_direction: Option<f64>,
    /// This member's snow level.
    pub snow_level: Option<f64>,
    /// This member's snow level on the NWS (wet-bulb) scale.
    pub snow_level_wet_bulb: Option<f64>,
    /// This member's snow-to-liquid ratio.
    pub slr: Option<f64>,
    /// This member's snowfall.
    pub snowfall: Option<f64>,
    /// This member's cumulative precipitation.
    pub cumulative_precipitation: Option<f64>,
    /// This member's cumulative snowfall.
    pub cumulative_snowfall: Option<f64>,
}

/// The distributions of the members' running (or 6-hour) totals.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct MembersCumulative {
    /// Precipitation.
    pub precipitation: Option<MemberQuantiles>,
    /// Snowfall.
    pub snowfall: Option<MemberQuantiles>,
}

/// A distribution over the ensemble members: the [`Quantiles`] shape, with
/// every statistic but `p50` and `probability` always set.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct MemberQuantiles {
    /// The members' mean.
    pub mean: f64,
    /// The 10th percentile.
    pub p10: f64,
    /// The 25th percentile.
    pub p25: f64,
    /// The members' median.
    pub p50: Option<f64>,
    /// The 75th percentile.
    pub p75: f64,
    /// The 90th percentile.
    pub p90: f64,
    /// The lowest member.
    pub min: f64,
    /// The highest member.
    pub max: f64,
    /// The chance of occurrence, 0..1, where the use says.
    pub probability: Option<f64>,
}

impl MemberQuantiles {
    /// The central 80 % range, p10 to p90, as the server sent it.
    #[must_use]
    pub fn range80(&self) -> Interval {
        Interval {
            low: self.p10,
            high: self.p90,
        }
    }

    /// The central 50 % range, p25 to p75, as the server sent it.
    #[must_use]
    pub fn range50(&self) -> Interval {
        Interval {
            low: self.p25,
            high: self.p75,
        }
    }
}

/// A detected temperature inversion or cold pool.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct InversionInfo {
    /// The kind.
    pub kind: InversionKind,
    /// The depth.
    pub depth: f64,
    /// The strength.
    pub strength: f64,
}

/// An hour's precipitation type with its probabilities.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PrecipType {
    /// The most likely type.
    #[serde(rename = "type")]
    pub kind: PrecipKind,
    /// The chance of snow, 0..1.
    pub snow_probability: f64,
    /// The chance of rain, 0..1.
    pub rain_probability: f64,
}

/// An hour's ensemble distributions, in the request's units.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct EnsembleStats {
    /// The temperature distribution.
    pub temperature: Quantiles,
    /// The feels like distribution.
    pub feels_like: Option<Quantiles>,
    /// The dewpoint distribution.
    pub dewpoint: Option<Quantiles>,
    /// The precipitation distribution.
    pub precipitation: Quantiles,
    /// The wind speed distribution.
    pub wind_speed: Quantiles,
    /// The wind gust distribution.
    pub wind_gust: Option<Quantiles>,
    /// The circular distribution of the members' wind directions.
    pub wind_direction: CircularStats,
    /// The snow level distribution.
    pub snow_level: Option<Quantiles>,
    /// The snow level on the NWS scale (the height of the 0.5 °C wet-bulb).
    pub snow_level_wet_bulb: Option<Quantiles>,
    /// The snow-to-liquid ratio distribution.
    pub slr: Option<Quantiles>,
    /// The snowfall distribution.
    pub snowfall: Option<Quantiles>,
    /// The cloud cover distribution.
    pub cloud_cover: Option<Quantiles>,
    /// The skin temperature distribution (with `include("skin_temperature")`).
    pub skin_temperature: Option<Quantiles>,
    /// The mean sea-level pressure distribution (with `include("mslp")`).
    pub mslp: Option<Quantiles>,
    /// The CAPE distribution.
    pub cape: Option<Quantiles>,
    /// The relative humidity distribution.
    pub relative_humidity: Option<Quantiles>,
}

/// An hour's freezing level. `kind` says which fields are set: `height` (and
/// `above_freezing_layer_aloft`) for [`FreezingLevelKind::Height`], `top` for
/// [`FreezingLevelKind::AboveProfile`], neither for
/// [`FreezingLevelKind::BelowSurface`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct FreezingLevel {
    /// Which fields are set.
    pub kind: FreezingLevelKind,
    /// The freezing height.
    pub height: Option<f64>,
    /// Whether an above-freezing layer sits aloft.
    pub above_freezing_layer_aloft: Option<bool>,
    /// The top of the profile.
    pub top: Option<f64>,
}

/// One model's statistics at an hour.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ModelMean {
    /// The model's ID.
    pub id: String,
    /// The temperature distribution.
    pub temperature: Option<Quantiles>,
    /// The dewpoint distribution.
    pub dewpoint: Option<Quantiles>,
    /// The precipitation distribution.
    pub precipitation: Option<Quantiles>,
    /// The wind speed distribution.
    pub wind_speed: Option<Quantiles>,
    /// The circular distribution of the wind direction.
    pub wind_direction: Option<CircularStats>,
    /// The snow level distribution.
    pub snow_level: Option<Quantiles>,
    /// The snow level on the NWS (wet-bulb) scale.
    pub snow_level_wet_bulb: Option<Quantiles>,
    /// The snow-to-liquid ratio distribution.
    pub slr: Option<Quantiles>,
    /// The snowfall distribution.
    pub snowfall: Option<Quantiles>,
    /// Whether the model's values were interpolated in time.
    pub interpolated: Option<bool>,
}

/// One local calendar date.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct DailyForecast {
    /// The local date, `YYYY-MM-DD`, in the location's timezone.
    pub date: String,
    /// The condition code.
    pub condition: Condition,
    /// The condition, for display.
    pub condition_text: String,
    /// The icon hint.
    pub condition_icon: ConditionIcon,
    /// The precipitation type; `None` when the day is unlikely to precipitate.
    pub precipitation_type: Option<PrecipKind>,
    /// The day's high.
    pub high: Quantiles,
    /// The day's low.
    pub low: Quantiles,
    /// The feels-like high.
    pub feels_like_high: Option<Quantiles>,
    /// The feels-like low.
    pub feels_like_low: Option<Quantiles>,
    /// The distribution of each member's own day total.
    pub total_precipitation: Quantiles,
    /// The distribution of each member's own day snowfall.
    pub total_snowfall: Option<Quantiles>,
    /// P(day total ≥ the daily precipitation threshold).
    pub precipitation_probability: f64,
    /// P(day total ≥ the occurrence threshold).
    pub precipitation_occurrence_probability: f64,
    /// P(day snowfall ≥ the daily snow threshold).
    pub snow_probability: f64,
    /// The mean snow level.
    pub snow_level_mean: Option<f64>,
    /// The snow level's distribution.
    pub snow_level: Option<Quantiles>,
    /// The day total among the wet members.
    pub total_precipitation_if_wet: Option<Quantiles>,
    /// The day snowfall among the wet members.
    pub total_snowfall_if_wet: Option<Quantiles>,
    /// The precipitation headline, for display.
    pub precipitation_display: PrecipDisplay,
    /// The mean cloud cover.
    pub cloud_cover_mean: Option<f64>,
    /// The day's wind.
    pub wind: Option<DailyWind>,
    /// How the storm's snow is layered.
    pub storm_orientation: Option<StormOrientation>,
    /// The orientation, for display.
    pub storm_orientation_description: Option<String>,
    /// Sun and moon.
    pub astro: DailyAstro,
    /// The date's temperature climatology.
    pub temperature_climatology: Option<DayClimatology>,
    /// For today's row, the instant from which the day totals are forecast
    /// (RFC 3339); earlier hours are measured (`measured_precipitation`).
    pub covered_from: Option<String>,
    /// The precipitation measured so far today, when any is.
    pub measured_precipitation: Option<MeasuredPrecipitation>,
    /// The character of the day's snow, when it snows.
    pub snow_character: Option<SnowCharacter>,
    /// The day's snow-to-liquid ratio.
    pub slr: Option<f64>,
    /// The snow level over the day's wet hours.
    pub snow_level_range: Option<SnowLevelRange>,
    /// The day's snow level on the NWS (wet-bulb) scale.
    pub snow_level_wet_bulb: Option<Quantiles>,
    /// The wet-bulb snow level over the day's wet hours.
    pub snow_level_wet_bulb_range: Option<SnowLevelRange>,
    /// The day's climate normal at the forecast point.
    pub normal: Option<DayNormal>,
    /// The served headline minus the normal.
    pub departure: Option<DayDeparture>,
    /// Where the served headline falls in the date's climatology.
    pub percentile: Option<DayPercentile>,
}

/// The precipitation measured so far on a day.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct MeasuredPrecipitation {
    /// The amount, in the precipitation unit.
    pub amount: f64,
    /// Measured through this instant (RFC 3339).
    pub through: String,
}

/// A day's snow level over its wet hours, in the length unit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SnowLevelRange {
    /// The lowest hourly snow level among the wet hours.
    pub low: f64,
    /// The highest hourly snow level among the wet hours.
    pub high: f64,
    /// The wet hours' snow level, weighted by their precipitation.
    pub mean: f64,
}

/// A day's climate normal at the forecast point, in the request's units.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct DayNormal {
    /// The mean local-calendar-day maximum temperature.
    pub high: Option<f64>,
    /// The mean local-calendar-day minimum temperature.
    pub low: Option<f64>,
    /// The mean local-calendar-day precipitation, dry days included.
    pub precipitation: Option<f64>,
    /// A climatology station's own record, or the ERA5 cell of the point.
    pub source: String,
    /// The distance from the point to the station (or cell centre), km.
    pub distance_km: f64,
}

/// The served headline minus the normal: `high.p50 - normal.high` and
/// `low.p50 - normal.low`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct DayDeparture {
    /// The high's departure.
    pub high: Option<f64>,
    /// The low's departure.
    pub low: Option<f64>,
}

/// Where the served headline (`p50`) falls in the date's climatological
/// distribution, as a percentile 0-100.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct DayPercentile {
    /// The high's percentile.
    pub high: Option<f64>,
    /// The low's percentile.
    pub low: Option<f64>,
    /// The precipitation's percentile.
    pub precipitation: Option<f64>,
}

/// A day's precipitation headline, for display.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PrecipDisplay {
    /// The type.
    pub kind: PrecipKind,
    /// The chance, 0..1.
    pub chance: f64,
    /// The amount.
    pub amount: Option<f64>,
    /// The day's snowfall headline, in the snowfall unit.
    pub snow_amount: Option<f64>,
}

/// A day's wind, from gusts when available.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct DailyWind {
    /// The category.
    pub status: Option<WindStatus>,
    /// The expected peak.
    pub peak_speed: Option<f64>,
    /// The 90th-percentile peak ("gusts could reach").
    pub peak_speed_p90: Option<f64>,
    /// Whether `peak_speed` is from gusts.
    pub is_gust: Option<bool>,
    /// The peak sustained wind.
    pub sustained_speed: Option<f64>,
}

/// A day's sun and moon.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct DailyAstro {
    /// Sunrise (RFC 3339), `None` in polar day or night.
    pub sunrise: Option<String>,
    /// Sunset (RFC 3339).
    pub sunset: Option<String>,
    /// The moon phase.
    pub moon_phase: MoonPhase,
    /// The lit fraction, 0..1.
    pub moon_illumination: f64,
    /// The astronomical season.
    pub astronomical_season: AstronomicalSeason,
}

/// A date's temperature climatology.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct DayClimatology {
    /// The mean.
    pub mean: f64,
    /// The standard deviation.
    pub std: f64,
}

/// A forecast at one of a location's extra elevations (ski areas: base, mid,
/// summit).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ElevationForecast {
    /// The elevation's name (pass it to
    /// [`ForecastRequest::elevations`](crate::ForecastRequest::elevations)).
    pub name: String,
    /// What the elevation is called, for display.
    pub display_name: Option<String>,
    /// (JSON `elevation`) The elevation, in the request's length unit.
    #[serde(rename(deserialize = "elevation"), alias = "height")]
    pub height: f64,
    /// The latitude.
    pub lat: f64,
    /// The longitude.
    pub lon: f64,
    /// The hourly series.
    #[serde(default)]
    pub hourly: Vec<TimeSeriesPoint>,
    /// Daily summaries.
    #[serde(default)]
    pub daily: Vec<DailyForecast>,
    /// Snow totals.
    pub snow_outlook: Option<SnowOutlook>,
    /// Liquid-equivalent totals.
    pub precipitation_outlook: Option<PrecipOutlook>,
    /// The members in columns (with [`Members::Plume`](crate::Members::Plume)).
    pub plume: Option<MemberPlume>,
    /// Observation sensors near this elevation's point.
    #[serde(default)]
    pub nearest_sensors: Vec<NearestSensor>,
    /// What the snowpack model (beta) states is on the ground here now.
    pub snow_on_ground: Option<SnowOnGround>,
}

with_meta!(Forecast);
