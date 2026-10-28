//! The SDK's own guarantees, beyond the shared vectors: every operation's
//! canonical URL, credentials, retries, the cache, paging and the bounds.

use std::sync::Arc;
use std::time::Duration;

use super::mock::MockTransport;
use crate::error::ConnectionErrorKind;
use crate::{
    Category, Chance, Client, Error, ErrorCode, LocationType, NullReason, Point, StationNetwork,
    StationStatus, TemplateVariant, Threshold, Total, Units, Variable, Window,
};

const FORECAST: &str = include_str!("../../spec/vectors/bodies/forecast-metric.json");
const SEARCH: &str = include_str!("../../spec/vectors/bodies/search.json");
const PAGE_1: &str = include_str!("../../spec/vectors/bodies/catalog-page-1.json");

fn client(mock: &Arc<MockTransport>) -> crate::ClientBuilder {
    let mock: Arc<dyn crate::transport::Transport> = mock.clone();
    Client::builder()
        .transport(mock)
        .jitter(|| 1.0)
        .base_url("https://api.example.test")
}

fn build(builder: crate::ClientBuilder) -> Client {
    builder.build_with_env(&|_| None).unwrap()
}

#[tokio::test(start_paused = true)]
async fn every_operation_sends_its_canonical_url() {
    let mock = MockTransport::new();
    let c = build(client(&mock).api_key("ow_k"));
    let ok = "{}";
    for _ in 0..40 {
        mock.push_json(ok);
    }
    let p = Point::new(45.0, -121.5);
    let _ = c
        .current("or/mt-hood-meadows")
        .include_nowcast_hours(true)
        .units(Units::Imperial)
        .await;
    let _ = c.current_at(p.elevation_m(1500.0)).await;
    let _ = c.alerts("OR/mt-hood-meadows").await;
    let _ = c.alerts_at(p).await;
    let _ = c
        .nearby("or/mt-hood-meadows")
        .limit(3)
        .radius_km(50.0)
        .await;
    let _ = c.nearby_at(p).limit(5).radius_km(25.5).await;
    let _ = c
        .search("hood")
        .state("or")
        .kind(LocationType::SkiArea)
        .limit(20)
        .await;
    let _ = c.locations().await;
    let _ = c
        .locations()
        .state("OR")
        .kind(LocationType::Summit)
        .limit(100)
        .cursor("x")
        .await;
    let _ = c
        .catalog_locations()
        .admin1("US-OR")
        .country("US")
        .kind(LocationType::SkiArea)
        .template(TemplateVariant::Mountain)
        .limit(20)
        .await;
    let _ = c
        .catalog_location("mt-hood-meadows")
        .include(["template", "content"])
        .locale("en")
        .await;
    let _ = c
        .catalog_stations()
        .networks([StationNetwork::Snotel, StationNetwork::Raws])
        .capabilities(["swe", "snow_depth", "swe"])
        .status(StationStatus::Active)
        .limit(50)
        .await;
    let _ = c.catalog_station("a-station").locale("es").await;
    let _ = c.runs().limit(10).await;
    let _ = c.runs().limit(3).await;
    let _ = c.attribution().await;
    let _ = c
        .forecast("or/mt-hood-meadows")
        .elevations(["all"])
        .feels_like(["nws"])
        .diagnostics(["provenance"])
        .sources(["snotel", "mesonet"])
        .run("2026-11-15T00:00:00Z")
        .max_days(7)
        .await;
    let _ = c
        .forecast_at(Point::new(45.3311, -121.6649).elevation_m(1661.0))
        .include(["skin_temperature", "mslp", "mslp"])
        .await;
    let base = "https://api.example.test/api/v1";
    let want = [
        "/locations/or/mt-hood-meadows/current?include=nowcast_hours&units=imperial",
        "/current?elevation_m=1500&lat=45&lon=-121.5",
        "/locations/or/mt-hood-meadows/alerts",
        "/alerts?lat=45&lon=-121.5",
        "/locations/or/mt-hood-meadows/nearby",
        "/nearby?lat=45&limit=5&lon=-121.5&radius_km=25.5",
        "/search/locations?q=hood&state=or&type=ski_area",
        "/locations",
        "/locations/or?cursor=x&limit=100&type=summit",
        "/catalog/locations?admin1=US-OR&country=US&template=mountain&type=ski_area",
        "/catalog/locations/mt-hood-meadows?include=template%2Ccontent",
        "/catalog/stations?capability=snow_depth%2Cswe&limit=50&network=raws%2Csnotel&status=active",
        "/catalog/stations/a-station?locale=es",
        "/runs",
        "/runs?limit=3",
        "/attribution",
        "/locations/or/mt-hood-meadows?diagnostics=provenance&elevations=all&max_days=7&run=2026-11-15T00%3A00%3A00Z&sources=snotel%2Cmesonet",
        "/forecast?elevation_m=1661&include=mslp%2Cskin_temperature&lat=45.3311&lon=-121.6649",
    ];
    let urls = mock.urls();
    assert_eq!(urls.len(), want.len());
    for (got, want) in urls.iter().zip(want) {
        assert_eq!(got, &format!("{base}{want}"));
    }
    // Open routes go keyless; the rest carry the key.
    let keyed: Vec<bool> = mock
        .requests()
        .iter()
        .map(|(_, r)| r.header("X-API-Key").is_some())
        .collect();
    assert_eq!(
        keyed,
        [
            true, true, true, true, true, true, false, true, true, true, false, true, false, true,
            true, false, true, true
        ]
    );
}

