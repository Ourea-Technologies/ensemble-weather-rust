//! A local stand-in for the Ourea API, used to run the examples without
//! network access or a key. It serves the shared vector bodies from
//! `spec/vectors/bodies` and echoes probability and totals requests back with
//! fixed answers. It is a test fixture: its numbers are placeholders, not
//! forecasts.

#![allow(dead_code, clippy::missing_panics_doc)]

use std::fmt::Write as _;
use std::net::SocketAddr;
use std::path::Path;

use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

fn read(rel: &str) -> Option<Value> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}

fn body(name: &str) -> Option<Value> {
    read(&format!("spec/vectors/bodies/{name}"))
}

fn fixture(name: &str) -> Option<Value> {
    read(&format!("tests/fixtures/{name}"))
}

/// Starts the stub on a free local port, on the current Tokio runtime, and
/// returns its base URL.
pub async fn start() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr: SocketAddr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let Ok((sock, _)) = listener.accept().await else {
                return;
            };
            tokio::spawn(handle(sock));
        }
    });
    format!("http://{addr}")
}

/// Starts the stub on its own thread and runtime, for blocking callers.
pub fn start_in_background() -> String {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async move {
            tx.send(start().await).unwrap();
            std::future::pending::<()>().await;
        });
    });
    rx.recv().unwrap()
}

struct Request {
    path: String,
    query: Vec<(String, String)>,
    headers: Vec<(String, String)>,
}

impl Request {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
    fn query(&self, name: &str) -> Option<&str> {
        self.query
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }
}

async fn handle(mut sock: TcpStream) {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    while !buf.windows(4).any(|w| w == b"\r\n\r\n") {
        match sock.read(&mut chunk).await {
            Ok(0) | Err(_) => return,
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
        }
    }
    let text = String::from_utf8_lossy(&buf).into_owned();
    let mut lines = text.split("\r\n");
    let target = lines
        .next()
        .unwrap_or_default()
        .split(' ')
        .nth(1)
        .unwrap_or("/")
        .to_owned();
    let headers = lines
        .take_while(|l| !l.is_empty())
        .filter_map(|l| {
            l.split_once(':')
                .map(|(k, v)| (k.trim().to_owned(), v.trim().to_owned()))
        })
        .collect();
    let url = url::Url::parse(&format!("http://stub{target}")).unwrap();
    let req = Request {
        path: url.path().trim_start_matches("/api/v1/").to_owned(),
        query: url
            .query_pairs()
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect(),
        headers,
    };
    let (status, extra, body) = route(&req);
    let mut head =
        format!("HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nConnection: close\r\n");
    for (k, v) in &extra {
        let _ = write!(head, "{k}: {v}\r\n");
    }
    let _ = write!(head, "Content-Length: {}\r\n\r\n", body.len());
    let _ = sock.write_all(head.as_bytes()).await;
    let _ = sock.write_all(&body).await;
    let _ = sock.shutdown().await;
}

type Reply = (u16, Vec<(String, String)>, Vec<u8>);

fn error(status: u16, code: &str, message: &str) -> Reply {
    let body = json!({"error": code, "message": message})
        .to_string()
        .into_bytes();
    (
        status,
        vec![("X-Request-Id".into(), "stub-request".into())],
        body,
    )
}

