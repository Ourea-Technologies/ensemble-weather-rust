//! Places ranked by a metric, and the blend's composition.

use serde::{Deserialize, Serialize};

use crate::enums::{ExploreMetric, Order};
use crate::meta::{ResponseMeta, with_meta};
use crate::models::{LocationRef, ModelEntry};
use crate::units::Units;

/// One page of places ranked by a metric, from
/// [`Client::explore`](crate::Client::explore).
///
/// ```no_run
/// # async fn run(client: ensemble_weather::Client) -> Result<(), ensemble_weather::Error> {
/// use ensemble_weather::ExploreMetric;
/// let page = client.explore(ExploreMetric::Snow5d).state("or").await?;
/// for item in &page.items {
///     println!("{:>3} {} {} {}", item.rank, item.location.name, item.value, page.unit);
/// }
/// # Ok(()) }
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ExplorePage {
    /// How the response was obtained.
    #[serde(skip)]
    pub meta: ResponseMeta,
    /// The metric ranked by.
    pub metric: ExploreMetric,
    /// The order of the ranking.
    pub order: Order,
    /// The unit system of the response.
    pub units: Units,
    /// The unit of every `value`, `p10` and `p90`: `"mm"`, `"cm"`, `"in"` or
    /// `"percent"`.
    pub unit: String,
    /// The ranked points.
    #[serde(default)]
    pub items: Vec<ExploreItem>,
    /// The number of items on this page.
    pub count: u32,
    /// The cursor of the next page; `None` on the last page.
    pub next_cursor: Option<String>,
}

/// One ranked point of a place.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ExploreItem {
    /// The rank, 1-based, across pages.
    pub rank: u32,
    /// The place.
    pub location: LocationRef,
    /// The named elevation; `None` for the place's own point.
    pub elevation_name: Option<String>,
    /// What that elevation is called, for display.
    pub elevation_display_name: Option<String>,
    /// The point's elevation, in the length unit.
    pub elevation: f64,
    /// The metric's value as the place's page serves it, in the page's
    /// `unit`.
    pub value: f64,
    /// The range's low end (`snow_today` and `precip_today` only).
    pub p10: Option<f64>,
    /// The range's high end, as `p10`.
    pub p90: Option<f64>,
    /// When the value was taken (RFC 3339).
    pub as_of: String,
    /// For forecast windows, the served run's init time (RFC 3339).
    pub initialized_at: Option<String>,
    /// Where the value comes from.
    pub source: String,
    /// The distance from `near`, km (with `near` only).
    pub distance_km: Option<f64>,
    /// The place's forecast route.
    pub path: String,
}

/// A place's value for a metric, as [`Client::explore`](crate::Client::explore)
/// ranks it (on a search hit with a metric sort).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ExploreValue {
    /// The metric.
    pub metric: ExploreMetric,
    /// The value, in `unit`.
    pub value: f64,
    /// The unit: `"mm"`, `"cm"`, `"in"` or `"percent"`.
    pub unit: String,
    /// The range's low end, where the metric has one.
    pub p10: Option<f64>,
    /// The range's high end, as `p10`.
    pub p90: Option<f64>,
    /// When the value was taken (RFC 3339).
    pub as_of: String,
    /// For forecast windows, the served run's init time (RFC 3339).
    pub initialized_at: Option<String>,
    /// Where the value comes from.
    pub source: String,
}

/// The blend's served models and members, from
/// [`Client::composition`](crate::Client::composition).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Composition {
    /// How the response was obtained.
    #[serde(skip)]
    pub meta: ResponseMeta,
    /// The reference init time: the newest contributing cycle, as a
    /// forecast's `model_run.initialized_at` (RFC 3339).
    pub initialized_at: String,
    /// Each contributing model, its own cycle and its members.
    #[serde(default)]
    pub models: Vec<ModelEntry>,
    /// The number of models.
    pub model_count: u32,
    /// The members the blend draws on, summed over `models`.
    pub total_members: u32,
}

with_meta!(ExplorePage, Composition);
