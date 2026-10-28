//! Runs every shared vector in `spec/vectors` (BEHAVIOR 13) through the
//! scripted transport, on Tokio's paused clock with jitter pinned to its
//! upper bound. A vector the runner cannot map is a failure, never a skip.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde_json::{Map, Value, json};

use super::mock::MockTransport;
use crate::{
    Aggregation, Chance, Client, Error, Forecast, Op, Point, Threshold, Total, Unit, Units,
    Variable, Window,
};

/// The number of vectors the pinned spec ships. A vector added or removed
/// upstream fails this test until the runner is looked at.
const EXPECTED_VECTORS: usize = 43;

fn vectors_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("spec/vectors")
}

fn vector_files() -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut dirs = vec![vectors_dir()];
    while let Some(dir) = dirs.pop() {
        for entry in std::fs::read_dir(&dir).expect("spec/vectors is readable") {
            let path = entry.unwrap().path();
            if path.is_dir() {
                if path.file_name().is_some_and(|n| n != "bodies") {
                    dirs.push(path);
                }
            } else if path.extension().is_some_and(|e| e == "json") {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

/// What one call produced, for the expectations.
struct Outcome {
    value: Value,
    forecast: Option<Forecast>,
    items: Option<Vec<String>>,
}

type Fail = String;

macro_rules! ensure {
    ($cond:expr, $($arg:tt)+) => {
        if !$cond {
            return Err(format!($($arg)+));
        }
    };
}

fn unit(raw: &str) -> Result<Unit, Fail> {
    Ok(match raw {
        "mm" => Unit::Millimeter,
        "in" => Unit::Inch,
        "degC" => Unit::Celsius,
        "degF" => Unit::Fahrenheit,
        "K" => Unit::Kelvin,
        "m" => Unit::Meter,
        "ft" => Unit::Foot,
        "m/s" => Unit::MeterPerSecond,
        "mph" => Unit::MilePerHour,
        other => return Err(format!("unknown threshold unit {other:?}")),
    })
}

fn window(v: Option<&Value>) -> Result<Option<Window>, Fail> {
    let Some(v) = v else { return Ok(None) };
    let num = |k: &str| {
        v.get(k)
            .and_then(Value::as_u64)
            .map(|n| u32::try_from(n).unwrap())
    };
    if let Some(h) = num("hours") {
        return Ok(Some(Window::hours(h)));
    }
    match (num("start_hours"), num("end_hours")) {
        (Some(a), Some(b)) => Ok(Some(Window::between(a, b))),
        _ => Err(format!("bad window {v}")),
    }
}

fn chance(v: &Value) -> Result<Chance, Fail> {
    let variable = Variable::from(v["variable"].as_str().ok_or("chance without variable")?);
    let op = Op::from(v["op"].as_str().ok_or("chance without op")?);
    let t = &v["threshold"];
    let threshold = Threshold::new(
        t["value"].as_f64().ok_or("bad threshold")?,
        unit(t["unit"].as_str().ok_or("no unit")?)?,
    );
    let mut c = Chance::new(variable, op, threshold);
    if let Some(w) = window(v.get("window"))? {
        c = c.window(w);
    }
    if let Some(a) = v.get("aggregation").and_then(Value::as_str) {
        c = c.aggregation(Aggregation::from(a));
    }
    Ok(c)
}

fn total(v: &Value) -> Result<Total, Fail> {
    let mut t = Total::new(Variable::from(
        v["variable"].as_str().ok_or("total without variable")?,
    ));
    if let Some(w) = window(v.get("window"))? {
        t = t.window(w);
    }
    Ok(t)
}

/// Fails on any argument the runner does not map, so no argument is ever
/// silently ignored.
fn check_args(args: &Map<String, Value>, known: &[&str]) -> Result<(), Fail> {
    for k in args.keys() {
        ensure!(
            known.contains(&k.as_str()),
            "the runner does not map argument {k:?}"
        );
    }
    Ok(())
}

fn to_value<T: serde::Serialize>(v: &T, meta: &crate::ResponseMeta) -> Value {
    let mut value = serde_json::to_value(v).unwrap();
    if let Value::Object(map) = &mut value {
        map.insert("revalidated".into(), Value::Bool(meta.revalidated));
        let rl = meta.rate_limit.as_ref();
        map.insert(
            "rate_limit".into(),
            json!({
                "limit": rl.and_then(|r| r.limit),
                "remaining": rl.and_then(|r| r.remaining),
                "reset": rl.and_then(|r| r.reset),
                "policy": rl.and_then(|r| r.policy.clone()),
            }),
        );
    }
    value
}

async fn run_call(client: &Client, call: &Value) -> Result<Result<Outcome, Error>, Fail> {
    let client = match call.get("with_api_key").and_then(Value::as_str) {
        Some(key) => client.with_api_key(key),
        None => client.clone(),
    };
    let op = call["operation"].as_str().ok_or("call without operation")?;
    let empty = Map::new();
    let args = call
        .get("args")
        .and_then(Value::as_object)
        .unwrap_or(&empty);
    let text = |k: &str| args.get(k).and_then(Value::as_str);
    let flag = |k: &str| args.get(k).and_then(Value::as_bool);
    let int = |k: &str| {
        args.get(k)
            .and_then(Value::as_u64)
            .map(|n| u32::try_from(n).unwrap())
    };
    let float = |k: &str| args.get(k).and_then(Value::as_f64);
    Ok(match op {
        "getLocationForecast" | "getPointForecast" => {
            let mut req = if op == "getLocationForecast" {
                check_args(
                    args,
                    &[
                        "location",
                        "state",
                        "slug",
                        "units",
                        "members",
                        "max_hours",
                        "nowcast",
                        "include_past",
                        "daily",
                        "hourly",
                        "models",
                        "prob",
                        "totals",
                    ],
                )?;
                match (text("location"), text("state"), text("slug")) {
                    (Some(loc), None, None) => client.forecast(loc),
                    (None, Some(state), Some(slug)) => client.forecast((state, slug)),
                    _ => return Err("a location forecast needs location or state + slug".into()),
                }
            } else {
                check_args(args, &["lat", "lon", "elevation_m", "max_hours"])?;
                let mut p =
                    Point::new(float("lat").ok_or("no lat")?, float("lon").ok_or("no lon")?);
                if let Some(e) = float("elevation_m") {
                    p = p.elevation_m(e);
                }
                client.forecast_at(p)
            };
            if let Some(u) = text("units") {
                req = req.units(Units::from(u));
            }
            if let Some(v) = int("max_hours") {
                req = req.max_hours(v);
            }
            for (k, setter) in [
                ("members", ForecastSetter::Members),
                ("nowcast", ForecastSetter::Nowcast),
                ("include_past", ForecastSetter::IncludePast),
                ("daily", ForecastSetter::Daily),
                ("hourly", ForecastSetter::Hourly),
                ("models", ForecastSetter::Models),
            ] {
                if let Some(v) = flag(k) {
                    req = setter.apply(req, v);
                }
            }
            for c in args
                .get("prob")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                req = req.chance(chance(c)?);
            }
            for t in args
                .get("totals")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                req = req.total(total(t)?);
            }
            req.await.map(|f| Outcome {
                value: to_value(&f, &f.meta),
                forecast: Some(f),
                items: None,
            })
        }
        "searchLocations" => {
            check_args(args, &["q"])?;
            client
                .search(text("q").ok_or("no q")?)
                .await
                .map(|r| Outcome {
                    value: to_value(&r, &r.meta),
                    forecast: None,
                    items: None,
                })
        }
        "getCatalogLocation" => {
            check_args(args, &["slug"])?;
            client
                .catalog_location(text("slug").ok_or("no slug")?)
                .await
                .map(|r| Outcome {
                    value: to_value(&r, &r.meta),
                    forecast: None,
                    items: None,
                })
        }
        "getAttribution" => {
            check_args(args, &[])?;
            client.attribution().await.map(|r| Outcome {
                value: to_value(&r, &r.meta),
                forecast: None,
                items: None,
            })
        }
        "listCatalogLocations" => {
            check_args(args, &["iterate"])?;
            if flag("iterate") == Some(true) {
                client
                    .catalog_locations()
                    .items()
                    .try_collect()
                    .await
                    .map(|items| Outcome {
                        value: Value::Null,
                        forecast: None,
                        items: Some(items.into_iter().filter_map(|l| l.location.slug).collect()),
                    })
            } else {
                client.catalog_locations().await.map(|r| Outcome {
                    value: to_value(&r, &r.meta),
                    forecast: None,
                    items: None,
                })
            }
        }
        other => return Err(format!("the runner does not map operation {other:?}")),
    })
}

#[derive(Clone, Copy)]
enum ForecastSetter {
    Members,
    Nowcast,
    IncludePast,
    Daily,
    Hourly,
    Models,
}

impl ForecastSetter {
    fn apply(self, r: crate::ForecastRequest<'_>, v: bool) -> crate::ForecastRequest<'_> {
        match self {
            Self::Members => r.members(v),
            Self::Nowcast => r.nowcast(v),
            Self::IncludePast => r.include_past(v),
            Self::Daily => r.daily(v),
            Self::Hourly => r.hourly(v),
            Self::Models => r.models(v),
        }
    }
}

/// Reads a dot-and-index path such as `probabilities[0].threshold`; `None`
/// when any step is missing.
fn lookup<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    let mut cur = value;
    for part in path.split('.') {
        let (name, rest) = part.split_once('[').map_or((part, ""), |(n, r)| (n, r));
        if !name.is_empty() {
            cur = cur.get(name)?;
        }
        for idx in rest.split('[').filter(|s| !s.is_empty()) {
            cur = cur.get(idx.trim_end_matches(']').parse::<usize>().ok()?)?;
        }
    }
    Some(cur)
}

fn check_result(expect: &Map<String, Value>, outcome: &Outcome) -> Result<(), Fail> {
    for (path, want) in expect {
        let got = lookup(&outcome.value, path).unwrap_or(&Value::Null);
        match want {
            Value::Null => ensure!(got.is_null(), "{path}: want absent, got {got}"),
            Value::Number(w) => {
                let (w, g) = (w.as_f64().unwrap(), got.as_f64());
                ensure!(
                    g.is_some_and(|g| (g - w).abs() < 1e-9),
                    "{path}: want {w}, got {got}"
                );
            }
            Value::Object(o) if o.contains_key("unknown") => {
                let raw = o["unknown"].as_str().unwrap();
                ensure!(
                    got.as_str() == Some(raw),
                    "{path}: want raw {raw:?}, got {got}"
                );
                let typed_unknown = match path.as_str() {
                    "location.type" => outcome
                        .forecast
                        .as_ref()
                        .and_then(|f| f.location.kind.as_ref())
                        .map(crate::LocationType::is_unknown),
                    _ => None,
                };
                ensure!(
                    typed_unknown == Some(true),
                    "{path}: not parsed to the Unknown case (or path not mapped)"
                );
            }
            other => ensure!(got == other, "{path}: want {other}, got {got}"),
        }
    }
    Ok(())
}

fn check_error(want: &Map<String, Value>, err: &Error) -> Result<(), Fail> {
    for (k, v) in want {
        match k.as_str() {
            "category" => ensure!(
                Some(err.category().as_str()) == v.as_str(),
                "category: want {v}, got {}",
                err.category()
            ),
            "code" => {
                let got = err.code().map(crate::ErrorCode::as_str);
                ensure!(got == v.as_str(), "code: want {v}, got {got:?}");
            }
            "status" => ensure!(
                err.status().map(u64::from) == v.as_u64(),
                "status: want {v}, got {:?}",
                err.status()
            ),
            "retry_after" => {
                let got = err.retry_after().map(|d| d.as_secs_f64());
                ensure!(got == v.as_f64(), "retry_after: want {v}, got {got:?}");
            }
            "string_excludes" => {
                let needle = v.as_str().unwrap();
                let text = format!("{err} {err:?}");
                ensure!(
                    !text.contains(needle),
                    "the error's text contains {needle:?}: {text}"
                );
            }
            other => {
                return Err(format!(
                    "the runner does not map error expectation {other:?}"
                ));
            }
        }
    }
    Ok(())
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Ok(b) = u8::from_str_radix(&s[i + 1..i + 3], 16)
        {
            out.push(b);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap()
}

fn check_request(want: &Value, req: &crate::transport::HttpRequest) -> Result<(), Fail> {
    let url = &req.url;
    let after_scheme = url.split_once("://").map_or(url.as_str(), |(_, r)| r);
    let path_and_query = after_scheme.find('/').map_or("/", |i| &after_scheme[i..]);
    let (path, query) = path_and_query
        .split_once('?')
        .unwrap_or((path_and_query, ""));
    let pairs: Vec<(String, String)> = query
        .split('&')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let (k, v) = p.split_once('=').unwrap_or((p, ""));
            (percent_decode(k), percent_decode(v))
        })
        .collect();
    let Value::Object(want) = want else {
        return Err(format!("bad expect_request {want}"));
    };
    for (k, v) in want {
        match k.as_str() {
            "method" => ensure!(v == "GET", "method: want {v}"),
            "path" => ensure!(Some(path) == v.as_str(), "path: want {v}, got {path:?}"),
            "query_string" => ensure!(
                Some(query) == v.as_str(),
                "query_string: want {v}, got {query:?}"
            ),
            "url_prefix" => ensure!(
                url.starts_with(v.as_str().unwrap()),
                "url_prefix: want {v}, got {url}"
            ),
            "query" => {
                for (name, value) in v.as_object().unwrap() {
                    let got: Vec<&str> = pairs
                        .iter()
                        .filter(|(k, _)| k == name)
                        .map(|(_, v)| v.as_str())
                        .collect();
                    ensure!(
                        got == [value.as_str().unwrap()],
                        "query {name}: want {value}, got {got:?}"
                    );
                }
            }
            "query_absent" => {
                for name in v.as_array().unwrap() {
                    ensure!(
                        !pairs.iter().any(|(k, _)| k == name),
                        "query {name} must be absent: {query}"
                    );
                }
            }
            "headers" => {
                for (name, value) in v.as_object().unwrap() {
                    ensure!(
                        req.header(name) == value.as_str(),
                        "header {name}: want {value}, got {:?}",
                        req.header(name)
                    );
                }
            }
            "headers_absent" => {
                for name in v.as_array().unwrap() {
                    let name = name.as_str().unwrap();
                    ensure!(
                        req.header(name).is_none(),
                        "header {name} must be absent, got {:?}",
                        req.header(name)
                    );
                }
            }
            "headers_match" => {
                for (name, pattern) in v.as_object().unwrap() {
                    let re = regex::Regex::new(pattern.as_str().unwrap()).unwrap();
                    let got = req.header(name).unwrap_or_default();
                    ensure!(
                        re.is_match(got),
                        "header {name}: {got:?} does not match {pattern}"
                    );
                }
            }
            other => {
                return Err(format!(
                    "the runner does not map request expectation {other:?}"
                ));
            }
        }
    }
    Ok(())
}