fn route(req: &Request) -> Reply {
    let keyed = req.header("X-API-Key").is_some();
    let parts: Vec<&str> = req.path.split('/').collect();
    let value = match parts.as_slice() {
        ["forecast"] => {
            if !keyed {
                return error(
                    403,
                    "PRO_REQUIRED",
                    "Forecasts at arbitrary coordinates need a Pro API key.",
                );
            }
            let lat: f64 = req.query("lat").and_then(|v| v.parse().ok()).unwrap_or(0.0);
            let lon: f64 = req.query("lon").and_then(|v| v.parse().ok()).unwrap_or(0.0);
            if lon > -60.0 || lat < 20.0 {
                return error(
                    400,
                    "OUTSIDE_COVERAGE",
                    &format!("{lat},{lon} is outside the forecast domain (CONUS)."),
                );
            }
            forecast(req).map(|mut f| {
                f["point"] = json!({
                    "requested_lat": lat, "requested_lon": lon, "lat": 45.34, "lon": -121.71,
                    "resolution_deg": 0.01, "elevation": 2600.0, "elevation_source": "request", "state": "or"
                });
                f
            })
        }
        ["locations", _, _] => forecast(req),
        ["locations", _, _, "current"] => fixture("current.json"),
        ["locations", _, _, "alerts"] => fixture("alerts.json"),
        ["search", "locations"] => body("search.json"),
        ["catalog", "locations"] => body(if req.query("cursor") == Some("c2") {
            "catalog-page-2.json"
        } else {
            "catalog-page-1.json"
        }),
        ["catalog", "locations", _] => body("catalog-detail.json"),
        ["attribution"] => body("attribution.json"),
        _ => return error(404, "NOT_FOUND", &format!("No route /api/v1/{}.", req.path)),
    };
    let Some(value) = value else {
        return error(500, "INTERNAL_ERROR", "stub fixture missing");
    };
    let raw = value.to_string().into_bytes();
    let etag = format!("\"stub-{:x}\"", raw.len());
    let mut headers = vec![("ETag".to_owned(), etag.clone())];
    if keyed {
        headers.push(("Cache-Control".into(), "private, max-age=0".into()));
        for (k, v) in [
            ("Limit", "600"),
            ("Remaining", "599"),
            ("Reset", "60"),
            ("Policy", "600;w=60"),
        ] {
            headers.push((format!("RateLimit-{k}"), v.into()));
        }
    } else {
        headers.push((
            "Cache-Control".into(),
            "public, max-age=0, s-maxage=600, stale-while-revalidate=60".into(),
        ));
    }
    if req.header("If-None-Match") == Some(etag.as_str()) {
        return (304, headers, Vec::new());
    }
    (200, headers, raw)
}

fn window(s: &str) -> (u64, u64) {
    let s = s.trim_end_matches('h');
    match s.split_once('-') {
        Some((a, b)) => (a.parse().unwrap_or(0), b.parse().unwrap_or(0)),
        None => (0, s.parse().unwrap_or(0)),
    }
}

fn hour(h: u64) -> String {
    format!("2026-11-{:02}T{:02}:00:00Z", 15 + h / 24, h % 24)
}

const PROBABILITIES: [f64; 4] = [0.42, 0.17, 0.08, 0.63];

fn forecast(req: &Request) -> Option<Value> {
    let mut f = body(if req.query("units") == Some("imperial") {
        "forecast-imperial.json"
    } else {
        "forecast-metric.json"
    })?;
    if let Some(prob) = req.query("prob") {
        let answers: Vec<Value> = prob
            .split(',')
            .enumerate()
            .map(|(i, term)| {
                let p: Vec<&str> = term.split(':').collect();
                let (start, end) = window(p[3]);
                let default = if matches!(p[0], "precipitation" | "snowfall") {
                    "total"
                } else {
                    "any"
                };
                json!({
                    "variable": p[0], "op": p[1], "threshold": p[2].parse::<f64>().unwrap_or(0.0),
                    "aggregation": p.get(4).copied().unwrap_or(default),
                    "window_start_hours": start, "window_end_hours": end,
                    "from": hour(start), "to": hour(end),
                    "probability": PROBABILITIES[i % 4], "members": 131
                })
            })
            .collect();
        f["probabilities"] = Value::Array(answers);
    }
    if let Some(totals) = req.query("totals") {
        let answers: Vec<Value> = totals
            .split(',')
            .map(|term| {
                let (variable, w) = term.split_once(':').unwrap_or((term, "24h"));
                let (start, end) = window(w);
                json!({
                    "variable": variable, "window_start_hours": start, "window_end_hours": end,
                    "from": hour(start), "to": hour(end),
                    "quantiles": {"mean": 31.0, "p10": 12.0, "p25": 20.0, "p50": 29.0, "p75": 40.0, "p90": 52.0, "min": 4.0, "max": 88.0},
                    "members": 131
                })
            })
            .collect();
        f["window_totals"] = Value::Array(answers);
    }
    Some(f)
}
