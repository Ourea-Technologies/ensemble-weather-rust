//! Error handling: catch an out-of-coverage error by its code, tell the
//! error categories apart, and read the rate-limit state.

use ensemble_weather::{Client, Error, ErrorCode, Point};

#[tokio::main]
async fn main() -> Result<(), Error> {
    let client = Client::new()?; // OUREA_API_KEY, if set, is used

    // Northern Italy is outside the forecast domain (CONUS).
    match client.forecast_at(Point::new(45.0, 10.0)).await {
        Ok(f) => println!("unexpectedly got a forecast for {}", f.location.name),
        Err(e) if e.code() == Some(&ErrorCode::OutsideCoverage) => {
            let api = e.api_error().expect("an API error");
            println!(
                "outside coverage: {} (status {}, request {}; see {})",
                api.message(),
                api.status(),
                api.request_id().unwrap_or("-"),
                api.doc_url().unwrap_or("-")
            );
        }
        Err(Error::Permission(e)) => {
            println!("coordinate forecasts need a Pro key: {}", e.message());
        }
        Err(Error::RateLimited(e)) => println!("rate limited; retry after {:?}", e.retry_after()),
        Err(e) => return Err(e),
    }

    let forecast = client.forecast("or/mt-hood-meadows").await?;
    match &forecast.meta.rate_limit {
        Some(rl) => {
            let n = |v: Option<u64>| v.map_or_else(|| "?".to_owned(), |v| v.to_string());
            println!(
                "rate limit: {} of {} left, resets in {} s (policy {})",
                n(rl.remaining),
                n(rl.limit),
                n(rl.reset),
                rl.policy.as_deref().unwrap_or("-")
            );
        }
        None => {
            println!("no rate-limit headers: this anonymous response is shared at the edge cache");
        }
    }
    Ok(())
}
