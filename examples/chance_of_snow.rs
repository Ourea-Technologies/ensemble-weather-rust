//! Chance of snow: several probability terms in one call. Each answer comes
//! back in the order asked, with its window and member count; a window the
//! forecast does not reach is absent, never 0.

use ensemble_weather::{Aggregation, Chance, Client, Threshold, Variable, Window};

#[tokio::main]
async fn main() -> Result<(), ensemble_weather::Error> {
    let client = Client::new()?;

    let forecast = client
        .forecast("or/mt-hood-meadows")
        .max_hours(240)
        .chances([
            Chance::at_least(Variable::Snowfall, Threshold::inches(6.0)).window(Window::hours(24)),
            Chance::at_least(Variable::Snowfall, Threshold::inches(12.0)).window(Window::hours(72)),
            Chance::at_least(Variable::Snowfall, Threshold::inches(24.0))
                .window(Window::hours(240)),
            Chance::at_most(Variable::Temperature, Threshold::fahrenheit(32.0))
                .window(Window::between(6, 18))
                .aggregation(Aggregation::All),
        ])
        .await?;

    for c in &forecast.probabilities {
        let unit = c.threshold_unit.map_or("", |u| u.symbol());
        let window = format!("{}-{} h", c.window_start_hours, c.window_end_hours);
        match c.probability {
            Some(p) => println!(
                "{} {} {} {unit} ({}) over {window}: {:.0}% of {} members",
                c.variable,
                c.op,
                c.threshold,
                c.aggregation,
                p * 100.0,
                c.members
            ),
            None => println!("{} over {window}: not covered by this forecast", c.variable),
        }
    }
    Ok(())
}
