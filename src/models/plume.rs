//! The plume view of a forecast (`members=plume`): the members in columns.
//!
//! Every column is aligned to its `valid_time`, and a `None` is a value the
//! member (or the band) does not have at that step.

use serde::{Deserialize, Serialize};

/// One served series' members in columns (`members=plume`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct MemberPlume {
    /// The headline.
    pub headline: PlumeHeadline,
    /// The bands.
    pub bands: PlumeBands,
    /// One entry per model with members in the served window, in the order
    /// `aifs`, `aigefs`, `gefs`, `ifs_ens`, `rrfs` (Plus and Pro).
    #[serde(default)]
    pub models: Vec<ModelColumns>,
    /// (JSON `members`) The members as one unlabelled pool (Free and
    /// anonymous callers; `members_view` is `pool`).
    #[serde(rename(deserialize = "members"), alias = "pool")]
    pub pool: Option<PooledColumns>,
}

/// The lines a plume draws over its members, at every served hour
/// (`valid_time`): the blend's means and running totals (the "Ourea forecast"
/// line).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PlumeHeadline {
    /// The valid time.
    #[serde(default)]
    pub valid_time: Vec<String>,
    /// The temperature.
    #[serde(default)]
    pub temperature: Vec<Option<f64>>,
    /// The snow level.
    #[serde(default)]
    pub snow_level: Vec<Option<f64>>,
    /// The blend's snow level on the NWS scale (`ensemble.snow_level_wet_bulb.mean`, the height
    /// of the 0.5 C wet-bulb); `snow_level` is the rain line.
    #[serde(default)]
    pub snow_level_wet_bulb: Vec<Option<f64>>,
    /// The cumulative precipitation.
    #[serde(default)]
    pub cumulative_precipitation: Vec<Option<f64>>,
    /// The hourly series' `cumulative_snowfall`: the running sum of the day cards' snowfall
    /// (`daily[].precipitation_display.snow_amount`), spread inside each local date by the
    /// blend's hourly mean snowfall, so at the end of each date served whole it equals the
    /// running sum of the cards.
    #[serde(default)]
    pub cumulative_snowfall: Vec<Option<f64>>,
    /// The headline's running snowfall at a fixed 10:1 snow-to-liquid ratio,
    /// in the snowfall unit.
    #[serde(default)]
    pub cumulative_snowfall_10to1: Vec<Option<f64>>,
    /// The forecast's hourly wind speed, the hourly series' `ensemble.wind_speed.mean`, in the
    /// wind unit (mph / km/h / m/s as `units` says) at the wind precision (0.1), at every
    /// served hour.
    #[serde(default)]
    pub wind_speed: Vec<Option<f64>>,
    /// The forecast's snowfall in the 6 hours ending at each 00/06/12/18Z hour, in the snowfall
    /// units (in / mm) at the amount precision: the rise of `cumulative_snowfall` over
    /// `(valid_time - 6 h, valid_time]`, computed here so no client differences the line.
    #[serde(default)]
    pub snowfall_6h: Vec<Option<f64>>,
}

/// The members' distributions, at the served hours that carry at least one
/// (`valid_time`; the synoptic hours in practice). `cumulative_precipitation`
/// and `cumulative_snowfall` are the spread of the members' running totals,
/// never a sum of hourly percentiles.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PlumeBands {
    /// The valid time.
    #[serde(default)]
    pub valid_time: Vec<String>,
    /// The cumulative precipitation.
    pub cumulative_precipitation: BandColumns,
    /// The cumulative snowfall.
    pub cumulative_snowfall: BandColumns,
    /// The members' running snowfall at 10:1 (each member's snowfall with its
    /// SLR replaced by 10), totalled as `cumulative_snowfall` is.
    pub cumulative_snowfall_10to1: BandColumns,
    /// The temperature.
    pub temperature: BandColumns,
    /// The snow level.
    pub snow_level: BandColumns,
    /// The row form's `members_spread.snow_level_wet_bulb` -- the members' snow level on the
    /// NWS scale, at the hours `snow_level` carries a band.
    pub snow_level_wet_bulb: BandColumns,
    /// The precipitation 6h.
    pub precipitation_6h: BandColumns,
    /// The snowfall 6h.
    pub snowfall_6h: BandColumns,
    /// The snow-to-liquid ratio.
    pub slr: Option<BandColumns>,
    /// `members_spread.wind_speed`.
    pub wind_speed: Option<BandColumns>,
}

