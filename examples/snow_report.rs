//! A snow report: the snowiest places of the next five days, a location's
//! season to date, and the map layers (alert outlines and avalanche zones).

use ensemble_weather::{AlertClass, Client, ExploreMetric};

#[tokio::main]
async fn main() -> Result<(), ensemble_weather::Error> {
    let client = Client::new()?;

    // Places ranked by forecast snowfall over five days, the most first.
    let page = client.explore(ExploreMetric::Snow5d).limit(5).await?;
    for item in &page.items {
        println!(
            "{:>2}. {} ({}): {} {}",
            item.rank, item.location.name, item.location.state, item.value, page.unit
        );
    }

    // Observed precipitation since the water year began.
    let season = client.season("or/mt-hood-meadows").await?;
    match &season.precipitation {
        Some(total) => println!(
            "water year {} at {}: {} {} since {}, {}% of normal",
            season.water_year,
            season.location.name,
            total.amount,
            total.unit,
            season.starts,
            total
                .percent_of_normal
                .map_or_else(|| "?".to_owned(), |p| p.to_string())
        ),
        None => println!("no observed days yet this water year"),
    }

    // The map layers are GeoJSON, passed through as the API sends them.
    let outlines = client
        .alert_outlines()
        .classes([AlertClass::Winter, AlertClass::Wind])
        .await?;
    for feature in &outlines.features {
        println!(
            "alert: {} ({})",
            feature.properties.event, feature.properties.class
        );
    }
    let zones = client.avalanche_zones().await?;
    for zone in &zones.features {
        let rating = zone
            .properties
            .danger_label
            .as_deref()
            .unwrap_or("no rating");
        println!("avalanche zone {}: {rating}", zone.properties.zone_name);
    }

    let composition = client.composition().await?;
    println!(
        "the blend: {} members from {} models",
        composition.total_members, composition.model_count
    );
    Ok(())
}
