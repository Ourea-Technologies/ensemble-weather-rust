# Changelog

All notable changes to this SDK are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses
[Semantic Versioning](https://semver.org/spec/v2.0.0.html). The SDK version is
independent of the API version; every 0.x release targets API contract v1.

Outside contributors are credited by GitHub handle on the line of their
change ("Thanks @handle"). Thank you, all of you.

## [Unreleased]

Moves to the v1 contract as deployed (ensemble-weather-spec 8b85fea, the
server's public spec byte for byte). Breaking, as 0.x allows.

### Changed

- The coordinate methods follow the operation ids: `forecast_at` is
  `get_point_forecast`, `current_at` is `get_point_current`, `alerts_at` is
  `get_point_alerts` and `nearby_at` is `get_point_nearby` (on the async and
  blocking clients). There are no aliases.
- `precip_*` fields are `precipitation_*`: `DailyForecast::total_precipitation`,
  `total_precipitation_if_wet`, `precipitation_probability`,
  `precipitation_occurrence_probability`, `precipitation_type`,
  `precipitation_display`; `TimeSeriesPoint::precipitation_type`;
  `CurrentConditions::precipitation_probability`;
  `SensorObservation::precipitation_rate`; and
  `ProbabilityThresholds::daily_precipitation_probability` /
  `daily_precipitation_occurrence_probability`.
- `EnsembleStats::wind_direction` and `ModelMean::wind_direction` are
  `CircularStats` (mean, stddev, 50 % and 90 % sectors), not `Quantiles`.
- Probabilities and window totals are matched to the caller's terms by the
  `term` the server echoes, never by position. `ProbabilityResult` and
  `WindowTotal` carry `term` and `null_reason`; `WindowTotal` carries
  `aggregation` (new `Aggregation::Min` / `Max`, refused in a `Chance`).
- `ErrorCode` is the public spec's 16 codes: `AuthUnavailable`
  (unavailable) is new; `Forbidden`, `ModelRunNotFound`, `Gone`,
  `InvalidMaxHours`, `Ambiguous` and `FeelsLikeUnavailable` are gone (an old
  code still parses to `Unknown` and maps by status). `OUTSIDE_COVERAGE` is
  a 404. `Retry-After` is honoured on 504 as well as 429 and 503.
- Typed snake_case enums: `ActiveAlert::severity` / `urgency`
  (`AlertSeverity`, `AlertUrgency`), the catalog's `StationNetwork`,
  `StationStatus` and `TemplateVariant`, and `condition_icon`
  (`ConditionIcon`). The catalog filters take them: `kind`, `template`,
  `networks`, `status`. `network` and `capability` are sent as one sorted,
  de-duplicated comma list (the server's canonical form); `capability` is
  no longer repeated.
- `LocationRef::slug` and `kind` are `Option`: `None` for a coordinate
  forecast that matched no named location. `LocationRef::key()` returns
  `Option<String>`.

### Added

- `ActiveAlert::certainty`, `CurrentStation::station_id`,
  `Runs::next_cursor`, `ForecastRequest::include` (`mslp`,
  `skin_temperature`), `NullReason`, and on `ApiError` `doc_url()`,
  `param()` and `nearest_location()` (`NearestLocation`); `request_id()` is
  read from the body's `request_id` first.
- The drift test also fails on any model field the spec does not have.

### Removed

- `storm_reports`, `storm_report` and their models (not in the public spec).
- Catalog ads (`CatalogAdSlot`, its placements, `AdSlotKind`, `AdProvider`;
  `include("ads")`).
- `EnsembleStats::feels_like_utci`, `ptype` and `wind_direction_circular`;
  `DailyForecast::feels_like_utci_high` / `low`;
  `ElevationForecast::default` and `timeseries` (`ElevationHour`).

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
