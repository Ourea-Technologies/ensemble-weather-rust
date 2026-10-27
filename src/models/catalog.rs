//! The location and station catalog.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::enums::{AdProvider, AdSlotKind, SectionKind};
use crate::meta::{ResponseMeta, with_meta};
use crate::models::LocationRef;

/// One page of the location catalog.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CatalogLocationPage {
    /// How the response was obtained.
    #[serde(skip)]
    pub meta: ResponseMeta,
    /// The records.
    #[serde(default)]
    pub items: Vec<CatalogLocation>,
    /// The cursor of the next page; `None` on the last page.
    pub next_cursor: Option<String>,
}

/// One page of the station catalog.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CatalogStationPage {
    /// How the response was obtained.
    #[serde(skip)]
    pub meta: ResponseMeta,
    /// The records.
    #[serde(default)]
    pub items: Vec<CatalogStation>,
    /// The cursor of the next page; `None` on the last page.
    pub next_cursor: Option<String>,
}

/// A location's catalog record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CatalogLocation {
    /// The location.
    #[serde(flatten)]
    pub location: LocationRef,
    /// The county.
    pub county: Option<String>,
    /// The ISO 3166-1 alpha-2 country code.
    pub country: Option<String>,
    /// Other names.
    #[serde(default)]
    pub aliases: Vec<String>,
    /// A short description.
    pub description_short: Option<String>,
    /// A long description.
    pub description_long: Option<String>,
    /// Notes on the local climate.
    pub microclimate_notes: Option<String>,
    /// The page title.
    pub meta_title: Option<String>,
    /// The page description.
    pub meta_description: Option<String>,
    /// The best seasons to visit.
    #[serde(default)]
    pub best_seasons: Vec<String>,
    /// What drives the local weather.
    #[serde(default)]
    pub key_factors: Vec<String>,
    /// Tags.
    #[serde(default)]
    pub tags: Vec<String>,
    /// External links, as the API sends them.
    pub external_links: Option<Value>,
    /// Whether the location is featured.
    #[serde(default)]
    pub is_featured: bool,
}

/// A location's full catalog record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CatalogLocationDetail {
    /// How the response was obtained.
    #[serde(skip)]
    pub meta: ResponseMeta,
    /// The record.
    #[serde(flatten)]
    pub location: CatalogLocation,
    /// The forecast elevations.
    #[serde(default)]
    pub elevations: Vec<CatalogElevation>,
    /// Photos.
    #[serde(default)]
    pub photos: Vec<CatalogPhoto>,
    /// Webcams.
    #[serde(default)]
    pub webcams: Vec<CatalogWebcam>,
    /// Stations nearby.
    #[serde(default)]
    pub nearby_stations: Vec<CatalogNearbyStation>,
    /// The page template (with `include` `"template"`).
    pub template: Option<CatalogTemplate>,
    /// Content entries (with `include` `"content"`).
    #[serde(default)]
    pub content: Vec<CatalogContentEntry>,
    /// Ad slots (with `include` `"ads"`).
    #[serde(default)]
    pub ads: Vec<CatalogAdSlot>,
}

/// An observation station's catalog record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CatalogStation {
    /// The station as a location.
    #[serde(flatten)]
    pub location: LocationRef,
    /// The county.
    pub county: Option<String>,
    /// The ISO 3166-1 alpha-2 country code.
    pub country: Option<String>,
    /// Other names.
    #[serde(default)]
    pub aliases: Vec<String>,
    /// The network, such as `"Snotel"`.
    pub network: Option<String>,
    /// The network's own station ID.
    pub source_station_id: Option<String>,
    /// The elevation above mean sea level, m.
    pub elevation_msl_m: Option<f64>,
    /// The sensor height above ground, m.
    pub sensor_height_agl_m: Option<f64>,
    /// What the station measures.
    #[serde(default)]
    pub sensor_capabilities: Vec<String>,
    /// The status, such as `"Active"`.
    pub station_status: Option<String>,
    /// The last report's time (RFC 3339).
    pub last_report_at: Option<String>,
    /// The last report, as the API sends it.
    pub last_report_summary: Option<Value>,
    /// The install date (`YYYY-MM-DD`).
    pub install_date: Option<String>,
    /// The decommission date (`YYYY-MM-DD`).
    pub decommission_date: Option<String>,
    /// How often it reports, minutes.
    pub update_frequency_minutes: Option<u32>,
    /// The station's page.
    pub station_url: Option<String>,
    /// The station's data.
    pub data_url: Option<String>,
    /// A short description.
    pub description_short: Option<String>,
    /// A long description.
    pub description_long: Option<String>,
    /// The page title.
    pub meta_title: Option<String>,
    /// The page description.
    pub meta_description: Option<String>,
    /// Tags.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Whether the station is featured.
    #[serde(default)]
    pub is_featured: bool,
}