#[tokio::test(start_paused = true)]
async fn usage_errors_are_raised_before_sending() {
    let mock = MockTransport::new();
    let c = build(client(&mock));
    let cases: Vec<Error> = vec![
        c.forecast("no-slash").await.unwrap_err(),
        c.forecast("a/b/c").await.unwrap_err(),
        c.forecast_at(Point::new(45.0, -121.0))
            .elevations(["all"])
            .await
            .unwrap_err(),
        c.alerts_at(Point::new(45.0, -121.0).elevation_m(10.0))
            .await
            .unwrap_err(),
        c.nearby_at(Point::new(45.0, -121.0).elevation_m(10.0))
            .await
            .unwrap_err(),
        c.forecast("or/x")
            .total(Total::new(Variable::WindSpeed))
            .await
            .unwrap_err(),
        c.forecast("or/x")
            .chance(
                Chance::at_least(Variable::Snowfall, Threshold::inches(1.0))
                    .window(Window::between(10, 5)),
            )
            .await
            .unwrap_err(),
    ];
    for e in cases {
        assert_eq!(e.category(), Category::Usage, "{e}");
    }
    assert!(mock.requests().is_empty());
    assert!(
        Client::builder()
            .base_url("ftp://nope")
            .build_with_env(&|_| None)
            .is_err()
    );
}

#[tokio::test(start_paused = true)]
async fn connection_errors_are_retried_but_tls_failures_are_not() {
    let mock = MockTransport::new();
    let c = build(client(&mock));
    mock.push_failure(ConnectionErrorKind::Other);
    mock.push_failure(ConnectionErrorKind::Timeout);
    mock.push_json(FORECAST);
    assert!(c.forecast("or/mt-hood-meadows").await.is_ok());
    assert_eq!(mock.requests().len(), 3);

    let mock = MockTransport::new();
    let c = build(client(&mock));
    mock.push_failure(ConnectionErrorKind::Tls);
    let err = c.forecast("or/mt-hood-meadows").await.unwrap_err();
    assert!(
        matches!(&err, Error::Connection(e) if e.kind() == ConnectionErrorKind::Tls),
        "{err}"
    );
    assert_eq!(mock.requests().len(), 1);
}

#[tokio::test(start_paused = true)]
async fn retry_after_http_date_and_504_are_honoured() {
    let mock = MockTransport::new();
    let c = build(client(&mock));
    mock.push(504, &[], "gateway timeout");
    mock.push(
        503,
        &[("Retry-After", "Thu, 01 Jan 1970 00:00:00 GMT")],
        "{}",
    );
    mock.push_json(FORECAST);
    assert!(c.forecast("or/mt-hood-meadows").await.is_ok());
    assert_eq!(mock.requests().len(), 3);

    // A 504's Retry-After sets the delay exactly (BEHAVIOR 9).
    let mock = MockTransport::new();
    let c = build(client(&mock));
    mock.push(504, &[("Retry-After", "7")], "{}");
    mock.push_json(FORECAST);
    let start = tokio::time::Instant::now();
    assert!(c.forecast("or/mt-hood-meadows").await.is_ok());
    assert_eq!(start.elapsed(), Duration::from_secs(7));
}

