//! Units: the same forecast in metric and imperial. The server converts every
//! value; the SDK never converts response values itself.

use ensemble_weather::{Client, Units, Variable};

#[tokio::main]
async fn main() -> Result<(), ensemble_weather::Error> {
    let client = Client::new()?;

    for units in [Units::Metric, Units::Imperial] {
        let forecast = client
            .forecast("or/mt-hood-meadows")
            .units(units.clone())
            .await?;
        let temp = forecast
            .units
            .unit_for(&Variable::Temperature)
            .map_or("", |u| u.symbol());
        let snow = forecast
            .units
            .unit_for(&Variable::Snowfall)
            .map_or("", |u| u.symbol());
        let Some(hour) = forecast.forecast.timeseries.first() else {
            continue;
        };
        let t = hour
            .ensemble
            .temperature
            .median()
            .map_or("?".to_owned(), |v| format!("{v:.1}"));
        let s = hour
            .ensemble
            .snowfall
            .as_ref()
            .and_then(ensemble_weather::Quantiles::median)
            .map_or("?".to_owned(), |v| format!("{v:.2}"));
        println!(
            "{units}: {} median temperature {t} {temp}, median snowfall {s} {snow}",
            hour.valid_time
        );
    }
    Ok(())
}