/// One model's members over its native steps inside the served window (`valid_time`: the hours
/// at which a member of the model carries a value -- every hour for RRFS, every sixth for the
/// global ensembles; never padded to the hourly series).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ModelColumns {
    /// The id.
    pub id: String,
    /// The valid time.
    #[serde(default)]
    pub valid_time: Vec<String>,
    /// The members, ascending (`MemberForecast.member_id`).
    #[serde(default)]
    pub member_ids: Vec<u32>,
    /// The temperature.
    #[serde(default)]
    pub temperature: Vec<Vec<Option<f64>>>,
    /// The precipitation.
    #[serde(default)]
    pub precipitation: Vec<Vec<Option<f64>>>,
    /// `[member][step]`: the hours each `precipitation` value covers; `null` where it is absent
    /// or its span unknown.
    #[serde(default)]
    pub precipitation_hours: Vec<Vec<Option<u32>>>,
    /// The cumulative precipitation.
    #[serde(default)]
    pub cumulative_precipitation: Vec<Vec<Option<f64>>>,
    /// The snow level.
    #[serde(default)]
    pub snow_level: Vec<Vec<Option<f64>>>,
    /// `[Member][step]`, each member's `MemberForecast.snow_level_wet_bulb` (the snow level on
    /// the NWS scale); `null` exactly where its `snow_level` is.
    #[serde(default)]
    pub snow_level_wet_bulb: Vec<Vec<Option<f64>>>,
    /// The snowfall.
    #[serde(default)]
    pub snowfall: Vec<Vec<Option<f64>>>,
    /// The cumulative snowfall.
    #[serde(default)]
    pub cumulative_snowfall: Vec<Vec<Option<f64>>>,
    /// One value per `valid_time` step (not per member): the median of the model's members'
    /// `cumulative_snowfall` there (Hyndman-Fan type 7, as `bands`: the middle value, or the
    /// midpoint of the two middle ones; a single-member model's own total), at the amount
    /// precision.
    #[serde(default)]
    pub cumulative_snowfall_p50: Vec<Option<f64>>,
    /// As `cumulative_snowfall_p50`, over the members' running snowfall at 10:1 (see
    /// `PlumeBands.cumulative_snowfall_10to1`).
    #[serde(default)]
    pub cumulative_snowfall_10to1_p50: Vec<Option<f64>>,
    /// The model's mean lines.
    #[serde(default)]
    pub temperature_mean: Vec<Option<f64>>,
    /// The mean of `precipitation` (see `temperature_mean`).
    #[serde(default)]
    pub precipitation_mean: Vec<Option<f64>>,
    /// The mean of `cumulative_precipitation` (see `temperature_mean`).
    #[serde(default)]
    pub cumulative_precipitation_mean: Vec<Option<f64>>,
    /// The mean of `snow_level` (see `temperature_mean`).
    #[serde(default)]
    pub snow_level_mean: Vec<Option<f64>>,
    /// The mean of `snow_level_wet_bulb` (see `temperature_mean`).
    #[serde(default)]
    pub snow_level_wet_bulb_mean: Vec<Option<f64>>,
    /// The mean of `snowfall` (see `temperature_mean`).
    #[serde(default)]
    pub snowfall_mean: Vec<Option<f64>>,
    /// The mean of `cumulative_snowfall` (see `temperature_mean`), beside its median
    /// `cumulative_snowfall_p50`.
    #[serde(default)]
    pub cumulative_snowfall_mean: Vec<Option<f64>>,
    /// The mean of the members' running snowfall at 10:1, beside
    /// `cumulative_snowfall_10to1_p50` (the members' 10:1 column itself is not served).
    #[serde(default)]
    pub cumulative_snowfall_10to1_mean: Vec<Option<f64>>,
    /// The mean of `wind_speed` (see `temperature_mean`).
    #[serde(default)]
    pub wind_speed_mean: Vec<Option<f64>>,
    /// Which of this model's `member_ids` are in `bands.cumulative_precipitation`, ascending.
    #[serde(default)]
    pub cumulative_precipitation_band_member_ids: Vec<u32>,
    /// As `cumulative_precipitation_band_member_ids`, for `bands.cumulative_snowfall`.
    #[serde(default)]
    pub cumulative_snowfall_band_member_ids: Vec<u32>,
    /// As `cumulative_precipitation_band_member_ids`, for `bands.cumulative_snowfall_10to1`.
    #[serde(default)]
    pub cumulative_snowfall_10to1_band_member_ids: Vec<u32>,
    /// Pro (`members_view: full`) only; absent in the plume view.
    #[serde(default)]
    pub wind_speed: Vec<Vec<Option<f64>>>,
    /// Pro only; absent in the plume view.
    #[serde(default)]
    pub wind_direction: Vec<Vec<Option<f64>>>,
    /// In both views.
    #[serde(default)]
    pub slr: Vec<Vec<Option<f64>>>,
}

