//! Current conditions and alerts: right now at a location, the nearby
//! sensors, and the weather alerts in effect.

use ensemble_weather::Client;

#[tokio::main]
async fn main() -> Result<(), ensemble_weather::Error> {
    let client = Client::new()?;

    let current = client.current("or/mt-hood-meadows").await?;
    if let Some(c) = &current.now {
        println!(
            "{}: {} {}, wind {} from {}° ({})",
            c.time, c.temperature, c.text, c.wind_speed, c.wind_direction, c.source
        );
    } else {
        println!("no current hour is being served");
    }
    if let Some(summary) = &current.summary {
        println!("{}", summary.short);
    }
    for sensor in &current.sensors {
        println!(
            "  sensor {} ({}) at {}",
            sensor.name, sensor.source, sensor.observed_at
        );
    }

    let alerts = client.alerts("or/mt-hood-meadows").await?;
    println!("{} alert(s) as of {}", alerts.count, alerts.as_of);
    for alert in &alerts.items {
        println!(
            "  {} [{}]: {} (expires {})",
            alert.event, alert.severity, alert.headline, alert.expires
        );
    }
    Ok(())
}
