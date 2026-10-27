# Contributing to ensemble-weather-rust

Thanks for helping make Ourea better. Whether you found a bug, have an idea,
fixed a typo or want to write code, you are welcome here, and small
contributions are as valuable as big ones.

This is the official Rust SDK for the [Ourea](https://oureawx.com) API. Its
behaviour is fixed by
[ensemble-weather-spec](https://github.com/Ourea-Technologies/ensemble-weather-spec)
(pinned in [`spec/`](spec/)), and its shape follows the
[Rust API Guidelines](https://rust-lang.github.io/api-guidelines/).

## From clone to green

You need Rust 1.96 or later (`rustup` is the easy way) and nothing else.

```sh
git clone https://github.com/Ourea-Technologies/ensemble-weather-rust
cd ensemble-weather-rust
scripts/check        # every gate CI runs; prints "all gates passed"
```

`scripts/check` needs `cargo-deny` and `cargo-audit` for its last two gates
(`cargo install --locked cargo-deny cargo-audit`); it tells you if they are
missing. No API key and no network access to the API are needed: every test
runs against a scripted transport or a local stub.

## The gates

`scripts/check` runs these in order and stops at the first failure. CI runs
the same commands.

| Gate | Command |
|---|---|
| format | `cargo fmt --check` |
| lint | `cargo clippy --all-features --all-targets -- -D warnings` (`clippy::pedantic` is on in `Cargo.toml`) |
| build, no default features | `cargo build` |
| unit tests, vectors, spec drift, examples | `cargo test --all-features` |
| doc tests | included in `cargo test`; every public type and method has an example |
| docs | `RUSTDOCFLAGS="-D warnings" cargo doc --all-features --no-deps` |
| package | `cargo package` (builds the crate as published) |
| dependencies | `scripts/check-deps` (see below) |
| advisories, licences, bans | `cargo deny check` and `cargo audit` |

- **Vectors.** `src/tests/vectors.rs` runs every case in `spec/vectors`
  through a scripted transport on Tokio's paused clock, with the backoff
  jitter pinned to its upper bound. A case the runner cannot map (an unknown
  operation, argument or expectation) is a failure, never a skip, and the
  count (43) is pinned.
- **Examples.** `tests/examples.rs` runs every `examples/*.rs` binary against
  `tests/support`, a local stand-in for the API, with `OUREA_BASE_URL`
  pointing at it, and checks the output. To try an example by hand, run
  `cargo test --test examples -- --ignored --nocapture serve_stub` and, in
  another terminal, `OUREA_BASE_URL=<the printed URL> cargo run --example quickstart`.
  Never point tests or examples at `api.oureawx.com`.
- **Spec drift.** `src/tests/drift.rs` fails when the spec has an operation
  the SDK does not expose (or the reverse), when a response synthesised with
  every property of every reachable schema loses a field in the models, or
  when a response example does not round-trip.

### Lints

`unsafe_code` is forbidden and `missing_docs` denied (in `Cargo.toml` and
`src/lib.rs`). Clippy runs with `pedantic` as warnings, turned into errors
by `-D warnings`. The documented allows:

- `module_name_repetitions` (crate-wide): `CacheEntry`, `ErrorCode` and
  friends read better at the call site than shortened names.
- `struct_field_names` on `ProbabilityThresholds`: the field names are the
  API's.
- `many_single_char_names` and `too_many_lines` on the SHA-256 function:
  the standard's names and constant table.
- `too_many_lines` in `src/tests`: the vector runner and the URL test are
  long tables of cases; splitting them would hide the cases.
- `derivable_impls` on `Default for Units`: the enum is macro-generated.
- `float_cmp` in the query builder: it compares a setting with its exact
  route default, not a computed value.
- `cast_precision_loss` in the jitter source: 53 random bits fit an `f64`.
- `clippy.toml` lists `ETag`, `OpenAPI`, `AdSense` and `snake_case` as names
  for `doc_markdown`.

## A good bug report

The [bug report form](https://github.com/Ourea-Technologies/ensemble-weather-rust/issues/new?template=bug_report.yml)
asks for these; each one saves a round trip:

- the SDK version (`ensemble_weather::VERSION`, or `cargo tree -i ensemble-weather`);
- your Rust version and OS (`rustc --version`, `uname -a`);
- what you did, what you expected and what happened instead;
- a minimal snippet that shows it, ideally runnable;
- the request ID from the error (`err.api_error().and_then(|e| e.request_id())`),
  which lets us find the request in our logs.

Never paste an API key into an issue. If you did by accident, rotate it at
https://oureawx.com.

## Feature ideas

Open a [feature request](https://github.com/Ourea-Technologies/ensemble-weather-rust/issues/new?template=feature_request.yml)
describing the problem first, then the API you would like. We decide by
asking whether it fits the SDK's job (carrying what the server computes; the
SDK never does statistics itself), whether it reads like idiomatic Rust, and
what it costs in dependencies and API surface. The API's behaviour lives in
ensemble-weather-spec, so a request that needs a server change (a new
parameter or answer) may be moved there and labelled `needs-spec`.

Questions and show-and-tell are best in
[Discussions](https://github.com/Ourea-Technologies/ensemble-weather-rust/discussions).

## How quickly we answer

We aim to give every new issue and pull request a first response within a
few business days. A first response is a human reply, not necessarily a fix.
If something has waited longer, a friendly ping on the thread is welcome.

## Updating the spec

```sh
scripts/sync-spec --ref <tag>   # rewrites spec/ and spec/VERSION
scripts/sync-spec --check       # what CI runs
```

`OUREA_SPEC_REPO=/path/to/ensemble-weather-spec` syncs from a local clone.
A spec change that changes behaviour arrives with new vectors; make them
pass rather than editing them.

## Dependencies

Every runtime dependency is a supply-chain risk and an install cost, so the
list is short and each entry is justified here. `scripts/check-deps` fails
CI if `Cargo.toml` names a dependency without a line below, and a new entry
needs Harrison's approval.

### Runtime dependencies

- `reqwest`: the HTTP client (HTTP/1.1, connection pooling, timeouts) over
  rustls, with `default-features = false` and only `rustls`: no OpenSSL, no
  native-tls. Rust has no standard-library HTTP client. Redirects are turned
  off in it; the SDK follows them itself so it can drop the key on another
  host.
- `serde`: the derive-based (de)serialisation of every model; fixed types
  only, no polymorphic deserialisation.
- `serde_json`: JSON parsing of responses (and `serde_json::Value` for the
  API's few free-form fields).
- `thiserror`: the `Error` enum's `Display` and `source` impls, with no
  runtime cost.
- `tokio`: timers for retries and per-attempt timeouts (`time` only); the
  `blocking` feature adds `rt` for the synchronous client's runtime. reqwest
  needs Tokio already.
- `futures-core`: the `Stream` trait the list iterators implement, so they
  work with any stream combinator library.
- `url`: parsing the base URL and resolving redirect `Location`s (already in
  reqwest's tree).

### Dev dependencies

These are pinned by `Cargo.lock` and never ship:

- `tokio` (with `macros`, `rt-multi-thread`, `test-util`, `net`, `io-util`,
  `sync`): async tests, the paused clock for retry delays, and the local
  stub server.
- `regex`: the vectors' `headers_match` expectations.
- `yaml-rust2`: parses `spec/openapi/v1/openapi.yaml` for the spec-drift
  tests.

The SDK hashes the API key into cache keys with a small built-in SHA-256
(tested against the FIPS vectors) and draws backoff jitter from the
standard library's randomly keyed hasher, rather than adding `sha2` and
`rand`.

## Commits and pull requests

- One logical change per commit; an imperative summary line (`Add`, `Fix`,
  ...) under about 72 characters, then a short body saying what and why.
- Every behaviour change updates the README or docs, adds a test (a red-first
  regression test for a fix), and a line under `[Unreleased]` in
  `CHANGELOG.md`, crediting outside contributors by GitHub handle
  ("Thanks @handle").
- `main` is protected: CI must pass and history stays linear (rebase, no
  merge commits).

## Releasing

1. Move the `[Unreleased]` notes to a new version section in `CHANGELOG.md`
   and bump `version` in `Cargo.toml`; merge to `main`.
2. Tag `main`: `git tag -s vX.Y.Z -m vX.Y.Z && git push origin vX.Y.Z`.
3. `release.yml` re-runs the gates, checks the tag matches `Cargo.toml`,
   publishes to crates.io through trusted publishing (OIDC, no stored token),
   and creates the GitHub release with the changelog section, the `.crate`
   file and its build-provenance attestation.

**First release only:** crates.io trusted publishing must be configured for
the crate (crate settings: repository `Ourea-Technologies/ensemble-weather-rust`,
workflow `release.yml`, environment `crates-io`). A brand-new crate cannot use
trusted publishing for its very first upload, so `0.1.0` is published once by
an owner with `cargo publish`, and every later release by the workflow.

## Code of conduct

This project follows the [Contributor Covenant](CODE_OF_CONDUCT.md).
