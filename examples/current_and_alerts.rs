//! Current conditions and alerts: right now at a location, the nearby
//! sensors, and the weather alerts in effect.

use ensemble_weather::Client;

#[tokio::main]
async fn main() -> Result<(), ensemble_weather::Error> {
    let client = Client::new()?;

    let now = client.current("or/mt-hood-meadows").await?;
    if let Some(c) = &now.current {
        println!(
            "{}: {} {}, wind {} from {}° ({})",
            c.time, c.temperature, c.condition_text, c.wind_speed, c.wind_direction, c.source
        );
    } else {
        println!("no current hour is being served");
    }
    for sensor in &now.sensors {
        println!(
            "  sensor {} ({}) at {}",
            sensor.name, sensor.source, sensor.observation_time
        );
    }

    let alerts = client.alerts("or/mt-hood-meadows").await?;
    println!("{} alert(s) as of {}", alerts.alert_count, alerts.as_of);
    for alert in &alerts.alerts {
        println!(
            "  {} [{}]: {} (expires {})",
            alert.event, alert.severity, alert.headline, alert.expires
        );
    }
    Ok(())
}
