//! A location's season to date, snow trend and snowpack (beta).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::meta::{ResponseMeta, with_meta};
use crate::models::{LocationRef, ResponseAttribution};
use crate::units::Units;

/// Observed season-to-date precipitation at a location, from
/// [`Client::season`](crate::Client::season).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Season {
    /// How the response was obtained.
    #[serde(skip)]
    pub meta: ResponseMeta,
    /// The location.
    pub location: LocationRef,
    /// The water year (October to September), named by the year it ends in.
    pub water_year: u32,
    /// The first local day of the water year (`YYYY-MM-DD`).
    pub starts: String,
    /// The last local day with an observed row (the day before `starts` when there is none).
    pub through: String,
    /// When the newest day row was computed; null with no rows.
    pub as_of: Option<String>,
    /// The units.
    pub units: Units,
    /// Null until the location has an observed day.
    pub precipitation: Option<SeasonTotal>,
    /// Always null for now; season snowfall is not served yet.
    pub snowfall: Option<SeasonTotal>,
    /// The nearest SNOTEL or SCAN gauge within 25 km and 300 m with rows this water year, or
    /// null.
    pub station: Option<SeasonStation>,
    /// Only with `include=daily`.
    #[serde(default)]
    pub daily: Vec<SeasonDay>,
    /// The attribution.
    pub attribution: ResponseAttribution,
}

/// A season-to-date total.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SeasonTotal {
    /// (JSON `total`) Sum over the covered days, in `unit`.
    #[serde(rename(deserialize = "total"), alias = "amount")]
    pub amount: f64,
    /// The unit.
    pub unit: String,
    /// `mrms_multisensor`: the gridded analysis at the location; `station_gauge`: the location
    /// is a gauge site and this is the gauge's accumulator.
    pub source: String,
    /// The days with a complete observation.
    pub days_covered: u32,
    /// Days since `starts` with no complete observation; not counted as dry.
    pub days_missing: u32,
    /// Covered days still inside the 7-day restate window.
    pub provisional_days: u32,
    /// PRISM 1991-2020 over the covered days, in `unit`; null where no normal is loaded.
    pub normal_to_date: Option<f64>,
    /// `amount` as a percent of `normal_to_date`.
    pub percent_of_normal: Option<u32>,
    /// Where the normal comes from.
    pub normal_source: Option<String>,
}

/// The nearest snow gauge with readings this water year.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SeasonStation {
    /// The NRCS station triplet.
    pub id: String,
    /// The network.
    pub network: String,
    /// The name.
    pub name: String,
    /// The distance from the location, km.
    pub distance_km: f64,
    /// The gauge's elevation, m.
    pub elevation_m: f64,
    /// The gauge's water-year accumulator on `through` (mm or in).
    pub precipitation_total: Option<f64>,
    /// `precipitation_total` as a percent of the median for the date.
    pub precipitation_percent_of_median: Option<u32>,
    /// Snow water equivalent on `through` (mm or in).
    pub swe: Option<f64>,
    /// `swe` as a percent of the median for the date.
    pub swe_percent_of_median: Option<u32>,
    /// Snow depth on `through` (cm or in).
    pub snow_depth: Option<f64>,
    /// The gauge's newest day.
    pub through: String,
}

/// One local day of the season record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SeasonDay {
    /// The date.
    pub date: String,
    /// The day's total; null when an hour was not observed.
    pub precipitation: Option<f64>,
    /// Always null for now.
    pub snowfall: Option<f64>,
    /// Whether the day is still inside the 7-day restate window.
    pub provisional: bool,
}

