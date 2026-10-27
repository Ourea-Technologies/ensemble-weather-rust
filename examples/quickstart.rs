//! Quickstart: an anonymous forecast for Mt. Hood Meadows. It prints the next
//! 24 hours' temperature as a range (p10 to p90) and the chance of at least
//! 6 in (15 cm) of snow in the next 24 hours. No API key is needed.

use ensemble_weather::{Chance, Client, Threshold, Variable, Window};

#[tokio::main]
async fn main() -> Result<(), ensemble_weather::Error> {
    let client = Client::new()?;

    let forecast = client
        .forecast("or/mt-hood-meadows")
        .chance(
            Chance::at_least(Variable::Snowfall, Threshold::inches(6.0)).window(Window::hours(24)),
        )
        .await?;

    let unit = forecast
        .units
        .unit_for(&Variable::Temperature)
        .map_or("", |u| u.symbol());
    for hour in forecast.forecast.timeseries.iter().take(24) {
        if let Some(range) = hour.ensemble.temperature.range80() {
            println!("{}  {range:.1} {unit}", hour.valid_time);
        }
    }
    for chance in &forecast.probabilities {
        if let Some(p) = chance.probability {
            let unit = chance.threshold_unit.map_or("", |u| u.symbol());
            println!(
                "Chance of >= {} {unit} of snow in {}-{} h: {:.0}% ({} members)",
                chance.threshold,
                chance.window_start_hours,
                chance.window_end_hours,
                p * 100.0,
                chance.members
            );
        }
    }
    Ok(())
}
