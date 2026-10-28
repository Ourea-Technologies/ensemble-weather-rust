//! Spec drift: the pinned OpenAPI spec against the SDK.
//!
//! - every operation in the spec (except `getMapsToken`) is exposed, and the
//!   SDK exposes nothing the spec lacks;
//! - every response example round-trips through the models without losing a
//!   field;
//! - for every operation, a response synthesised with every property of every
//!   schema it reaches decodes, and no property is dropped: a property the
//!   models lack fails here, and so does a model field the spec lacks.

use std::collections::BTreeSet;

use serde_json::{Map, Value, json};
use yaml_rust2::{Yaml, YamlLoader};

use crate::requests::ops;
use crate::{
    Alerts, Attribution, CatalogLocationDetail, CatalogLocationPage, CatalogStationDetail,
    CatalogStationPage, Current, Forecast, LocationsPage, NearbyLocations, NearbyPoint, Runs,
    SearchResults,
};

const OUT_OF_SCOPE: &[&str] = &["getMapsToken"];

/// Response examples the pinned spec gets wrong, reported upstream. Each must
/// still fail to decode, so this list is emptied when the spec is fixed. The
/// pinned spec has none.
const KNOWN_SPEC_DEFECTS: &[(&str, &str)] = &[];

fn spec() -> Value {
    let text = include_str!("../../spec/openapi/v1/openapi.yaml");
    let docs = YamlLoader::load_from_str(text).expect("the spec parses");
    to_json(&docs[0])
}

fn to_json(y: &Yaml) -> Value {
    match y {
        Yaml::Real(r) => json!(r.parse::<f64>().unwrap()),
        Yaml::Integer(i) => json!(i),
        Yaml::String(s) => json!(s),
        Yaml::Boolean(b) => json!(b),
        Yaml::Array(a) => Value::Array(a.iter().map(to_json).collect()),
        Yaml::Hash(h) => Value::Object(
            h.iter()
                .map(|(k, v)| {
                    let key = match k {
                        Yaml::String(s) => s.clone(),
                        Yaml::Integer(i) => i.to_string(),
                        Yaml::Boolean(b) => b.to_string(),
                        other => format!("{other:?}"),
                    };
                    (key, to_json(v))
                })
                .collect(),
        ),
        _ => Value::Null,
    }
}

/// Decodes `v` as the operation's response type and serialises it back.
fn round_trip(operation: &str, v: Value) -> Result<Value, String> {
    fn rt<T: serde::de::DeserializeOwned + serde::Serialize>(v: Value) -> Result<Value, String> {
        let t: T = serde_json::from_value(v).map_err(|e| e.to_string())?;
        serde_json::to_value(&t).map_err(|e| e.to_string())
    }
    match operation {
        "getLocationForecast" | "getPointForecast" => rt::<Forecast>(v),
        "getCurrentConditions" | "getCurrentByCoordinates" => rt::<Current>(v),
        "getLocationAlerts" | "getAlertsByCoordinates" => rt::<Alerts>(v),
        "getNearbyLocations" => rt::<NearbyLocations>(v),
        "getNearbyByCoordinates" => rt::<NearbyPoint>(v),
        "searchLocations" => rt::<SearchResults>(v),
        "getAllLocations" | "getLocations" => rt::<LocationsPage>(v),
        "listCatalogLocations" => rt::<CatalogLocationPage>(v),
        "getCatalogLocation" => rt::<CatalogLocationDetail>(v),
        "listCatalogStations" => rt::<CatalogStationPage>(v),
        "getCatalogStation" => rt::<CatalogStationDetail>(v),
        "getAvailableRuns" => rt::<Runs>(v),
        "getAttribution" => rt::<Attribution>(v),
        other => Err(format!("no model for {other}")),
    }
}

