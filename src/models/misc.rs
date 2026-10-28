//! Model runs and attribution.

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
    /// The cursor of the next page; `None` on the last page.
    pub next_cursor: Option<String>,
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

with_meta!(Runs, Attribution);