/// Each recent run's daily snowfall at a location's point, by source, from
/// [`Client::snow_trend`](crate::Client::snow_trend). Read down a column to
/// see a storm firm up or fade as the runs come in.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SnowTrend {
    /// How the response was obtained.
    #[serde(skip)]
    pub meta: ResponseMeta,
    /// The location.
    pub location: LocationRef,
    /// The units.
    pub units: Units,
    /// The point the grids are for.
    pub elevation: SnowTrendElevation,
    /// How many days back runs are kept.
    pub retention_days: u32,
    /// The columns, location-local dates ascending.
    #[serde(default)]
    pub dates: Vec<String>,
    /// `ourea` first; with the `model_data` entitlement then `aifs`, `aigefs`, `gefs`,
    /// `ifs_ens` and `rrfs`.
    #[serde(default)]
    pub grids: Vec<SnowTrendGrid>,
}

/// The point the grids are for.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SnowTrendElevation {
    /// The named elevation; `null` for a location with one point.
    pub name: Option<String>,
    /// What that elevation is called, as `ElevationSummary.display_name`; absent where it has
    /// no name.
    pub display_name: Option<String>,
    /// (JSON `elevation`) Its elevation, in m (metric) or ft (imperial).
    #[serde(rename(deserialize = "elevation"), alias = "height")]
    pub height: f64,
}

/// One source's grid: a row per run, a column per date.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SnowTrendGrid {
    /// `ourea`, or a model id.
    pub source: String,
    /// Oldest first; the last is the newest.
    #[serde(default)]
    pub runs: Vec<SnowTrendRun>,
}

/// One run's row of a snow-trend grid.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SnowTrendRun {
    /// `true`: re-rendered after the fact from the pinned run (`?run=<initialized_at>`) by the
    /// post-deploy backfill -- today's calibration and no live nowcast, so it can differ
    /// slightly from what was served then.
    pub backfilled: bool,
    /// `ourea`: the served body's `model_run.initialized_at`.
    pub initialized_at: String,
    /// One entry per `dates` entry, in order; `null` where the run has no value.
    #[serde(default)]
    pub days: Vec<Option<SnowTrendDay>>,
}

/// One run's snowfall on one date.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SnowTrendDay {
    /// The `dates` entry this day is for.
    pub date: String,
    /// (JSON `snow`) The day's snowfall in mm (metric) or inches (imperial), at the served amount precision.
    #[serde(rename(deserialize = "snow"), alias = "amount")]
    pub amount: Option<f64>,
    /// The run covers only part of this local day.
    pub partial: bool,
    /// The range's low end.
    pub p10: Option<f64>,
    /// The range's high end, as `p10`.
    pub p90: Option<f64>,
    /// `ourea` only: the day's `precipitation_display.kind`.
    pub kind: Option<String>,
}

/// The snowpack forecast of a location's spots.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Snowpack {
    /// How the response was obtained.
    #[serde(skip)]
    pub meta: ResponseMeta,
    /// Always `true` while the product is a beta; fields may be added.
    pub beta: bool,
    /// The location.
    pub location: LocationRef,
    /// The units.
    pub units: Units,
    /// The model run.
    pub model_run: Option<SnowpackRun>,
    /// The newest run's blend cycle is more than 24 h old.
    pub stale: bool,
    /// The location's own point first, then its named elevations in their display order.
    #[serde(default)]
    pub spots: Vec<SnowpackSpot>,
    /// What each surface class and layer form means.
    pub definitions: SnowpackDefinitions,
}

/// What each surface class and layer form means. The keys are data (such
/// as `buried_surface_hoar`), kept as the API sends them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SnowpackDefinitions {
    /// Each surface class and what it means.
    pub surface: BTreeMap<String, String>,
    /// Each layer form and what it is.
    pub layer_form: BTreeMap<String, String>,
}

/// The newest snowpack run among the spots served.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SnowpackRun {
    /// The blend cycle the snowpack model was driven with.
    pub initialized_at: String,
    /// When the run finished (RFC 3339).
    pub generated_at: String,
    /// The model.
    pub model: SnowpackModel,
}