#[tokio::test(start_paused = true)]
async fn error_bodies_carry_request_id_doc_url_param_and_nearest_location() {
    let mock = MockTransport::new();
    let c = build(client(&mock));
    mock.push(
        403,
        &[("X-Request-Id", "header-id")],
        r#"{"error":"PRO_REQUIRED","message":"pro","request_id":"body-id",
            "doc_url":"https://docs.oureawx.com/features/errors/#pro_required",
            "nearest_location":{"slug":"mt-hood-meadows","url":"/api/v1/locations/or/mt-hood-meadows",
            "distance_km":4.25,"elevation_m":1635.6}}"#,
    );
    let err = c.forecast_at(Point::new(45.33, -121.67)).await.unwrap_err();
    let api = err.api_error().unwrap();
    assert_eq!(api.code(), Some(&ErrorCode::ProRequired));
    assert_eq!(api.request_id(), Some("body-id"));
    assert_eq!(
        api.doc_url(),
        Some("https://docs.oureawx.com/features/errors/#pro_required")
    );
    let near = api.nearest_location().unwrap();
    assert_eq!(
        (near.slug.as_str(), near.distance_km),
        ("mt-hood-meadows", 4.25)
    );

    mock.push(
        400,
        &[("X-Request-Id", "header-id")],
        r#"{"error":"BAD_REQUEST","message":"bad","param":"max_hours"}"#,
    );
    let err = c.forecast("or/mt-hood-meadows").await.unwrap_err();
    let api = err.api_error().unwrap();
    assert_eq!(api.param(), Some("max_hours"));
    assert_eq!(api.request_id(), Some("header-id"));
    assert!(api.nearest_location().is_none());

    // Codes the public spec dropped keep their string and map by status.
    mock.push(410, &[], r#"{"error":"GONE","message":"gone"}"#);
    let err = c.forecast("or/mt-hood-meadows").await.unwrap_err();
    assert_eq!(err.category(), Category::NotFound);
    assert_eq!(err.code().map(ErrorCode::as_str), Some("GONE"));
    mock.push(
        503,
        &[],
        r#"{"error":"AUTH_UNAVAILABLE","message":"later"}"#,
    );
    let c = build(client(&mock).max_retries(0));
    let err = c.forecast("or/mt-hood-meadows").await.unwrap_err();
    assert_eq!(err.category(), Category::Unavailable);
}

#[tokio::test(start_paused = true)]
async fn the_key_is_never_echoed_in_an_error() {
    let mock = MockTransport::new();
    let c = build(client(&mock).api_key("ow_leaky_key"));
    mock.push(
        401,
        &[("X-Request-Id", "req-1")],
        r#"{"error":"INVALID_API_KEY","message":"key ow_leaky_key is not valid"}"#,
    );
    let err = c.forecast("or/mt-hood-meadows").await.unwrap_err();
    let text = format!("{err} {err:?}");
    assert!(!text.contains("ow_leaky_key"), "{text}");
    assert!(text.contains("[redacted]"));
    assert_eq!(err.api_error().unwrap().request_id(), Some("req-1"));
}

#[tokio::test(start_paused = true)]
async fn redirects_stop_after_five() {
    let mock = MockTransport::new();
    let c = build(client(&mock));
    for _ in 0..7 {
        mock.push(302, &[("Location", "/api/v1/locations/or/again")], "");
    }
    let err = c.forecast("or/mt-hood-meadows").await.unwrap_err();
    assert!(
        matches!(&err, Error::Connection(e) if e.kind() == ConnectionErrorKind::Redirect),
        "{err}"
    );
    assert_eq!(mock.requests().len(), 6);
}

#[tokio::test(start_paused = true)]
async fn a_partial_answer_is_an_error_and_no_answer_stays_empty() {
    let mock = MockTransport::new();
    let c = build(client(&mock));
    // The metric body has no probabilities: nothing is invented.
    mock.push_json(FORECAST);
    let f = c
        .forecast("or/mt-hood-meadows")
        .chance(Chance::at_least(
            Variable::Snowfall,
            Threshold::millimeters(10.0),
        ))
        .await
        .unwrap();
    assert!(f.probabilities.is_empty());

    let mut body: serde_json::Value = serde_json::from_str(FORECAST).unwrap();
    body["probabilities"] = serde_json::json!([{
        "term": "snowfall:ge:10.00:24h",
        "variable": "snowfall", "op": "ge", "threshold": 10.0, "window_start_hours": 0, "window_end_hours": 24,
        "aggregation": "total", "from": null, "to": null, "probability": 0.5, "members": 131,
        "null_reason": null
    }]);
    mock.push_json(body.to_string().as_str());
    let err = c
        .forecast("or/mt-hood-meadows")
        .chance(Chance::at_least(
            Variable::Snowfall,
            Threshold::millimeters(10.0),
        ))
        .chance(Chance::at_least(
            Variable::Snowfall,
            Threshold::millimeters(20.0),
        ))
        .await
        .unwrap_err();
    assert_eq!(err.category(), Category::Unavailable, "{err}");

    // Answers are matched by their echoed term, never by position.
    body["probabilities"] = serde_json::json!([
        {"term": "snowfall:ge:20.00:24h", "variable": "snowfall", "op": "ge", "threshold": 20.0,
         "window_start_hours": 0, "window_end_hours": 24, "aggregation": "total", "from": null, "to": null,
         "probability": null, "members": 0, "null_reason": "window_not_covered"},
        {"term": "snowfall:ge:10.00:24h", "variable": "snowfall", "op": "ge", "threshold": 10.0,
         "window_start_hours": 0, "window_end_hours": 24, "aggregation": "total", "from": null, "to": null,
         "probability": 0.5, "members": 131, "null_reason": null}
    ]);
    mock.push_json(body.to_string().as_str());
    let f = c
        .forecast("or/mt-hood-meadows")
        .chance(Chance::at_least(
            Variable::Snowfall,
            Threshold::millimeters(10.0),
        ))
        .chance(Chance::at_least(
            Variable::Snowfall,
            Threshold::millimeters(20.0),
        ))
        .await
        .unwrap();
    assert_eq!(f.probabilities[0].probability, Some(0.5));
    assert_eq!(f.probabilities[1].probability, None);
    assert_eq!(
        f.probabilities[1].null_reason,
        Some(NullReason::WindowNotCovered)
    );
}

