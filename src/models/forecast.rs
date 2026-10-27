//! Forecasts: the hourly series, daily summaries, probabilities and totals.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::enums::{
    ClampedOption, Condition, ElevationSource, FreezingLevelKind, GapFlowStatus, InversionKind,
    MembersView, MoonPhase, NowcastSource, PrecipKind, Season, StormOrientation, WindStatus,
};
use crate::meta::{ResponseMeta, with_meta};
use crate::models::{CurrentConditions, LocationRef, ResponseAttribution};
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
/// For wind direction the quantiles describe an arc on the circle, not an
/// interval: `max` may be numerically less than `min`. The SDK does not
/// "fix" this.
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
/// for hour in &forecast.forecast.timeseries {
///     println!("{} {:?}", hour.valid_time, hour.ensemble.temperature.range80());
/// }
/// # Ok(()) }
/// ```
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
    /// The location.
    pub location: LocationRef,
    /// The model run behind the forecast.
    pub model_run: ModelRunInfo,
    /// The unit system of every value in this response.
    pub units: Units,
    /// The hourly series and daily summaries.
    pub forecast: ForecastData,
    /// Forecasts at the location's extra elevations, when asked for.
    #[serde(default)]
    pub elevation_forecasts: Vec<ElevationForecast>,
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
    /// The data sources; carry `credit` with redistributed output.
    pub attribution: ResponseAttribution,
}

/// One probability term, answered, with its reference class: the variable,
/// comparison, threshold, window and member count.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ProbabilityResult {
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
    /// The share of members meeting the term, 0..1. `None` when the served
    /// series does not reach the window's end: absent, never 0.
    pub probability: Option<f64>,
    /// The number of members that voted.
    pub members: u32,
}

/// The distribution of each member's own total over a window.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct WindowTotal {
    /// Precipitation or snowfall.
    pub variable: Variable,
    /// The quantiles' unit (set by the SDK from the response's units).
    #[serde(skip)]
    pub unit: Option<Unit>,
    /// The window's first hour.
    pub window_start_hours: u32,
    /// The window's end hour.
    pub window_end_hours: u32,
    /// The window's first instant (RFC 3339); `None` when not covered.
    pub from: Option<String>,
    /// The window's exclusive end (RFC 3339); `None` when not covered.
    pub to: Option<String>,
    /// The distribution; `None` when the forecast does not cover the window.
    pub quantiles: Option<Quantiles>,
    /// The number of members.
    pub members: u32,
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
    /// Behind each day's `precip_probability`.
    pub daily_precip_probability: f64,
    /// Behind each day's `precip_occurrence_probability`.
    pub daily_precip_occurrence_probability: f64,
    /// Behind each day's `snow_probability`.
    pub daily_snow_probability: f64,
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
    /// `include_past`).
    #[serde(default)]
    pub timeseries: Vec<TimeSeriesPoint>,
    /// The daily summaries, one per local date.
    #[serde(default)]
    pub daily: Vec<DailyForecast>,
    /// Snow totals for common display windows.
    pub snow_outlook: Option<SnowOutlook>,
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
    /// The condition code.
    pub condition: Condition,
    /// The condition, for display.
    pub condition_text: String,
    /// The icon name.
    pub condition_icon: String,
    /// Whether gusts are notable.
    pub gust_notable: Option<bool>,
    /// The precipitation type with its probabilities.
    pub precip_type: Option<PrecipType>,
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
    /// The server's running total of hourly precipitation means.
    pub cumulative_precipitation: Option<f64>,
    /// The server's running total of hourly snowfall means.
    pub cumulative_snowfall: Option<f64>,
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

/// Wind direction as a circular distribution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CircularStats {
    /// The circular mean, degrees.
    pub mean: f64,
    /// The circular standard deviation, degrees.
    pub stddev: Option<f64>,
    /// The narrowest arc holding half the members.
    pub sector_50: Sector,
    /// The narrowest arc holding 90 % of the members.
    pub sector_90: Sector,
}

/// One model's member values at an hour.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ModelMembers {
    /// The model's ID.
    pub id: String,
    /// One entry per member.
    #[serde(default)]
    pub members: Vec<MemberForecast>,
}

/// One ensemble member's values at an hour. The plume view (Free) withholds
/// wind and SLR.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct MemberForecast {
    /// The member's index within its model.
    pub member_id: u32,
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
    /// This member's snow-to-liquid ratio.
    pub slr: Option<f64>,
    /// This member's snowfall.
    pub snowfall: Option<f64>,
    /// This member's cumulative precipitation.
    pub cumulative_precipitation: Option<f64>,
    /// This member's cumulative snowfall.
    pub cumulative_snowfall: Option<f64>,
}

