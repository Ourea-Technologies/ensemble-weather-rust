//! The GeoJSON map layers: alert outlines and avalanche zones.
//!
//! These bodies are passed through as the API sends them (BEHAVIOR.md 3.0):
//! every key keeps its wire name, and each geometry keeps its raw
//! `coordinates` (nested arrays of numbers), never reprojected or
//! simplified, so a value serialises straight back to GeoJSON for a map
//! library.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::enums::AlertClass;
use crate::meta::{ResponseMeta, with_meta};
use crate::models::ResponseAttribution;

/// A GeoJSON `FeatureCollection` (RFC 7946) of the active winter and wind alerts' outlines,
/// with this response's `attribution` as a foreign member.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct AlertOutlines {
    /// How the response was obtained.
    #[serde(skip)]
    pub meta: ResponseMeta,
    /// `"FeatureCollection"`.
    #[serde(rename = "type")]
    pub kind: String,
    /// One Feature per alert, ordered by `properties.id`.
    #[serde(default)]
    pub features: Vec<AlertOutlineFeature>,
    /// The data sources (a foreign member of the collection).
    pub attribution: ResponseAttribution,
}

/// One alert's outline, a GeoJSON `Feature`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct AlertOutlineFeature {
    /// `"Feature"`.
    #[serde(rename = "type")]
    pub kind: String,
    /// The outline.
    pub geometry: AlertOutlineGeometry,
    /// The alert.
    pub properties: AlertOutlineProperties,
}

/// A GeoJSON `Polygon` or `MultiPolygon`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct AlertOutlineGeometry {
    /// The geometry type, such as `"MultiPolygon"`.
    #[serde(rename = "type")]
    pub kind: String,
    /// The coordinates, as the API sends them (nested arrays of numbers).
    pub coordinates: Value,
}

/// The alert, under the names `ActiveAlert` uses for the same fields, and its `class`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct AlertOutlineProperties {
    /// NWS alert identifier (urn:oid:…), as `ActiveAlert.id`.
    pub id: String,
    /// The event, such as `"Winter Storm Warning"`.
    pub event: String,
    /// The alert's class.
    pub class: AlertClass,
    /// As `ActiveAlert.severity`.
    pub severity: String,
    /// As `ActiveAlert.urgency`.
    pub urgency: String,
    /// As `ActiveAlert.certainty`.
    pub certainty: String,
    /// The headline.
    pub headline: String,
    /// When the hazard begins.
    pub onset: Option<String>,
    /// When NWS is due to re-issue this message; not the alert's end (see
    /// `ActiveAlert.expires`).
    pub expires: String,
    /// When the hazard ends.
    pub ends: Option<String>,
    /// The NWS office.
    pub sender_name: Option<String>,
    /// The NWS forecast page of a point inside the outline, which lists the alert and links to
    /// its text: where "view on NWS" goes.
    pub source_url: String,
}

/// A GeoJSON `FeatureCollection` (RFC 7946) of the avalanche centers' forecast zones, each with
/// its published rating, and this response's `attribution` as a foreign member.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct AvalancheZones {
    /// How the response was obtained.
    #[serde(skip)]
    pub meta: ResponseMeta,
    /// `"FeatureCollection"`.
    #[serde(rename = "type")]
    pub kind: String,
    /// One Feature per zone, the less specific first: where zones overlap, the one drawn last
    /// is the one a point there is served.
    #[serde(default)]
    pub features: Vec<AvalancheZoneFeature>,
    /// The data sources (a foreign member of the collection).
    pub attribution: ResponseAttribution,
}

/// One forecast zone, a GeoJSON `Feature`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct AvalancheZoneFeature {
    /// `"Feature"`.
    #[serde(rename = "type")]
    pub kind: String,
    /// The zone's id, as `properties.zone_id`.
    pub id: i64,
    /// The outline.
    pub geometry: AvalancheZoneGeometry,
    /// The zone and its rating.
    pub properties: AvalancheZoneProperties,
}

/// A GeoJSON `MultiPolygon`: an array of polygons, each an array of rings (the outer one first,
/// then any holes), each ring closed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct AvalancheZoneGeometry {
    /// The geometry type, such as `"MultiPolygon"`.
    #[serde(rename = "type")]
    pub kind: String,
    /// The coordinates, as the API sends them (nested arrays of numbers).
    pub coordinates: Value,
}

/// An avalanche zone's rating: the fields of
/// [`AvalancheForecast`](crate::AvalancheForecast) under their wire names
/// (GeoJSON is passed through untouched, so `forecast_url` keeps its name
/// here).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct AvalancheZoneProperties {
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
    /// The center's full forecast for the zone.
    pub forecast_url: String,
    /// The center's site.
    pub center_url: String,
    /// The credit to show with the rating.
    pub attribution: String,
}

with_meta!(AlertOutlines, AvalancheZones);
