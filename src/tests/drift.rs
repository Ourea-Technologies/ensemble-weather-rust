//! Spec drift: the pinned OpenAPI spec against the SDK.
//!
//! - every operation in the spec (except `getMapsToken`) is exposed, and the
//!   SDK exposes nothing the spec lacks;
//! - every response example round-trips through the models without losing a
//!   field;
//! - for every operation, a response synthesised with every property of every
//!   schema it reaches decodes, and no property is dropped: a property the
//!   models lack fails here, and so does a model field the spec lacks.
//!
//! The models serialise the SDK names of BEHAVIOR.md 3.0, so both checks
//! compare them with the spec **through** the rename table, read from the
//! pinned `spec/BEHAVIOR.md`: a row the models do not follow fails, and so
//! does a rename the table does not list. The GeoJSON routes are compared
//! with no renames, as BEHAVIOR.md 3.0 passes them through untouched.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value, json};
use yaml_rust2::{Yaml, YamlLoader};

use crate::requests::ops;
use crate::{
    AlertOutlines, Alerts, Attribution, AvalancheZones, CatalogLocationDetail, CatalogLocationPage,
    CatalogStationDetail, CatalogStationPage, Composition, Current, ErrorCode, ExplorePage,
    Forecast, LocationsPage, NearbyLocations, NearbyPoint, Runs, SearchResults, Season, SnowTrend,
    Snowpack, SnowpackPlume,
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

/// The rename table of BEHAVIOR.md 3.0: holder (a schema, or `Schema.field`
/// for an inline object) -> wire name -> SDK name.
fn rename_table() -> BTreeMap<String, BTreeMap<String, String>> {
    let text = include_str!("../../spec/BEHAVIOR.md");
    let section = text
        .split("### 3.0")
        .nth(1)
        .and_then(|s| s.split("### 3.1").next())
        .expect("BEHAVIOR.md has section 3.0");
    let ticked = |cell: &str| -> Vec<String> {
        cell.split('`')
            .skip(1)
            .step_by(2)
            .map(|s| s.trim_end_matches("[]").to_owned())
            .collect()
    };
    let mut table: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    for line in section.lines().filter(|l| l.starts_with("| `")) {
        let cells: Vec<&str> = line.split('|').map(str::trim).collect();
        let (holders, wires, sdks) = (ticked(cells[1]), ticked(cells[2]), ticked(cells[3]));
        // `ErrorResponse.error` is the SDK error's `code()`, not a model field.
        if holders == ["ErrorResponse"] {
            continue;
        }
        assert_eq!(wires.len(), sdks.len(), "a BEHAVIOR.md 3.0 row: {line}");
        for holder in holders {
            let row = table.entry(holder).or_default();
            for (w, s) in wires.iter().zip(&sdks) {
                row.insert(w.clone(), s.clone());
            }
        }
    }
    assert!(table.len() > 20, "the rename table did not parse");
    table
}

/// Decodes `v` as the operation's response type and serialises it back
/// (in the SDK names). The serialised value must itself decode to the same
/// value, so a model read back from its own JSON is unchanged.
fn round_trip(operation: &str, v: Value) -> Result<Value, String> {
    fn rt<T: serde::de::DeserializeOwned + serde::Serialize>(v: Value) -> Result<Value, String> {
        let t: T = serde_json::from_value(v).map_err(|e| e.to_string())?;
        let out = serde_json::to_value(&t).map_err(|e| e.to_string())?;
        let again: T = serde_json::from_value(out.clone())
            .map_err(|e| format!("its own serialisation does not decode: {e}"))?;
        let again = serde_json::to_value(&again).map_err(|e| e.to_string())?;
        if again != out {
            return Err("its own serialisation decodes to another value".into());
        }
        Ok(out)
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
        "explore" => rt::<ExplorePage>(v),
        "getComposition" => rt::<Composition>(v),
        "getLocationSeason" => rt::<Season>(v),
        "getLocationSnowTrend" => rt::<SnowTrend>(v),
        "getLocationSnowpack" => rt::<Snowpack>(v),
        "getLocationSnowpackPlume" => rt::<SnowpackPlume>(v),
        "getAlertOutlines" => rt::<AlertOutlines>(v),
        "getAvalancheZones" => rt::<AvalancheZones>(v),
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
    renames: BTreeMap<String, BTreeMap<String, String>>,
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

    fn response_content(op: &Value) -> &Value {
        let content = &op["responses"]["200"]["content"];
        content
            .get("application/json")
            .or_else(|| content.get("application/geo+json"))
            .unwrap_or(&Value::Null)
    }

    fn response_schema(op: &Value) -> Option<Value> {
        Self::response_content(op).get("schema").cloned()
    }

    /// Whether the operation's body is GeoJSON, passed through untouched.
    fn is_geojson(op: &Value) -> bool {
        op["responses"]["200"]["content"]
            .get("application/geo+json")
            .is_some()
    }

    /// The schema's name when it is a `$ref` (directly or as the one
    /// non-null alternative of an `allOf`, `oneOf` or `anyOf`).
    fn ref_name(s: &Value) -> Option<&str> {
        if let Some(r) = s.get("$ref").and_then(Value::as_str) {
            return r.rsplit('/').next();
        }
        for key in ["allOf", "oneOf", "anyOf"] {
            let parts: Vec<&Value> = s
                .get(key)
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter(|p| p.get("type") != Some(&json!("null")))
                .collect();
            if let [one] = parts.as_slice() {
                return Self::ref_name(one);
            }
        }
        None
    }

    /// Every property of an object schema, each with the holder its rename
    /// row is keyed by: the schema that declares it (through `allOf`), or
    /// `holder` for an inline part.
    fn properties<'a>(
        &'a self,
        s: &'a Value,
        holder: &str,
        out: &mut Vec<(String, String, &'a Value)>,
    ) {
        let (holder, s) = match Self::ref_name(s) {
            Some(name) if s.get("$ref").is_some() => (name.to_owned(), self.resolve(s)),
            _ => (holder.to_owned(), s),
        };
        for key in ["allOf", "oneOf", "anyOf"] {
            for part in s.get(key).and_then(Value::as_array).into_iter().flatten() {
                self.properties(part, &holder, out);
            }
        }
        for (k, v) in s
            .get("properties")
            .and_then(Value::as_object)
            .into_iter()
            .flatten()
        {
            out.push((holder.clone(), k.clone(), v));
        }
    }

    /// `v` (wire names) under the SDK names of the rename table, walking the
    /// schema alongside. `holder` names an inline object (`Schema.field`).
    fn to_sdk(&self, schema: &Value, v: &Value, holder: &str, geo: bool) -> Value {
        let holder = Self::ref_name(schema).map_or_else(|| holder.to_owned(), str::to_owned);
        let s = self.resolve(schema);
        match v {
            Value::Array(items) => {
                let item_schema = self.item_schema(s);
                Value::Array(
                    items
                        .iter()
                        .map(|i| self.to_sdk(&item_schema, i, &holder, geo))
                        .collect(),
                )
            }
            Value::Object(map) => {
                let mut props = Vec::new();
                self.properties(s, &holder, &mut props);
                let extra = s.get("additionalProperties").cloned().unwrap_or(json!({}));
                let mut out = Map::new();
                for (k, val) in map {
                    let found = props.iter().find(|(_, name, _)| name == k);
                    let (sdk, child, child_holder) = match found {
                        Some((h, name, ps)) => {
                            let sdk = if geo {
                                None
                            } else {
                                self.renames.get(h).and_then(|row| row.get(name)).cloned()
                            };
                            (
                                sdk.unwrap_or_else(|| k.clone()),
                                (*ps).clone(),
                                format!("{h}.{name}"),
                            )
                        }
                        None => (k.clone(), extra.clone(), holder.clone()),
                    };
                    out.insert(sdk, self.to_sdk(&child, val, &child_holder, geo));
                }
                Value::Object(out)
            }
            other => other.clone(),
        }
    }

    /// The items schema of an array schema (through `allOf`, `oneOf` and
    /// `anyOf`), or `{}`.
    fn item_schema(&self, s: &Value) -> Value {
        let s = self.resolve(s);
        if let Some(items) = s.get("items") {
            return items.clone();
        }
        for key in ["allOf", "oneOf", "anyOf"] {
            for part in s.get(key).and_then(Value::as_array).into_iter().flatten() {
                let items = self.item_schema(part);
                if items != json!({}) {
                    return items;
                }
            }
        }
        json!({})
    }
}

fn ops_from_spec() -> Ops {
    Ops {
        spec: spec(),
        renames: rename_table(),
    }
}

#[test]
fn every_spec_operation_is_exposed_and_no_other() {
    let ops = ops_from_spec();
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
    let ops = ops_from_spec();
    let mut checked = 0;
    for (id, op) in ops.operations() {
        if OUT_OF_SCOPE.contains(&id.as_str()) {
            continue;
        }
        let content = Ops::response_content(&op);
        let schema = content.get("schema").cloned().unwrap_or(json!({}));
        let geo = Ops::is_geojson(&op);
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
            let want = ops.to_sdk(&schema, &example, "", geo);
            let mut missing = Vec::new();
            missing_keys(&want, &decoded, &id, &mut missing);
            assert!(missing.is_empty(), "{id} example lost fields: {missing:?}");
            checked += 1;
        }
    }
    assert!(checked >= 1, "no usable response example found");
}

#[test]
fn the_models_have_every_property_of_every_reachable_schema() {
    let ops = ops_from_spec();
    for (id, op) in ops.operations() {
        if OUT_OF_SCOPE.contains(&id.as_str()) {
            continue;
        }
        let schema =
            Ops::response_schema(&op).unwrap_or_else(|| panic!("{id} has no JSON response"));
        let full = ops.synthesise(&schema, 0);
        let decoded =
            round_trip(&id, full.clone()).unwrap_or_else(|e| panic!("{id}: {e}\n{full:#}"));
        // The models serialise the SDK names: compare through the table.
        let want = ops.to_sdk(&schema, &full, "", Ops::is_geojson(&op));
        let mut missing = Vec::new();
        missing_keys(&want, &decoded, &id, &mut missing);
        assert!(missing.is_empty(), "{id}: the models lack {missing:?}");
        // And the reverse: a model field the spec does not have (a renamed or
        // withdrawn one, or a rename the table does not list) fails too.
        let mut extra = Vec::new();
        missing_keys(&decoded, &want, &id, &mut extra);
        assert!(
            extra.is_empty(),
            "{id}: the models have fields the spec lacks: {extra:?}"
        );
    }
}

#[test]
fn every_rename_row_names_a_spec_field() {
    let ops = ops_from_spec();
    let schemas = &ops.spec["components"]["schemas"];
    for (holder, row) in &ops.renames {
        let (schema, field) = holder.split_once('.').unwrap_or((holder.as_str(), ""));
        let mut s = ops.resolve(&schemas[schema]).clone();
        if !field.is_empty() {
            let mut props = Vec::new();
            ops.properties(&s, schema, &mut props);
            let (_, _, inline) = props
                .iter()
                .find(|(_, name, _)| name == field)
                .unwrap_or_else(|| panic!("{holder}: no such field"));
            let next = ops.resolve(inline).clone();
            s = if next.get("items").is_some() {
                ops.item_schema(&next)
            } else {
                next
            };
        }
        let mut props = Vec::new();
        ops.properties(&s, holder, &mut props);
        for wire in row.keys() {
            assert!(
                props.iter().any(|(_, name, _)| name == wire),
                "BEHAVIOR.md 3.0 renames {holder}.{wire}, which the spec does not have"
            );
        }
    }
}

#[test]
fn every_spec_error_code_is_known() {
    let ops = ops_from_spec();
    let codes = ops.spec["components"]["schemas"]["ErrorResponse"]["properties"]["error"]["enum"]
        .as_array()
        .expect("ErrorResponse.error is an enum");
    for code in codes {
        let code = code.as_str().unwrap();
        let parsed = ErrorCode::from(code);
        assert!(!parsed.is_unknown(), "ErrorCode lacks {code}");
        assert!(parsed.category().is_some(), "{code} has no category");
    }
}