async fn run_vector(path: &Path) -> Result<(), Fail> {
    let case: Value =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).map_err(|e| e.to_string())?;
    let cfg = case.get("client").cloned().unwrap_or(json!({}));
    let mock = MockTransport::new();
    for r in case["responses"].as_array().ok_or("no responses")? {
        let status = u16::try_from(r["status"].as_u64().ok_or("no status")?).unwrap();
        let headers: Vec<(String, String)> = r
            .get("headers")
            .and_then(Value::as_object)
            .into_iter()
            .flatten()
            .map(|(k, v)| (k.clone(), v.as_str().unwrap().to_owned()))
            .collect();
        let body = match (
            r.get("body_file").and_then(Value::as_str),
            r.get("body").and_then(Value::as_str),
        ) {
            (Some(file), _) => {
                std::fs::read(vectors_dir().join(file)).map_err(|e| format!("{file}: {e}"))?
            }
            (None, Some(body)) => body.as_bytes().to_vec(),
            (None, None) => Vec::new(),
        };
        let pairs: Vec<(&str, &str)> = headers
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        mock.push(status, &pairs, body);
    }

    let warnings = Arc::new(Mutex::new(Vec::<String>::new()));
    let sink = warnings.clone();
    let mut builder = Client::builder()
        .transport(mock.clone())
        .jitter(|| 1.0)
        .on_deprecation(move |d| sink.lock().unwrap().push(d.to_string()));
    for (k, v) in cfg.as_object().ok_or("bad client")? {
        builder = match k.as_str() {
            "api_key" => builder.api_key(v.as_str().unwrap()),
            "base_url" => builder.base_url(v.as_str().unwrap()),
            "max_retries" => builder.max_retries(u32::try_from(v.as_u64().unwrap()).unwrap()),
            "cache" if v == false => builder.no_cache(),
            "send_key_on_open_routes" => builder.send_key_on_open_routes(v.as_bool().unwrap()),
            "cache" | "env" => builder,
            other => return Err(format!("the runner does not map client setting {other:?}")),
        };
    }
    // The vector's environment only: the real one never leaks in.
    let env: HashMap<String, String> = cfg
        .get("env")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .map(|(k, v)| (k.clone(), v.as_str().unwrap().to_owned()))
        .collect();
    let client = builder
        .build_with_env(&|name| env.get(name).cloned())
        .map_err(|e| e.to_string())?;

    let calls: Vec<Value> = match (case.get("call"), case.get("calls")) {
        (Some(c), None) => vec![c.clone()],
        (None, Some(Value::Array(cs))) => cs.clone(),
        _ => return Err("a vector needs call or calls".into()),
    };
    let mut outcomes = Vec::new();
    for call in &calls {
        outcomes.push(run_call(&client, call).await?);
    }

    let requests = mock.requests();
    let expected_requests: Vec<Value> =
        match (case.get("expect_request"), case.get("expect_requests")) {
            (Some(Value::Null), _) => Vec::new(),
            (Some(r), None) => vec![r.clone()],
            (None, Some(Value::Array(rs))) => rs.clone(),
            _ => return Err("a vector needs expect_request or expect_requests".into()),
        };
    if case.get("expect_request") == Some(&Value::Null) {
        ensure!(
            requests.is_empty(),
            "nothing may be sent, but {} requests were",
            requests.len()
        );
    }
    for (i, want) in expected_requests.iter().enumerate() {
        let (_, req) = requests
            .get(i)
            .ok_or_else(|| format!("request {i} was never sent"))?;
        check_request(want, req).map_err(|e| format!("request {i}: {e}"))?;
    }

    let expect = case
        .get("expect")
        .and_then(Value::as_object)
        .ok_or("no expect")?;
    for (k, v) in expect {
        match k.as_str() {
            "requests" => ensure!(
                Some(requests.len() as u64) == v.as_u64(),
                "requests: want {v}, got {}",
                requests.len()
            ),
            "result" | "results" => {
                let wants: Vec<&Value> = if k == "result" {
                    vec![v]
                } else {
                    v.as_array().unwrap().iter().collect()
                };
                ensure!(
                    wants.len() == outcomes.len(),
                    "{} results expected for {} calls",
                    wants.len(),
                    outcomes.len()
                );
                for (i, (want, outcome)) in wants.iter().zip(&outcomes).enumerate() {
                    let outcome = outcome
                        .as_ref()
                        .map_err(|e| format!("call {i} failed: {e}"))?;
                    check_result(want.as_object().unwrap(), outcome)
                        .map_err(|e| format!("result {i}: {e}"))?;
                }
            }
            "error" => {
                let last = outcomes.last().ok_or("no call")?;
                let err = last.as_ref().err().ok_or("want an error, got a result")?;
                check_error(v.as_object().unwrap(), err)?;
            }
            "items" => {
                let outcome = outcomes[0]
                    .as_ref()
                    .map_err(|e| format!("iterate failed: {e}"))?;
                let got = outcome.items.clone().ok_or("no items collected")?;
                let want: Vec<String> = v
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|s| s.as_str().unwrap().to_owned())
                    .collect();
                ensure!(got == want, "items: want {want:?}, got {got:?}");
            }
            "delays_s" => {
                let got: Vec<f64> = requests
                    .windows(2)
                    .map(|w| (w[1].0 - w[0].0).as_secs_f64())
                    .filter(|d| *d > 0.0)
                    .collect();
                let want: Vec<f64> = v
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|d| d.as_f64().unwrap())
                    .collect();
                ensure!(
                    got.len() == want.len()
                        && got.iter().zip(&want).all(|(g, w)| (g - w).abs() < 1e-6),
                    "delays: want {want:?}, got {got:?}"
                );
            }
            "warnings" => {
                let got = warnings.lock().unwrap().clone();
                let want = v.as_array().unwrap();
                ensure!(
                    got.len() == want.len(),
                    "warnings: want {}, got {got:?}",
                    want.len()
                );
                for (g, w) in got.iter().zip(want) {
                    let needle = w["contains"].as_str().unwrap();
                    ensure!(g.contains(needle), "warning {g:?} lacks {needle:?}");
                }
            }
            other => return Err(format!("the runner does not map expectation {other:?}")),
        }
    }
    if !expect.contains_key("error") {
        for (i, o) in outcomes.iter().enumerate() {
            if let Err(e) = o {
                return Err(format!("call {i} failed unexpectedly: {e}"));
            }
        }
    }
    Ok(())
}

#[tokio::test(start_paused = true)]
async fn every_shared_vector_passes() {
    let files = vector_files();
    let mut failures = Vec::new();
    for file in &files {
        let name = file
            .strip_prefix(vectors_dir())
            .unwrap()
            .display()
            .to_string();
        match run_vector(file).await {
            Ok(()) => println!("ok   {name}"),
            Err(e) => {
                println!("FAIL {name}: {e}");
                failures.push(format!("{name}: {e}"));
            }
        }
    }
    println!(
        "{}/{} vectors passed",
        files.len() - failures.len(),
        files.len()
    );
    assert!(
        failures.is_empty(),
        "{} vectors failed:\n{}",
        failures.len(),
        failures.join("\n")
    );
    assert_eq!(
        files.len(),
        EXPECTED_VECTORS,
        "the pinned spec's vector count changed"
    );
}

#[test]
fn percent_decoding() {
    assert_eq!(
        percent_decode("Mt.%20Hood%20%26%20co%2F%C3%B1"),
        "Mt. Hood & co/ñ"
    );
}
