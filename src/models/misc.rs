//! Model runs, storm reports and attribution.

use serde::{Deserialize, Serialize};

use crate::enums::SourceKind;
use crate::meta::{ResponseMeta, with_meta};
use crate::models::ModelEntry;

/// The available model runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Runs {
    /// How the response was obtained.
    #[serde(skip)]
    pub meta: ResponseMeta,
    /// What a forecast request serves now; `None` when no cycle is servable.
    pub served: Option<ServedRun>,
    /// The runs, newest first.
    #[serde(default)]
    pub runs: Vec<AvailableRun>,
    /// The number of runs.
    pub count: u32,
}

/// The run currently served, with its models.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ServedRun {
    /// The reference init time (RFC 3339).
    pub initialized_at: String,
    /// Each contributing model and its own cycle.
    #[serde(default)]
    pub models: Vec<ModelEntry>,
}

/// One available model run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct AvailableRun {
    /// The run's init time (RFC 3339).
    pub initialized_at: String,
}

/// Recent storm verification reports.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct StormReports {
    /// How the response was obtained.
    #[serde(skip)]
    pub meta: ResponseMeta,
    /// The reports, newest first.
    #[serde(default)]
    pub reports: Vec<StormReportCard>,
    /// The number of reports.
    pub count: u32,
}

/// A storm's forecast-versus-observed verification report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct StormReportCard {
    /// The report's ID.
    pub id: i64,
    /// The location.
    pub location_name: String,
    /// The kind of storm.
    pub event_type: String,
    /// The onset (RFC 3339).
    pub onset_time: String,
    /// The end (RFC 3339).
    pub end_time: String,
    /// The forecast precipitation total, mm.
    pub forecast_total_mm: f64,
    /// The observed precipitation total, mm.
    pub observed_total_mm: f64,
    /// The forecast snowfall, mm.
    pub forecast_snowfall_mm: Option<f64>,
    /// The observed snowfall, mm.
    pub observed_snowfall_mm: Option<f64>,
    /// The best-scoring model.
    pub best_model: Option<String>,
    /// The best-scoring lead time, hours.
    pub best_lead_time_hours: Option<i32>,
    /// The composite score.
    pub composite_score: Option<f64>,
    /// The headline.
    pub headline: Option<String>,
}

/// One storm report, as [`Client::storm_report`](crate::Client::storm_report)
/// returns it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct StormReport {
    /// How the response was obtained.
    #[serde(skip)]
    pub meta: ResponseMeta,
    /// The report.
    #[serde(flatten)]
    pub report: StormReportCard,
}

/// Every data source, with its licence, terms and required notices.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Attribution {
    /// How the response was obtained.
    #[serde(skip)]
    pub meta: ResponseMeta,
    /// The providers.
    #[serde(default)]
    pub providers: Vec<AttributionProvider>,
}

/// A data provider with its licence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct AttributionProvider {
    /// The provider's ID.
    pub id: String,
    /// The name.
    pub name: String,
    /// The credit line to carry.
    pub credit: String,
    /// The licence.
    pub licence: String,
    /// The licence's URL.
    pub licence_url: String,
    /// The terms of use.
    pub terms_url: String,
    /// Whether Ourea modifies the data.
    pub modified: bool,
    /// Notices to carry.
    #[serde(default)]
    pub notices: Vec<String>,
    /// The provider's sources.
    #[serde(default)]
    pub sources: Vec<AttributionSource>,
}

/// One data source of a provider.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct AttributionSource {
    /// The source's ID.
    pub id: String,
    /// The name.
    pub name: String,
    /// The kind of source.
    pub kind: SourceKind,
    /// The provider's ID.
    pub provider: String,
}

with_meta!(Runs, StormReports, StormReport, Attribution);
