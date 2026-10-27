//! Point forecast: a forecast at coordinates with an elevation, which needs a
//! Pro API key from `OUREA_API_KEY`. Without one it says so and exits.

use ensemble_weather::{Client, Point};

#[tokio::main]
async fn main() -> Result<(), ensemble_weather::Error> {
    let client = Client::new()?;
    if !client.has_api_key() {
        println!("point_forecast needs a Pro API key in OUREA_API_KEY; skipping.");
        return Ok(());
    }

    // Mt. Hood's Palmer snowfield, at its own elevation rather than the grid's.
    let palmer = Point::new(45.3442, -121.7122).elevation_m(2_600.0);
    let forecast = client.forecast_at(palmer).max_hours(48).await?;

    if let Some(point) = &forecast.point {
        println!(
            "Served from {:.3}, {:.3} at {} ({})",
            point.lat, point.lon, point.elevation, point.elevation_source
        );
    }
    for hour in forecast.forecast.timeseries.iter().take(6) {
        if let Some(median) = hour.ensemble.temperature.median() {
            println!("{}: median temperature {median:.1}", hour.valid_time);
        }
    }
    Ok(())
}
