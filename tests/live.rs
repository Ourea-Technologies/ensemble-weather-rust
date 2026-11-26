//! The nightly contract run against the live API (`live.yml`). Every test is
//! `#[ignore]`d, so `cargo test` never calls the network; the workflow runs
//! them with `--ignored` and a read-only key in `OUREA_LIVE_API_KEY`.

use ensemble_weather::{
    AlertClass, Category, Chance, Client, ExploreMetric, Members, Threshold, Total, Variable,
    Window,
};

fn key() -> String {
    std::env::var("OUREA_LIVE_API_KEY").expect("OUREA_LIVE_API_KEY must hold the read-only CI key")
}

#[tokio::test]
#[ignore = "calls the live API; run by live.yml"]
async fn anonymous_forecast_with_probabilities_and_totals() {
    let client = Client::builder().anonymous().build().unwrap();
    let f = client
        .forecast("or/mt-hood-meadows")
        .chance(Chance::at_least(Variable::Snowfall, Threshold::inches(6.0)))
        .total(Total::new(Variable::Precipitation).window(Window::hours(48)))
        .await
        .unwrap();
    assert_eq!(f.location.slug.as_deref(), Some("mt-hood-meadows"));
    assert_eq!(f.probabilities.len(), 1);
    assert_eq!(f.window_totals.len(), 1);
    assert!(!f.series.timeseries.is_empty(), "no hours served");
}

#[tokio::test]
#[ignore = "calls the live API; run by live.yml"]
async fn keyed_calls_and_open_routes() {
    let client = Client::builder().api_key(key()).build().unwrap();
    let hits = client.search("hood meadows").await.unwrap();
    assert!(hits.items.iter().any(|h| h.key == "or-mt-hood-meadows"));
    let current = client.current("or/mt-hood-meadows").await.unwrap();
    assert_eq!(current.location.slug.as_deref(), Some("mt-hood-meadows"));
    let _ = client.alerts("or/mt-hood-meadows").await.unwrap();
    let _ = client.attribution().await.unwrap();
    let _ = client.runs().limit(1).await.unwrap();
    let page = client.catalog_locations().limit(5).await.unwrap();
    assert!(!page.items.is_empty(), "an empty catalog page");
    let missing = client
        .forecast("or/no-such-place-anywhere")
        .await
        .unwrap_err();
    assert_eq!(missing.category(), Category::NotFound);
}

#[tokio::test]
#[ignore = "calls the live API; run by live.yml"]
async fn the_routes_added_in_0_1_0() {
    let client = Client::builder().api_key(key()).build().unwrap();
    let page = client
        .explore(ExploreMetric::Snow5d)
        .state("or")
        .limit(3)
        .await
        .unwrap();
    assert_eq!(page.metric, ExploreMetric::Snow5d);
    let sorted = client
        .search("hood")
        .sort(ExploreMetric::Snow5d)
        .near(45.33, -121.66)
        .await
        .unwrap();
    assert!(!sorted.items.is_empty(), "no sorted hits");
    let composition = client.composition().await.unwrap();
    assert!(composition.total_members > 0);
    let season = client.season("or/mt-hood-meadows").await.unwrap();
    assert_eq!(season.location.slug.as_deref(), Some("mt-hood-meadows"));
    let snowpack = client.snowpack("or/mt-hood-meadows").await.unwrap();
    assert!(snowpack.beta);
    let plume = client.snowpack_plume("or/mt-hood-meadows").await.unwrap();
    assert!(plume.beta);
    let outlines = client
        .alert_outlines()
        .classes([AlertClass::Winter])
        .await
        .unwrap();
    assert_eq!(outlines.kind, "FeatureCollection");
    let zones = client.avalanche_zones().await.unwrap();
    assert_eq!(zones.kind, "FeatureCollection");
    let plume = client
        .forecast("or/mt-hood-meadows")
        .members(Members::Plume)
        .max_hours(12)
        .await
        .unwrap();
    assert!(plume.series.plume.is_some());
    // The snow trend needs Plus or Pro; the CI key's plan decides which
    // answer is right, and either is a typed result.
    match client.snow_trend("or/mt-hood-meadows").await {
        Ok(trend) => assert!(!trend.dates.is_empty(), "a snow trend with no dates"),
        Err(e) => assert_eq!(e.category(), Category::Permission),
    }
}