/// Every key of `original` is present at the same place in `decoded`.
/// Free-form JSON (kept as `serde_json::Value`) is compared whole.
fn missing_keys(original: &Value, decoded: &Value, path: &str, out: &mut Vec<String>) {
    match (original, decoded) {
        (Value::Object(o), Value::Object(d)) => {
            for (k, v) in o {
                match d.get(k) {
                    None => out.push(format!("{path}.{k}")),
                    Some(dv) => missing_keys(v, dv, &format!("{path}.{k}"), out),
                }
            }
        }
        (Value::Array(o), Value::Array(d)) => {
            for (i, (ov, dv)) in o.iter().zip(d).enumerate() {
                missing_keys(ov, dv, &format!("{path}[{i}]"), out);
            }
            if o.len() != d.len() {
                out.push(format!("{path}: {} items became {}", o.len(), d.len()));
            }
        }
        _ => {}
    }
}

struct Ops {
    spec: Value,
}

impl Ops {
    fn operations(&self) -> Vec<(String, Value)> {
        let mut out = Vec::new();
        for item in self.spec["paths"].as_object().unwrap().values() {
            for (method, op) in item.as_object().unwrap() {
                if method == "get" {
                    out.push((op["operationId"].as_str().unwrap().to_owned(), op.clone()));
                }
            }
        }
        out
    }

    fn resolve<'a>(&'a self, s: &'a Value) -> &'a Value {
        match s.get("$ref").and_then(Value::as_str) {
            Some(r) => {
                let name = r.rsplit('/').next().unwrap();
                let kind = if r.contains("/schemas/") {
                    "schemas"
                } else {
                    "parameters"
                };
                self.resolve(&self.spec["components"][kind][name])
            }
            None => s,
        }
    }

    /// A value with every property of `schema` (and of every schema it
    /// reaches) set.
    fn synthesise(&self, schema: &Value, depth: usize) -> Value {
        assert!(depth < 40, "schema nesting too deep");
        let s = self.resolve(schema);
        let mut merged = Map::new();
        let mut is_object = false;
        for key in ["allOf", "oneOf", "anyOf"] {
            for part in s.get(key).and_then(Value::as_array).into_iter().flatten() {
                let part = self.resolve(part);
                if part.get("type") == Some(&json!("null")) {
                    continue;
                }
                match self.synthesise(part, depth + 1) {
                    Value::Object(m) => {
                        is_object = true;
                        for (k, v) in m {
                            merged.entry(k).or_insert(v);
                        }
                    }
                    other if merged.is_empty() && !is_object => return other,
                    _ => {}
                }
            }
        }
        let ty = match s.get("type") {
            Some(Value::Array(ts)) => ts
                .iter()
                .find(|t| *t != "null")
                .cloned()
                .unwrap_or(Value::Null),
            Some(t) => t.clone(),
            None if s.get("properties").is_some() => json!("object"),
            None => Value::Null,
        };
        if let Some(props) = s.get("properties").and_then(Value::as_object) {
            for (k, v) in props {
                merged.insert(k.clone(), self.synthesise(v, depth + 1));
            }
            return Value::Object(merged);
        }
        if is_object {
            return Value::Object(merged);
        }
        if let Some(first) = s
            .get("enum")
            .and_then(Value::as_array)
            .and_then(|e| e.iter().find(|v| !v.is_null()))
        {
            return first.clone();
        }
        match ty.as_str() {
            Some("array") => {
                json!([self.synthesise(s.get("items").unwrap_or(&json!({})), depth + 1)])
            }
            Some("integer") => json!(1),
            Some("number") => json!(1.5),
            Some("boolean") => json!(true),
            Some("string") => match s.get("format").and_then(Value::as_str) {
                Some("date-time") => json!("2026-11-15T00:00:00Z"),
                Some("date") => json!("2026-11-15"),
                _ => json!("x"),
            },
            _ => json!({}),
        }
    }

    fn response_schema(op: &Value) -> Option<Value> {
        op["responses"]["200"]["content"]["application/json"]
            .get("schema")
            .cloned()
    }
}

