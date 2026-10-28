//! Runs every program in `examples/` against the local stub API (with
//! `OUREA_BASE_URL` pointing at it), checks its output, and drives the
//! blocking client through the same stub. Nothing here touches the network.
//!
//! `cargo test` builds the examples before this test runs; a missing example
//! binary is a failure, not a skip.

mod support;

use std::path::PathBuf;
use std::process::Command;

fn example_binary(name: &str) -> PathBuf {
    // target/<profile>/deps/examples-<hash> -> target/<profile>/examples/<name>
    let exe = std::env::current_exe().unwrap();
    let dir = exe.parent().unwrap().parent().unwrap().join("examples");
    let path = dir.join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
    assert!(
        path.exists(),
        "example binary {} is missing; run `cargo test` (which builds examples)",
        path.display()
    );
    path
}

fn run(name: &str, base_url: &str, api_key: Option<&str>) -> String {
    let mut cmd = Command::new(example_binary(name));
    cmd.env("OUREA_BASE_URL", base_url)
        .env_remove("OUREA_API_KEY");
    if let Some(key) = api_key {
        cmd.env("OUREA_API_KEY", key);
    }
    let out = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "{name} failed ({}):\n{stdout}\n{stderr}",
        out.status
    );
    println!("--- {name}\n{stdout}");
    stdout
}

#[tokio::test(flavor = "multi_thread")]
async fn every_example_runs_against_the_stub() {
    let base = support::start().await;
    let cases: &[(&str, Option<&str>, &[&str])] = &[
        (
            "quickstart",
            None,
            &[
                "2026-11-15T00:00:00Z  -4.0–0.0 °C",
                "Chance of >= 152.4 mm of snow in 0-24 h: 42% (131 members)",
            ],
        ),
        ("point_forecast", None, &["needs a Pro API key"]),
        (
            "point_forecast",
            Some("ow_stub_pro"),
            &[
                "Served from 45.340, -121.710 at 2600 (request)",
                "median temperature",
            ],
        ),
        (
            "chance_of_snow",
            None,
            &[
                "snowfall ge 152.4 mm (total) over 0-24 h:",
                "temperature le 0 °C (all) over 6-18 h",
            ],
        ),
        (
            "storm_total",
            None,
            &["precipitation 0-72 h: median 29.0 mm, 80% range 12.0–52.0 mm (131 members)"],
        ),
        (
            "search_and_catalog",
            None,
            &[
                "or/mt-hood-meadows  Mt. Hood Meadows (Hood River County)",
                "  2. Timberline",
            ],
        ),
        (
            "current_and_alerts",
            None,
            &["Snow", "1 alert(s)", "Winter Storm Warning [severe]"],
        ),
        (
            "error_handling",
            None,
            &[
                "coordinate forecasts need a Pro key",
                "no rate-limit headers",
            ],
        ),
        (
            "error_handling",
            Some("ow_stub_pro"),
            &[
                "outside coverage:",
                "status 404, request stub-request; see https://docs.oureawx.com/features/errors/#outside_coverage",
                "rate limit: 599 of 600 left, resets in 60 s (policy 600;w=60)",
            ],
        ),
        ("units", None, &["metric:", "°C", "imperial:", "°F"]),
    ];
    let mut handles = Vec::new();
    for &(name, key, wants) in cases {
        let base = base.clone();
        handles.push(tokio::task::spawn_blocking(move || {
            let out = run(name, &base, key);
            for want in wants {
                assert!(out.contains(want), "{name} output lacks {want:?}:\n{out}");
            }
        }));
    }
    let mut failures = Vec::new();
    for h in handles {
        if let Err(e) = h.await {
            failures.push(e.to_string());
        }
    }
    assert!(
        failures.is_empty(),
        "{} example runs failed:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[cfg(feature = "blocking")]
#[test]
fn the_blocking_client_works_end_to_end() {
    use ensemble_weather::{Chance, Threshold, Variable, blocking};

    let base = support::start_in_background();
    let client = blocking::Client::builder()
        .base_url(&base)
        .anonymous()
        .build()
        .unwrap();
    let f = client
        .forecast("or/mt-hood-meadows")
        .chance(Chance::at_least(Variable::Snowfall, Threshold::inches(6.0)))
        .send()
        .unwrap();
    assert_eq!(f.probabilities[0].probability, Some(0.42));
    let again = client.forecast("or/mt-hood-meadows").send().unwrap();
    assert!(!again.meta.revalidated); // a different URL: no prob=
    let again = client.forecast("or/mt-hood-meadows").send().unwrap();
    assert!(
        again.meta.revalidated,
        "the second identical call revalidates with a 304"
    );
    let slugs: Vec<String> = client
        .catalog_locations()
        .items()
        .map(|r| r.map(|l| l.location.slug.unwrap_or_default()))
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(slugs, ["mt-hood-meadows", "timberline-lodge"]);
    let err = client
        .forecast_at(ensemble_weather::Point::new(45.0, -121.0))
        .send()
        .unwrap_err();
    assert_eq!(err.category(), ensemble_weather::Category::Permission);
}

/// For trying examples by hand: `cargo test --test examples -- --ignored
/// --nocapture serve_stub`, then `OUREA_BASE_URL=<printed URL> cargo run
/// --example quickstart` in another terminal. Serves for ten minutes.
#[tokio::test]
#[ignore = "a manual helper, not a test"]
async fn serve_stub() {
    let base = support::start().await;
    println!("stub API listening at {base}");
    tokio::time::sleep(std::time::Duration::from_secs(600)).await;
}

/// The README's quickstart is `examples/quickstart.rs` without its header
/// comment, so the README can never rot.
#[test]
fn the_readme_quickstart_is_the_example() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let readme = std::fs::read_to_string(root.join("README.md")).unwrap();
    let example = std::fs::read_to_string(root.join("examples/quickstart.rs")).unwrap();
    let example: String = example
        .lines()
        .skip_while(|l| l.starts_with("//!"))
        .skip_while(|l| l.is_empty())
        .fold(String::new(), |mut out, l| {
            out.push_str(l);
            out.push('\n');
            out
        });
    let section = readme
        .split("## Quickstart")
        .nth(1)
        .expect("a Quickstart section");
    let code = section
        .split("```rust\n")
        .nth(1)
        .and_then(|s| s.split("```").next())
        .expect("a rust block");
    assert_eq!(
        code, example,
        "README quickstart and examples/quickstart.rs differ"
    );
}