/// The members as one unlabelled pool, for a caller without the plume's paid detail (Free and
/// anonymous; `members_view: pool`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PooledColumns {
    /// The valid time.
    #[serde(default)]
    pub valid_time: Vec<String>,
    /// The temperature.
    #[serde(default)]
    pub temperature: Vec<Vec<Option<f64>>>,
    /// The precipitation.
    #[serde(default)]
    pub precipitation: Vec<Vec<Option<f64>>>,
    /// `[member][step]`: the hours each `precipitation` and `snowfall` value covers: the grid
    /// step (6), less where a member's first value covers less; `null` where the value is
    /// absent.
    #[serde(default)]
    pub precipitation_hours: Vec<Vec<Option<u32>>>,
    /// The cumulative precipitation.
    #[serde(default)]
    pub cumulative_precipitation: Vec<Vec<Option<f64>>>,
    /// The snow level.
    #[serde(default)]
    pub snow_level: Vec<Vec<Option<f64>>>,
    /// The snow level on the NWS (wet-bulb) scale.
    #[serde(default)]
    pub snow_level_wet_bulb: Vec<Vec<Option<f64>>>,
    /// The snowfall.
    #[serde(default)]
    pub snowfall: Vec<Vec<Option<f64>>>,
    /// The cumulative snowfall.
    #[serde(default)]
    pub cumulative_snowfall: Vec<Vec<Option<f64>>>,
    /// The snow-to-liquid ratio.
    #[serde(default)]
    pub slr: Vec<Vec<Option<f64>>>,
    /// The rows (indices into every column) in `bands.cumulative_precipitation`'s fixed
    /// population, ascending: that band's `mean` is the mean of these rows' running totals.
    #[serde(default)]
    pub cumulative_precipitation_band_members: Vec<u32>,
    /// Likewise for `bands.cumulative_snowfall`.
    #[serde(default)]
    pub cumulative_snowfall_band_members: Vec<u32>,
    /// Likewise for `bands.cumulative_snowfall_10to1`.
    #[serde(default)]
    pub cumulative_snowfall_10to1_band_members: Vec<u32>,
}

/// One band's statistics (Hyndman-Fan type 7 over the served member values), each an array
/// aligned to `PlumeBands.valid_time`, `null` where the band is absent at that hour.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct BandColumns {
    /// The mean.
    #[serde(default)]
    pub mean: Vec<Option<f64>>,
    /// The min.
    #[serde(default)]
    pub min: Vec<Option<f64>>,
    /// The p10.
    #[serde(default)]
    pub p10: Vec<Option<f64>>,
    /// The p25.
    #[serde(default)]
    pub p25: Vec<Option<f64>>,
    /// The p50.
    #[serde(default)]
    pub p50: Vec<Option<f64>>,
    /// The p75.
    #[serde(default)]
    pub p75: Vec<Option<f64>>,
    /// The p90.
    #[serde(default)]
    pub p90: Vec<Option<f64>>,
    /// The max.
    #[serde(default)]
    pub max: Vec<Option<f64>>,
}
