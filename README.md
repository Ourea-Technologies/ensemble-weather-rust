# ensemble-weather-rust

Official Rust SDK for the [Ourea](https://oureawx.com) probabilistic weather API.

[![CI](https://github.com/Ourea-Technologies/ensemble-weather-rust/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/Ourea-Technologies/ensemble-weather-rust/actions/workflows/ci.yml?query=branch%3Amain)
[![Security](https://github.com/Ourea-Technologies/ensemble-weather-rust/actions/workflows/security.yml/badge.svg?branch=main)](https://github.com/Ourea-Technologies/ensemble-weather-rust/actions/workflows/security.yml?query=branch%3Amain)
[![crates.io](https://img.shields.io/crates/v/ensemble-weather.svg)](https://crates.io/crates/ensemble-weather)
[![MSRV 1.96](https://img.shields.io/crates/msrv/ensemble-weather.svg?label=rust)](Cargo.toml)
[![Licence: BSD-2-Clause](https://img.shields.io/badge/licence-BSD--2--Clause-blue.svg)](LICENSE)
[![Docs](https://img.shields.io/badge/docs-docs.oureawx.com-informational)](https://docs.oureawx.com/rust)
[![OpenSSF Scorecard](https://api.scorecard.dev/projects/github.com/Ourea-Technologies/ensemble-weather-rust/badge)](https://scorecard.dev/viewer/?uri=github.com/Ourea-Technologies/ensemble-weather-rust)

## Install

```sh
cargo add ensemble-weather tokio --features tokio/macros,tokio/rt-multi-thread
```

(The SDK is async on Tokio. For a synchronous client, add
`--features ensemble-weather/blocking` and see [Blocking](#blocking).)

## Quickstart

No signup and no key: an anonymous forecast for Mt. Hood Meadows, printing
the next 24 hours' temperature as a range (p10 to p90) and the chance of at
least 6 in (15 cm) of snow in the next 24 hours. Put it in `src/main.rs` and
`cargo run`:

```rust
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
```

This is [`examples/quickstart.rs`](examples/quickstart.rs) without its header
comment; CI runs it on every change.

## Authentication

Pass a key to the builder, or set `OUREA_API_KEY` and use `Client::new()`:

```rust
let client = ensemble_weather::Client::builder().api_key("ow_...").build()?;
```

| | Anonymous / Free key | Pro key |
|---|---|---|
| Named-location forecasts, current conditions, alerts | yes, 10-day (240 h) horizon | yes, 15-day (360 h) horizon |
| Forecasts and current conditions at coordinates | no | yes |
| Per-model breakdowns, historical runs | no | yes |
| Ensemble members | plume view | every variable |

Plans and limits: https://oureawx.com/pricing. The key is sent only as
`X-API-Key`, only to the configured base URL, and never appears in errors or
`Debug` output. Search, catalog details and attribution are sent without the
key so every caller shares one edge-cache entry;
`.send_key_on_open_routes(true)` sends it there too. `client.with_api_key(..)`
gives a per-customer copy that shares the cache and connection pool.

## Probabilities, not single numbers

Every forecast value is a distribution, `Quantiles`, computed by the server
from the full ensemble (up to five weather models). Ask the server for the
number you need; the SDK never sums, averages or interpolates quantiles
itself, because only the server has every member. (Summing hourly medians,
for instance, does not give the median storm total.) More:
https://docs.oureawx.com/concepts/probabilities.

**A chance**, with its reference class (variable, threshold, window, members):

```rust
let f = client
    .forecast("or/mt-hood-meadows")
    .chance(Chance::at_least(Variable::Snowfall, Threshold::inches(12.0)).window(Window::hours(72)))
    .chance(
        Chance::at_most(Variable::Temperature, Threshold::fahrenheit(32.0))
            .window(Window::between(6, 18))
            .aggregation(Aggregation::All),
    )
    .await?;
// f.probabilities[i] answers the i-th chance (matched by the term the server
// echoes, never by position); `probability` is None (never 0) when the server
// cannot answer, with the reason in `null_reason`.
```

**A window total**, the distribution of each member's own total:

```rust
let f = client
    .forecast("or/mt-hood-meadows")
    .total(Total::new(Variable::Snowfall).window(Window::hours(72)))
    .await?;
if let Some(q) = &f.window_totals[0].quantiles {
    println!("median {:?}, 80% range {:?}", q.median(), q.range80());
}
```

**Quantiles**: the percentiles the server sent, as `Option<f64>` (`None` is
absent), with the served ranges as views:

```rust
let t = &f.forecast.timeseries[0].ensemble.temperature;
let median = t.median(); // p50: the median, not "the forecast"
let r80 = t.range80();   // p10..p90
let r50 = t.range50();   // p25..p75
```

Thresholds may carry any unit (`Threshold::inches`, `::celsius`,
`::miles_per_hour`, ...); the SDK converts your threshold to the request's
units with exact factors and rounds it as the server does. Response values
are never converted: choose the unit system and the server converts.

| Units | Temperature | Precipitation, snowfall | Wind | Heights |
|---|---|---|---|---|
| `Units::Metric` (default) | °C | mm | m/s | m |
| `Units::Imperial` | °F | in | mph | ft |
| `Units::Kelvin` | K | mm | m/s | m |

## More examples

Each is a complete program: `cargo run --example <name>`.

| Example | Shows |
|---|---|
| [quickstart](examples/quickstart.rs) | the program above, anonymous |
| [point_forecast](examples/point_forecast.rs) | a forecast at coordinates with elevation (Pro key from `OUREA_API_KEY`) |
| [chance_of_snow](examples/chance_of_snow.rs) | several `Chance` terms in one call, with windows and member counts |
| [storm_total](examples/storm_total.rs) | `Total` window distributions for precipitation and snowfall |
| [search_and_catalog](examples/search_and_catalog.rs) | search, then page through the catalog with the cursor |
| [current_and_alerts](examples/current_and_alerts.rs) | current conditions and active alerts for a location |
| [error_handling](examples/error_handling.rs) | catching an out-of-coverage error by its code; reading rate-limit state |
| [units](examples/units.rs) | the same forecast in metric and imperial |

List calls also stream every item, following the cursor:

```rust
let mut stations = client.catalog_stations().admin1("US-OR").items(); // a futures_core::Stream
while let Some(station) = stations.try_next().await? {
    println!("{}", station.location.name);
}
```

### Blocking

With the `blocking` feature, `ensemble_weather::blocking::Client` makes the
same calls without async (it runs its own single-threaded Tokio runtime; do
not use it inside an async runtime):

```rust
let client = ensemble_weather::blocking::Client::new()?;
let forecast = client.forecast("or/mt-hood-meadows").send()?;
for location in client.catalog_locations().items() {
    println!("{}", location?.location.name);
}
```

## Errors, retries and rate limits

Every call returns `Result<T, ensemble_weather::Error>`. `Error` has one
variant per category: `Authentication`, `Permission`, `NotFound`,
`InvalidRequest`, `OutsideCoverage`, `RateLimited`, `Unavailable` (each with
an `ApiError`: `status()`, `code()` kept verbatim, `message()`,
`request_id()`, `doc_url()` (the code's page in the API docs), `param()` (the
parameter a 400 is about), `nearest_location()` (on a `PRO_REQUIRED` from a
coordinate route, a nearby location that needs no key), `retry_after()`),
`Connection` (no response) and `Usage`
(refused before sending, such as an invalid `Chance`).

```rust
match client.get_point_forecast(Point::new(45.0, 10.0)).await {
    Err(e) if e.code() == Some(&ErrorCode::OutsideCoverage) => println!("outside the forecast domain"),
    Err(Error::RateLimited(e)) => println!("retry after {:?}", e.retry_after()),
    Err(e) => return Err(e),
    Ok(f) => println!("{}", f.location.name),
}
```

Connection errors (except TLS verification failures), 429, 502, 503 and 504
are retried twice by default (`.max_retries(n)`), with full-jitter backoff; a
429, 503 or 504 `Retry-After` is honoured exactly when it is 60 s or less, and
returned in `retry_after()` when it is longer. Nothing else is retried. Each
attempt times out after 30 s (`.timeout(..)`).

Every result has a `meta: ResponseMeta`: `meta.rate_limit` holds the
response's `RateLimit-*` headers (`None` on edge-cacheable responses, which
carry none), `meta.request_id` the `X-Request-Id`, and `meta.revalidated`
reports a `304` served from the client's ETag cache (256 entries / 32 MiB in
memory by default; `.cache(..)` replaces it with your own `Cache`,
`.no_cache()` turns it off).

## Documentation

- Guides: https://docs.oureawx.com
- This SDK's guide: https://docs.oureawx.com/rust
- API reference for this crate: https://docs.rs/ensemble-weather
- The OpenAPI contract (API v1): [ensemble-weather-spec](https://github.com/Ourea-Technologies/ensemble-weather-spec), pinned here in [`spec/`](spec/)

This SDK targets API contract v1. Its own version (0.x) is independent of the
API version.

## Supported versions

Rust 1.96 (the MSRV, declared as `rust-version`) and the latest stable, both
tested in CI, on Linux, macOS and Windows. Runtime dependencies: reqwest
(rustls only; no OpenSSL), serde, serde_json, thiserror, tokio, futures-core
and url; each is justified in [CONTRIBUTING.md](CONTRIBUTING.md#dependencies).

## Contributing and community

This SDK is open source and we want it to be the best weather SDK in Rust.
Bug reports, feature requests, docs fixes and pull requests are all welcome,
including small ones.

- **Report a bug**: [open a bug report](https://github.com/Ourea-Technologies/ensemble-weather-rust/issues/new?template=bug_report.yml)
- **Request a feature** or an API capability: [open a feature request](https://github.com/Ourea-Technologies/ensemble-weather-rust/issues/new?template=feature_request.yml)
- **Ask a question or share what you built**: [GitHub Discussions](https://github.com/Ourea-Technologies/ensemble-weather-rust/discussions)
- **Contribute code or docs**: start with [CONTRIBUTING.md](CONTRIBUTING.md) and the [`good first issue`](https://github.com/Ourea-Technologies/ensemble-weather-rust/labels/good%20first%20issue) label
- **Security issues**: never in public issues; see [SECURITY.md](SECURITY.md)

Licensed under [BSD-2-Clause](LICENSE).
