# Changelog

All notable changes to this SDK are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses
[Semantic Versioning](https://semver.org/spec/v2.0.0.html). The SDK version is
independent of the API version; every 0.x release targets API contract v1.

Outside contributors are credited by GitHub handle on the line of their
change ("Thanks @handle"). Thank you, all of you.

## [Unreleased]

## [0.1.0] - 2026-10-26

### Added

- `Client` (async, Tokio) with a builder: API key (or `OUREA_API_KEY`),
  base URL (or `OUREA_BASE_URL`), units, timeout, retries, cache, keyless
  open routes, User-Agent product token and a deprecation handler.
- A `blocking` feature with `blocking::Client`, the same calls without async.
- Every v1 operation except `getMapsToken`, as request builders you
  `.await`: `forecast`, `forecast_at`, `current`, `current_at`, `alerts`,
  `alerts_at`, `nearby`, `nearby_at`, `search`, `locations`,
  `catalog_locations`, `catalog_location`, `catalog_stations`,
  `catalog_station`, `runs`, `storm_reports`, `storm_report` and
  `attribution`, with hand-written serde models for every schema they reach.
  Unknown enum values parse to `Unknown(String)`.
- Server-computed probabilities (`Chance`, `prob=`) and window totals
  (`Total`, `totals=`), answered in the caller's order; threshold unit
  conversion with the API's exact factors and rounding.
- Canonical URLs, keyless open routes, same-origin redirects (the key is
  dropped elsewhere), retries with full-jitter backoff and `Retry-After`, an
  in-memory ETag cache behind a `Cache` trait, rate-limit state on every
  result, and an `Error` enum with one variant per category.
- `Items`, a `Stream` (and a blocking `Iterator`) that follows the cursor.
- The shared test vectors (43/43), spec-drift tests and runnable examples.

[Unreleased]: https://github.com/Ourea-Technologies/ensemble-weather-rust/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/Ourea-Technologies/ensemble-weather-rust/releases/tag/v0.1.0
