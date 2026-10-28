//! The nightly contract run against the live API (`live.yml`). Every test is
//! `#[ignore]`d, so `cargo test` never calls the network; the workflow runs
//! them with `--ignored` and a read-only key in `OUREA_LIVE_API_KEY`.

use ensemble_weather::{Category, Chance, Client, Threshold, Total, Variable, Window};

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
    assert!(!f.forecast.timeseries.is_empty());
}

#[tokio::test]
#[ignore = "calls the live API; run by live.yml"]
async fn keyed_calls_and_open_routes() {
    let client = Client::builder().api_key(key()).build().unwrap();
    let hits = client.search("hood meadows").await.unwrap();
    assert!(hits.items.iter().any(|h| h.key == "or/mt-hood-meadows"));
    let current = client.current("or/mt-hood-meadows").await.unwrap();
    assert_eq!(current.location.slug.as_deref(), Some("mt-hood-meadows"));
    let _ = client.alerts("or/mt-hood-meadows").await.unwrap();
    let _ = client.attribution().await.unwrap();
    let _ = client.runs().limit(1).await.unwrap();
    let page = client.catalog_locations().limit(5).await.unwrap();
    assert!(!page.items.is_empty());
    let missing = client
        .forecast("or/no-such-place-anywhere")
        .await
        .unwrap_err();
    assert_eq!(missing.category(), Category::NotFound);
}