/// The snowpack model that ran.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SnowpackModel {
    /// The name.
    pub name: String,
    /// Who makes the model.
    pub by: String,
    /// The model's licence.
    pub license: String,
    /// The build that ran: the commits of Ourea's SNOWPACK and MeteoIO source it was built
    /// from, as `snowpack <commit> meteoio <commit>`.
    pub version: Option<String>,
}

/// The snowpack forecast at one spot of a location.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SnowpackSpot {
    /// The named elevation (`location.elevations[].name`); `null` is the location's own point.
    pub name: Option<String>,
    /// What the named elevation is called, as `ElevationSummary.display_name`; absent for the
    /// location's own point and where the catalog has no name for the elevation.
    pub display_name: Option<String>,
    /// In the response's units (m or ft).
    pub elevation: f64,
    /// The spot's elevation, m.
    pub elevation_m: f64,
    /// The blend cycle this spot's forecast was driven with; older than
    /// `model_run.initialized_at` when the newest run did not reach the spot.
    pub initialized_at: String,
    /// The snow-depth sensor at the spot.
    pub anchor: Option<SnowpackAnchor>,
    /// Hourly snow depth.
    pub depth: SnowpackDepth,
    /// The depth at `depth.start_time` less this spot's modelled depth 24 h before, in the
    /// depth's units and rounding (cm or in; a loss is negative): the model's own change,
    /// snowfall, settlement and melt together.
    pub depth_change_24h: Option<f64>,
    /// Hourly snow water equivalent (mm or in), parallel to `depth.p50`.
    #[serde(default)]
    pub swe: Vec<f64>,
    /// One entry every six hours.
    #[serde(default)]
    pub steps: Vec<SnowpackStep>,
    /// Buried layers at `depth.start_time`, at most three.
    #[serde(default)]
    pub layers: Vec<SnowpackLayer>,
    /// The snow on a 38° slope at the spot facing each way, in the order north, east, south,
    /// west.
    #[serde(default)]
    pub aspects: Vec<SnowpackAspect>,
}

/// The snow-depth sensor at the spot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SnowpackAnchor {
    /// The station's catalog slug.
    pub station: String,
    /// The name.
    pub name: String,
    /// The network.
    pub network: String,
}

/// Hourly snow depth from `start_time` (cm or in).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SnowpackDepth {
    /// The time of the first value (RFC 3339).
    pub start_time: String,
    /// Hours between values.
    pub interval_hours: u32,
    /// `measured`: started from a sensor reading at the spot.
    pub basis: String,
    /// The 10th percentile.
    #[serde(default)]
    pub p10: Vec<f64>,
    /// The median.
    #[serde(default)]
    pub p50: Vec<f64>,
    /// The 90th percentile.
    #[serde(default)]
    pub p90: Vec<f64>,
}

/// The surface every six hours.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SnowpackStep {
    /// The step's time (RFC 3339).
    pub valid_time: String,
    /// A key of `definitions.surface`: today `none`, `powder`, `wind_affected`, `settled`,
    /// `wet`, `crust` or `ice_crust`.
    pub surface: String,
    /// New snow of the 24 h to `valid_time`, as it lies (cm or in).
    pub new_snow_24h: f64,
    /// Its density.
    pub new_snow_density_kg_m3: Option<f64>,
    /// How far the snow on the ground 24 h earlier settled (cm or in).
    pub settlement_24h: Option<f64>,
}

/// A buried layer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SnowpackLayer {
    /// A key of `definitions.layer_form`: today `faceted_grains`, `depth_hoar`,
    /// `buried_surface_hoar`, `melt_freeze_crust`, `ice_layer` or `rounding_facets`.
    pub form: String,
    /// cm or in.
    pub thickness: f64,
    /// From the surface down to the layer's top (cm or in).
    pub depth: f64,
    /// When the layer formed (RFC 3339).
    pub formed: String,
}

