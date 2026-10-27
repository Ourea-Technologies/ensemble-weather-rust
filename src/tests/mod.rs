//! In-crate tests: the shared vectors, the SDK's own guarantees and spec drift.

// Test runners are long tables of cases; splitting them would hide the cases.
#![allow(clippy::too_many_lines)]

mod behaviour;
mod drift;
mod mock;
mod vectors;