/// The distributions of the members' running totals.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct MembersCumulative {
    /// Precipitation.
    pub precipitation: Option<Quantiles>,
    /// Snowfall.
    pub snowfall: Option<Quantiles>,
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
    /// The feels like UTCI distribution.
    pub feels_like_utci: Option<Quantiles>,
    /// The dewpoint distribution.
    pub dewpoint: Option<Quantiles>,
    /// The precipitation distribution.
    pub precipitation: Quantiles,
    /// The wind speed distribution.
    pub wind_speed: Quantiles,
    /// The wind gust distribution.
    pub wind_gust: Option<Quantiles>,
    /// Circular: the quantiles describe an arc, and `max` may be less than
    /// `min`. Prefer `wind_direction_circular`.
    pub wind_direction: Quantiles,
    /// Wind direction as a circular distribution.
    pub wind_direction_circular: Option<CircularStats>,
    /// The snow level distribution.
    pub snow_level: Option<Quantiles>,
    /// The snow-to-liquid ratio distribution.
    pub slr: Option<Quantiles>,
    /// The snowfall distribution.
    pub snowfall: Option<Quantiles>,
    /// The cloud cover distribution.
    pub cloud_cover: Option<Quantiles>,
    /// The skin temperature distribution.
    pub skin_temperature: Option<Quantiles>,
    /// The precipitation-type distribution.
    pub ptype: Option<Quantiles>,
    /// The mean sea-level pressure distribution.
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
    /// The wind direction distribution.
    pub wind_direction: Option<Quantiles>,
    /// The snow level distribution.
    pub snow_level: Option<Quantiles>,
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
    /// The icon name.
    pub condition_icon: String,
    /// The precipitation type; `None` when the day is unlikely to precipitate.
    pub precip_type: Option<PrecipKind>,
    /// The day's high.
    pub high: Quantiles,
    /// The day's low.
    pub low: Quantiles,
    /// The feels-like high.
    pub feels_like_high: Option<Quantiles>,
    /// The feels-like low.
    pub feels_like_low: Option<Quantiles>,
    /// The UTCI feels-like high.
    pub feels_like_utci_high: Option<Quantiles>,
    /// The UTCI feels-like low.
    pub feels_like_utci_low: Option<Quantiles>,
    /// The distribution of each member's own day total.
    pub total_precip: Quantiles,
    /// The distribution of each member's own day snowfall.
    pub total_snowfall: Option<Quantiles>,
    /// P(day total ≥ the daily precipitation threshold).
    pub precip_probability: f64,
    /// P(day total ≥ the occurrence threshold).
    pub precip_occurrence_probability: f64,
    /// P(day snowfall ≥ the daily snow threshold).
    pub snow_probability: f64,
    /// The mean snow level.
    pub snow_level_mean: Option<f64>,
    /// The snow level's distribution.
    pub snow_level: Option<Quantiles>,
    /// The day total among the wet members.
    pub total_precip_if_wet: Option<Quantiles>,
    /// The day snowfall among the wet members.
    pub total_snowfall_if_wet: Option<Quantiles>,
    /// The precipitation headline, for display.
    pub precip_display: PrecipDisplay,
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
    pub astronomical_season: Season,
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
    /// The elevation's name.
    pub name: String,
    /// The elevation, in the request's length unit.
    pub elevation: f64,
    /// Whether this is the primary elevation.
    #[serde(default)]
    pub default: bool,
    /// The latitude.
    pub lat: f64,
    /// The longitude.
    pub lon: f64,
    /// A compact hourly series.
    #[serde(default)]
    pub timeseries: Vec<ElevationHour>,
    /// The full hourly series.
    #[serde(default)]
    pub hourly: Vec<TimeSeriesPoint>,
    /// Daily summaries.
    #[serde(default)]
    pub daily: Vec<DailyForecast>,
    /// Snow totals.
    pub snow_outlook: Option<SnowOutlook>,
}

/// One hour of an elevation forecast's compact series.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ElevationHour {
    /// The hour (RFC 3339).
    pub valid_time: String,
    /// Hours since the run's init.
    pub lead_time_hours: i32,
    /// The temperature distribution.
    pub temperature: Quantiles,
    /// The snow level relative to this elevation.
    pub snow_level_delta: Option<Quantiles>,
    /// The wind speed distribution.
    pub wind_speed: Option<Quantiles>,
    /// The wind gust distribution.
    pub wind_gust: Option<Quantiles>,
    /// The precipitation distribution.
    pub precipitation: Quantiles,
    /// The condition code.
    pub condition: Condition,
    /// The condition, for display.
    pub condition_text: String,
    /// The icon name.
    pub condition_icon: String,
}

with_meta!(Forecast);