/// The snow on one slope at a spot, described as the spot's flat ground is: the model run on a
/// slope of `slope_angle_deg` facing `aspect`, with the flat ground's snowfall and the slope's
/// own sun.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SnowpackAspect {
    /// The way the slope faces: today `north`, `east`, `south` or `west`.
    pub aspect: String,
    /// How steep the slope is, degrees from horizontal.
    pub slope_angle_deg: f64,
    /// Hourly snow depth on the slope, measured vertically as the flat depth is.
    pub depth: SnowpackDepth,
    /// As the spot's `depth_change_24h`, this slope's own (open loop on both sides), read from
    /// the same earlier run as the spot's and from no other.
    pub depth_change_24h: Option<f64>,
    /// One entry every six hours, at the spot's own `steps[].valid_time`.
    #[serde(default)]
    pub steps: Vec<SnowpackStep>,
    /// Buried layers on the slope at `depth.start_time`, at most three.
    #[serde(default)]
    pub layers: Vec<SnowpackLayer>,
}

/// A location's snow-depth plume.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SnowpackPlume {
    /// How the response was obtained.
    #[serde(skip)]
    pub meta: ResponseMeta,
    /// Always `true` while the product is a beta; fields may be added.
    pub beta: bool,
    /// The location.
    pub location: LocationRef,
    /// The units.
    pub units: Units,
    /// The run the plume is from: the 12Z blend cycle the members were read from.
    pub model_run: Option<SnowpackRun>,
    /// The plume's blend cycle is more than 24 h old.
    pub stale: bool,
    /// The members' depth at each step.
    pub depth: Option<SnowpackPlumeDepth>,
    /// Every member's own series, by model, each model to its own horizon.
    #[serde(default)]
    pub models: Vec<SnowpackPlumeModel>,
    /// Every member's depth as ONE unlabelled pool, for Free and anonymous callers (absent for
    /// Plus and Pro, who are served `models`).
    #[serde(default)]
    pub member_depths: Vec<Vec<f64>>,
    /// What the bands are, in a sentence a client may show beside them.
    pub spread: String,
}

/// The members' depth at each step.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SnowpackPlumeDepth {
    /// The time of the first value of every series in the body.
    pub start_time: String,
    /// Hours between values (3).
    pub interval_hours: u32,
    /// Members that reach each step; it falls where a model's horizon ends.
    #[serde(default)]
    pub members: Vec<u32>,
    /// The members' median depth, in the response's units.
    #[serde(default)]
    pub p50: Vec<f64>,
    /// Served to every caller, as the bands are.
    #[serde(default)]
    pub mean: Vec<f64>,
    /// Every caller (Plus and Pro only before).
    #[serde(default)]
    pub p10: Vec<f64>,
    /// Every caller (Plus and Pro only before).
    #[serde(default)]
    pub p25: Vec<f64>,
    /// Every caller (Plus and Pro only before).
    #[serde(default)]
    pub p75: Vec<f64>,
    /// Every caller (Plus and Pro only before).
    #[serde(default)]
    pub p90: Vec<f64>,
}

/// One model's members in a snow-depth plume.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SnowpackPlumeModel {
    /// `aifs`, `aigefs`, `gefs` or `ifs_ens`; a client must tolerate one it does not know.
    pub id: String,
    /// The model's own member numbers, in the order of `depth`.
    #[serde(default)]
    pub member_ids: Vec<u32>,
    /// One series per member, from `depth.start_time` every `depth.interval_hours`.
    #[serde(default)]
    pub depth: Vec<Vec<f64>>,
    /// The equal-weight mean of this model's members' `depth` at each of the model's own steps
    /// (from `depth.start_time` every `depth.interval_hours`, to the model's horizon: the
    /// length of its members' series), in the response's units.
    #[serde(default)]
    pub depth_mean: Vec<f64>,
    /// Members not run because the ground starts bare and they bring no snow; their series is 0
    /// throughout.
    pub members_skipped: u32,
}

with_meta!(Season, SnowTrend, Snowpack, SnowpackPlume);
