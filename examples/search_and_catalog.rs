//! Search and catalog: find a location by name, then walk the location
//! catalog page by page, following the cursor.

use ensemble_weather::Client;

#[tokio::main]
async fn main() -> Result<(), ensemble_weather::Error> {
    let client = Client::new()?;

    let hits = client.search("hood meadows").limit(5).await?;
    for hit in &hits.items {
        println!("{}  {} ({} County)", hit.key, hit.location.name, hit.county);
    }

    // One page at a time:
    let first = client.catalog_locations().limit(20).await?;
    println!(
        "first page: {} records, more: {}",
        first.items.len(),
        first.next_cursor.is_some()
    );

    // Or every record, the cursor followed for you:
    let mut all = client.catalog_locations().limit(20).items();
    let mut count = 0;
    while let Some(record) = all.try_next().await? {
        count += 1;
        println!(
            "{count:>3}. {} ({})",
            record.location.name,
            record.location.key().unwrap_or_default()
        );
    }
    Ok(())
}
