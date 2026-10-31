# Good first issues (draft)

Three starter issues to file with the `good first issue` label when the
repository is published. Each is small, self-contained and real.

## 1. Add a `nearby` example

**What:** a new `examples/nearby.rs` that lists the locations near
`or/mt-hood-meadows` (`client.nearby(..).limit(5)`) and near a point
(`client.get_point_nearby(Point::new(45.33, -121.66))`), printing each name, key and
distance.

**Why:** nearby is the one family of operations with no example yet.

**Done when:** the example runs against the stub in `tests/examples.rs` (add a
`["locations", _, _, "nearby"]` and `["nearby"]` route to `tests/support`
with a small fixture in `tests/fixtures/`), it is listed in the README's
examples table, and `scripts/check` passes.

## 2. `Display` for `ProbabilityResult` and `WindowTotal`

**What:** implement `std::fmt::Display` so an answer prints with its
reference class, for example
`42% chance of snowfall >= 6.000 in over 0-24 h (131 members)`, and
`not covered over 0-240 h` when `probability` is `None`. The same for a
`WindowTotal` (median and 80 % range with the unit).

**Why:** every example formats these by hand; BEHAVIOR 1.3 says a probability
always travels with its reference class, and a `Display` impl makes that the
easy path.

**Done when:** both impls have doc examples and unit tests (including the
absent case), and the examples use them.

## 3. Run the shared vectors through the blocking client too

**What:** run every case in `spec/vectors` a second time through
`blocking::Client` (feature `blocking`), so the two clients are proven to
behave identically.

**Why:** today the vectors run on the async client, and the blocking client
is covered by one end-to-end test.

**Done when:** `cargo test --all-features` runs 43 vectors per client; the
runner's per-operation mapping is shared rather than copied.