/// A station's full catalog record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CatalogStationDetail {
    /// How the response was obtained.
    #[serde(skip)]
    pub meta: ResponseMeta,
    /// The record.
    #[serde(flatten)]
    pub station: CatalogStation,
    /// Photos.
    #[serde(default)]
    pub photos: Vec<CatalogPhoto>,
    /// Webcams.
    #[serde(default)]
    pub webcams: Vec<CatalogWebcam>,
    /// The location the station primarily serves.
    pub primary_location: Option<CatalogPrimaryLocation>,
    /// The page template (with `include` `"template"`).
    pub template: Option<CatalogTemplate>,
    /// Content entries (with `include` `"content"`).
    #[serde(default)]
    pub content: Vec<CatalogContentEntry>,
    /// Ad slots (with `include` `"ads"`).
    #[serde(default)]
    pub ads: Vec<CatalogAdSlot>,
}

/// The location a station primarily serves.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CatalogPrimaryLocation {
    /// The slug.
    pub slug: String,
    /// The name.
    pub name: String,
    /// Latitude.
    pub lat: f64,
    /// Longitude.
    pub lon: f64,
    /// Elevation, m.
    pub elevation_m: f64,
    /// The kind of location.
    #[serde(rename = "type")]
    pub kind: String,
}

/// One of a catalog location's forecast elevations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CatalogElevation {
    /// The label, such as `"summit"`.
    pub label: String,
    /// The elevation, m.
    pub elevation_m: f64,
    /// A latitude other than the location's.
    pub lat_override: Option<f64>,
    /// A longitude other than the location's.
    pub lon_override: Option<f64>,
    /// Whether this is the primary elevation.
    pub is_default: bool,
}

/// A catalog photo with its credit and licence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CatalogPhoto {
    /// The image.
    pub url: String,
    /// The caption.
    pub caption: Option<String>,
    /// The credit line.
    pub credit: Option<String>,
    /// The alt text.
    pub alt_text: Option<String>,
    /// The licence.
    pub license: Option<String>,
    /// The licence's URL.
    pub license_url: Option<String>,
}

/// A webcam at a catalog location or station.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CatalogWebcam {
    /// The name.
    pub name: String,
    /// The feed.
    pub url: String,
    /// How to embed it.
    pub embed_type: Option<String>,
    /// The provider.
    pub provider: Option<String>,
    /// How often it refreshes, seconds.
    pub refresh_interval_seconds: Option<u32>,
}

/// An observation station near a catalog location.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CatalogNearbyStation {
    /// The station's slug.
    pub slug: String,
    /// The name.
    pub name: String,
    /// The network.
    pub network: String,
    /// The status.
    pub station_status: String,
    /// The distance, km.
    pub distance_km: Option<f64>,
    /// The elevation difference, m.
    pub elevation_diff_m: Option<f64>,
    /// Whether this is the location's primary station.
    pub is_primary: bool,
}

/// The page template of a catalog record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CatalogTemplate {
    /// The template's ID.
    pub id: String,
    /// The location type it is for.
    pub location_type: Option<String>,
    /// The template variant.
    pub template_variant: Option<String>,
    /// The sections, in order.
    #[serde(default)]
    pub spec: Vec<TemplateSection>,
}

/// One section of a catalog page template.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct TemplateSection {
    /// The kind of section.
    pub kind: SectionKind,
    /// The section's key.
    pub key: String,
    /// Its configuration, as the API sends it.
    pub config: Option<Value>,
}

/// One resolved content entry of a catalog record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CatalogContentEntry {
    /// The entry's ID.
    pub id: String,
    /// The content type.
    pub content_type_key: String,
    /// The title.
    pub title: String,
    /// The body, as the API sends it.
    pub body: Value,
    /// The slot it fills.
    pub slot: String,
    /// Its order within the slot.
    pub display_order: i32,
    /// The locale served.
    pub resolved_locale: String,
    /// When it last changed (RFC 3339).
    pub updated_at: String,
}

/// One resolved ad slot of a catalog record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CatalogAdSlot {
    /// The slot's key.
    pub slot_key: String,
    /// House or third party.
    pub kind: AdSlotKind,
    /// The house ad, for [`AdSlotKind::House`].
    pub house: Option<HousePlacement>,
    /// The third-party unit, for [`AdSlotKind::ThirdParty`].
    pub third_party: Option<ThirdPartyPlacement>,
    /// The sizes, as `[width, height]` pairs.
    #[serde(default)]
    pub sizes: Vec<Vec<u32>>,
}

/// A house ad in a slot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct HousePlacement {
    /// The campaign's ID.
    pub campaign_id: String,
    /// The creative's ID.
    pub creative_id: String,
    /// The creative.
    pub content: AdCreative,
}

/// A house ad's content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct AdCreative {
    /// The headline.
    pub headline: Option<String>,
    /// The body.
    pub body: Option<String>,
    /// The call to action.
    pub cta: Option<String>,
    /// Where a click goes.
    pub click_url: Option<String>,
    /// The photo's ID.
    pub photo_id: Option<String>,
}

/// A third-party ad unit in a slot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ThirdPartyPlacement {
    /// The unit's ID.
    pub unit_id: String,
    /// The provider.
    pub provider: AdProvider,
    /// Its configuration, as the API sends it.
    pub config: Value,
}

with_meta!(
    CatalogLocationPage,
    CatalogStationPage,
    CatalogLocationDetail,
    CatalogStationDetail
);