#[test]
fn every_spec_operation_is_exposed_and_no_other() {
    let ops = Ops { spec: spec() };
    let spec_ids: BTreeSet<String> = ops
        .operations()
        .into_iter()
        .map(|(id, _)| id)
        .filter(|id| !OUT_OF_SCOPE.contains(&id.as_str()))
        .collect();
    let sdk_ids: BTreeSet<String> = ops::ALL.iter().map(|o| o.id.to_owned()).collect();
    assert_eq!(spec_ids, sdk_ids);
    // Open routes (`security: []`) are exactly the ones sent keyless by default.
    for (id, op) in ops.operations() {
        if let Some(sdk) = ops::ALL.iter().find(|o| o.id == id) {
            let open = op.get("security") == Some(&json!([]));
            let keyless_by_default = sdk.open;
            let listed_with_key =
                matches!(id.as_str(), "listCatalogLocations" | "listCatalogStations");
            assert_eq!(keyless_by_default, open && !listed_with_key, "{id}");
        }
    }
}

#[test]
fn response_examples_round_trip_without_losing_fields() {
    let ops = Ops { spec: spec() };
    let mut checked = 0;
    for (id, op) in ops.operations() {
        if OUT_OF_SCOPE.contains(&id.as_str()) {
            continue;
        }
        let content = &op["responses"]["200"]["content"]["application/json"];
        let mut examples: Vec<Value> = content.get("example").cloned().into_iter().collect();
        for (name, ex) in content
            .get("examples")
            .and_then(Value::as_object)
            .into_iter()
            .flatten()
        {
            if KNOWN_SPEC_DEFECTS.contains(&(id.as_str(), name.as_str())) {
                assert!(
                    round_trip(&id, ex["value"].clone()).is_err(),
                    "{id}/{name} decodes now: drop it from KNOWN_SPEC_DEFECTS"
                );
                if let Some(point) = ex["value"].get("point") {
                    let decoded: crate::models::PointInfo = serde_json::from_value(point.clone())
                        .unwrap_or_else(|e| panic!("{id}/{name} point block: {e}"));
                    let mut missing = Vec::new();
                    missing_keys(
                        point,
                        &serde_json::to_value(decoded).unwrap(),
                        "point",
                        &mut missing,
                    );
                    assert!(missing.is_empty(), "{missing:?}");
                }
                continue;
            }
            examples.push(ex["value"].clone());
        }
        for example in examples {
            let decoded =
                round_trip(&id, example.clone()).unwrap_or_else(|e| panic!("{id} example: {e}"));
            let mut missing = Vec::new();
            missing_keys(&example, &decoded, &id, &mut missing);
            assert!(missing.is_empty(), "{id} example lost fields: {missing:?}");
            checked += 1;
        }
    }
    assert!(checked >= 1, "no usable response example found");
}

#[test]
fn the_models_have_every_property_of_every_reachable_schema() {
    let ops = Ops { spec: spec() };
    for (id, op) in ops.operations() {
        if OUT_OF_SCOPE.contains(&id.as_str()) {
            continue;
        }
        let schema =
            Ops::response_schema(&op).unwrap_or_else(|| panic!("{id} has no JSON response"));
        let full = ops.synthesise(&schema, 0);
        let decoded =
            round_trip(&id, full.clone()).unwrap_or_else(|e| panic!("{id}: {e}\n{full:#}"));
        let mut missing = Vec::new();
        missing_keys(&full, &decoded, &id, &mut missing);
        assert!(missing.is_empty(), "{id}: the models lack {missing:?}");
        // And the reverse: a model field the spec does not have (a renamed or
        // withdrawn one) fails too.
        let mut extra = Vec::new();
        missing_keys(&decoded, &full, &id, &mut extra);
        assert!(
            extra.is_empty(),
            "{id}: the models have fields the spec lacks: {extra:?}"
        );
    }
}