#[tokio::test(start_paused = true)]
async fn max_age_serves_from_the_cache_and_304_without_a_copy_is_an_error() {
    let mock = MockTransport::new();
    let c = build(client(&mock));
    mock.push(
        200,
        &[("ETag", "\"a\""), ("Cache-Control", "max-age=60")],
        SEARCH,
    );
    assert_eq!(c.search("hood").await.unwrap().count, 1);
    let again = c.search("hood").await.unwrap();
    assert_eq!(again.count, 1);
    assert_eq!(
        mock.requests().len(),
        1,
        "a fresh entry is served without a request"
    );

    let mock = MockTransport::new();
    let c = build(client(&mock).no_cache());
    mock.push(304, &[], "");
    assert_eq!(
        c.search("hood").await.unwrap_err().category(),
        Category::Unavailable
    );
}

#[tokio::test(start_paused = true)]
async fn a_failed_page_ends_the_stream() {
    let mock = MockTransport::new();
    let c = build(client(&mock).max_retries(0));
    mock.push_json(PAGE_1);
    mock.push(500, &[], "boom");
    let mut items = c.catalog_locations().items();
    assert!(items.try_next().await.unwrap().is_some());
    assert!(items.try_next().await.is_err());
    assert!(items.try_next().await.unwrap().is_none());
}

#[tokio::test(start_paused = true)]
async fn rate_limit_state_and_request_id_are_exposed() {
    let mock = MockTransport::new();
    let c = build(client(&mock).api_key("ow_k"));
    mock.push(
        200,
        &[
            ("RateLimit-Limit", "600"),
            ("RateLimit-Remaining", "12"),
            ("RateLimit-Reset", "30"),
            ("X-Request-Id", "r9"),
        ],
        FORECAST,
    );
    let f = c.forecast("or/mt-hood-meadows").await.unwrap();
    let rl = f.meta.rate_limit.clone().unwrap();
    assert_eq!(
        (rl.limit, rl.remaining, rl.reset, rl.policy),
        (Some(600), Some(12), Some(30), None)
    );
    assert_eq!(f.meta.request_id.as_deref(), Some("r9"));
    assert_eq!(f.meta.headers.get("ratelimit-limit"), Some("600"));
}

#[tokio::test(start_paused = true)]
async fn the_attempt_timeout_is_a_retryable_connection_error() {
    #[derive(Debug)]
    struct Hang;
    impl crate::transport::Transport for Hang {
        fn send(&self, _: crate::transport::HttpRequest) -> crate::transport::SendFuture<'_> {
            Box::pin(std::future::pending())
        }
    }
    let c = Client::builder()
        .transport(Arc::new(Hang))
        .timeout(Duration::from_secs(5))
        .max_retries(1)
        .jitter(|| 0.0)
        .build_with_env(&|_| None)
        .unwrap();
    let start = tokio::time::Instant::now();
    let err = c.forecast("or/mt-hood-meadows").await.unwrap_err();
    assert!(
        matches!(&err, Error::Connection(e) if e.kind() == ConnectionErrorKind::Timeout),
        "{err}"
    );
    assert_eq!(start.elapsed(), Duration::from_secs(10));
}

/// The real transport against a local socket: a body over 64 MiB is refused,
/// never buffered.
#[tokio::test]
async fn oversized_bodies_are_an_error() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let (mut sock, _) = listener.accept().await.unwrap();
        let mut buf = [0u8; 4096];
        let _ = sock.read(&mut buf).await;
        let _ = sock
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 70000000\r\n\r\n{")
            .await;
    });
    let c = Client::builder()
        .base_url(format!("http://{addr}"))
        .max_retries(0)
        .build_with_env(&|_| None)
        .unwrap();
    let err = c.forecast("or/mt-hood-meadows").await.unwrap_err();
    assert!(
        matches!(&err, Error::Connection(e) if e.kind() == ConnectionErrorKind::BodyTooLarge),
        "{err}"
    );
}
