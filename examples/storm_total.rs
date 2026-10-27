//! Storm total: the distribution of each ensemble member's own total
//! precipitation and snowfall over a window. The SDK never sums hourly
//! values; the server computes these from every member.

use ensemble_weather::{Client, Total, Variable, Window};

#[tokio::main]
async fn main() -> Result<(), ensemble_weather::Error> {
    let client = Client::new()?;

    let forecast = client
        .forecast("or/mt-hood-meadows")
        .max_hours(72)
        .totals([
            Total::new(Variable::Precipitation).window(Window::hours(72)),
            Total::new(Variable::Snowfall).window(Window::hours(72)),
            Total::new(Variable::Snowfall).window(Window::between(24, 48)),
        ])
        .await?;

    for total in &forecast.window_totals {
        let unit = total.unit.map_or("", |u| u.symbol());
        let window = format!("{}-{} h", total.window_start_hours, total.window_end_hours);
        match &total.quantiles {
            Some(q) => {
                let median = q.median().map_or("?".to_owned(), |m| format!("{m:.1}"));
                let range = q.range80().map_or("?".to_owned(), |r| format!("{r:.1}"));
                println!(
                    "{} {window}: median {median} {unit}, 80% range {range} {unit} ({} members)",
                    total.variable, total.members
                );
            }
            None => println!("{} {window}: not covered by this forecast", total.variable),
        }
    }
    Ok(())
}
