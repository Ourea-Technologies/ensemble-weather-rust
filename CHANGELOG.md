# Changelog

All notable changes to this SDK are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses
[Semantic Versioning](https://semver.org/spec/v2.0.0.html). The SDK version is
independent of the API version; every 0.x release targets API contract v1.

Outside contributors are credited by GitHub handle on the line of their
change ("Thanks @handle"). Thank you, all of you.

## [0.1.0] - unreleased

The first release on crates.io. Targets the v1 contract at
ensemble-weather-spec 7954268 (the server's public spec at 256cd770, byte
for byte), and passes all 58 shared vectors.

### Added since the v1 contract of 2026-10-27

- `Client::explore`: places ranked by a metric (`ExploreMetric`: `Snow5d`,
  `BaseDepth`, `SeasonPrecip`, ...), with `order` (`Order`), `kind`,
  `state`, `near`, `radius_km`, `elevation_min` / `elevation_max`, `points`
  (`ExplorePoints`), `limit`, `cursor` and `units`; awaiting it gives an
  `ExplorePage`, and `.items()` streams every `ExploreItem`.
- `Client::season` (observed season-to-date precipitation),
  `Client::snow_trend` (each recent run's daily snowfall; Plus or Pro),
  `Client::snowpack` and `Client::snowpack_plume` (beta),
  `Client::composition` (an open route, sent without the key), and the
  GeoJSON routes `Client::alert_outlines` (`classes`, `bbox`) and
  `Client::avalanche_zones`, with their models (`Season`, `SnowTrend`,
  `Snowpack`, `SnowpackPlume`, `Composition`, `AlertOutlines`,
  `AvalancheZones` and the types they hold).
- `SearchRequest::sort`, `order`, `near`, `radius_km` and `units`; a hit
  carries `explore` (`ExploreValue`) with a metric sort.
- `ForecastRequest::members` takes `Members::Plume` (the plume view, in
  `ForecastData::plume`: `MemberPlume`) as well as a `bool`;
  `MembersView::Pool`.
- `RunsRequest::cursor` and `RunsRequest::items()`: the runs page like every
  other list.
- New fields: `Forecast::avalanche` (`AvalancheForecast`) and
  `snow_on_ground` (`SnowOnGround`); `snow_level_wet_bulb` on
  `EnsembleStats`, `ModelMean`, `MemberForecast` and `DailyForecast`;
  `TimeSeriesPoint::measured`, `snow_character`, `members_spread`,
  `members_6h` and `precipitation_by_type`; `DailyForecast::covered_from`,
  `measured_precipitation`, `snow_character`, `slr`, `snow_level_range`,
  `snow_level_wet_bulb_range`, `normal`, `departure` and `percentile`;
  `precipitation_outlook` and `plume` on `ForecastData` and
  `ElevationForecast`; `ElevationForecast::display_name`,
  `nearest_sensors` and `snow_on_ground`; `next_72h` on `SnowOutlook` and
  `SnowOutlookQuantiles`; `WindowTotal::method`;
  `CurrentConditions::measured` and `cloud_cover`; `CurrentStation::calm`;
  `Current::summary` (`PlaceSummary`) and `pass` (`PassForecast`);
  `SensorObservation::distance_km` and `elevation_diff`; ten NWS fields on
  `ActiveAlert` (`sent`, `message_type`, `references`, `sender_name`,
  `area_desc`, `description`, `instruction`, `source_url`, `event_key`,
  `vtec_action`); `PrecipDisplay::snow_amount`;
  `ModelEntry::display_members` and `display_members_snowfall`;
  `CatalogElevation::display_name`; `CatalogStationDetail::indexable`.
- Enum values: `Condition::LightSnow` / `HeavySnow`, the four new snow
  `ConditionIcon`s, `SourceKind::AvalancheForecast`,
  `StationNetwork::RainGauge` / `Surfrad`; new enums `SnowCharacter`,
  `AlertClass`, `PassStatusLevel`, `ExploreMetric`, `Order` and
  `ExplorePoints`. `ErrorCode::PlusRequired` is a permission error.
- `examples/snow_report.rs`; the live contract test covers the new routes.

### Changed

- JSON fields that repeat a word of their own type have SDK names
  (BEHAVIOR.md 3.0, one table for every Ourea SDK). The models decode the
  JSON names and serialise the SDK names (which they also decode); each
  renamed field's docs start with its JSON name.
  - `Forecast::forecast` is `series` and `elevation_forecasts` is
    `elevations`.
  - `Current::current` is `now`; `CurrentConditions::condition`,
    `condition_text` and `condition_icon` are `code`, `text` and `icon`;
    `CurrentStation::station_id` is `id`.
  - `ProbabilityResult::probability` is `value`;
    `ProbabilityThresholds::daily_*_probability` drop `_probability`;
    `WindowTotal::window_start_hours` / `window_end_hours` are
    `start_hours` / `end_hours`.
  - The list responses' rows are `items` and their counts `count`
    (`Alerts::alerts` and `alert_count`, `LocationsPage::locations`,
    `Runs::runs`, both nearby routes' `locations`); `NearbyLocations::location`
    is `origin`.
  - `ModelMembers::members` is `items`; `MemberForecast::member_id` is `id`.
  - An elevation's `elevation` is `height` (`ElevationForecast`,
    `ElevationSummary`, `SnowTrendElevation`); `CatalogElevation::elevation_m`
    is `height_m`.
  - The catalog's `source_station_id`, `station_status` and `station_url`
    are `source_id`, `status` and `url`; `CatalogTemplate::template_variant`
    is `variant`; `CatalogContentEntry::resolved_locale` is `locale`.
  - `SensorObservation::observation_time` is `observed_at`.
  - The README's "Names that differ from the API's JSON" has the whole
    table.

  GeoJSON bodies are not renamed: `AvalancheZoneFeature::properties` is an
  `AvalancheZoneProperties` with `forecast_url`.
- No name stutters (BEHAVIOR.md 3.0): the `models` module is private, so
  every type is read from the crate root (`ensemble_weather::ModelEntry`,
  not `ensemble_weather::models::ModelEntry`). The astronomical `Season`
  enum is `AstronomicalSeason`, and `Season` is the season-to-date
  response.
- `Forecast::location` is a `ForecastLocation` (the `LocationRef` fields plus
  `elevation` and `elevations`). `NamedElevation` is `ElevationSummary`, the
  spec's name, with `display_name`, and its `default` is an `Option<bool>`.
  `MembersCumulative` holds `MemberQuantiles`.
- Comma lists keep literal commas (BEHAVIOR.md 3.1 rule 5): each item is
  percent-encoded and the commas between items are sent as they are
  (`network=raws,snotel`, not `raws%2Csnotel`). `sources`,
  `exclude_sources`, `elevations`, `diagnostics`, `feels_like` and the
  catalog `include` are sorted and de-duplicated too, like `network`,
  `capability` and the forecast `include` before. `near` and `bbox` are
  built from numbers, with literal commas.
- Search sends the client's units (when not metric) and lowercases a
  two-letter `state`; `getAllLocations` omits `limit=500`, its default.

### Fixed

- `SearchHit::key` is documented as the API sends it (`or-mt-hood-meadows`,
  a dash), and the live contract test checks that form.

### Changes from the draft of 2026-10-26 (never published)

The draft of 2026-10-26 was never published. These changes moved it to the
v1 contract as deployed (ensemble-weather-spec 8b85fea).

#### Changed

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

#### Added

- `ActiveAlert::certainty`, `CurrentStation::station_id`,
  `Runs::next_cursor`, `ForecastRequest::include` (`mslp`,
  `skin_temperature`), `NullReason`, and on `ApiError` `doc_url()`,
  `param()` and `nearest_location()` (`NearestLocation`); `request_id()` is
  read from the body's `request_id` first.
- The drift test also fails on any model field the spec does not have.

#### Removed

- `storm_reports`, `storm_report` and their models (not in the public spec).
- Catalog ads (`CatalogAdSlot`, its placements, `AdSlotKind`, `AdProvider`;
  `include("ads")`).
- `EnsembleStats::feels_like_utci`, `ptype` and `wind_direction_circular`;
  `DailyForecast::feels_like_utci_high` / `low`;
  `ElevationForecast::default` and `timeseries` (`ElevationHour`).

#### The draft's first list

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

[0.1.0]: https://github.com/Ourea-Technologies/ensemble-weather-rust/releases/tag/v0.1.0
