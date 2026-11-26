//! Typed models of every schema the SDK's operations reach. The module is
//! private: every type is re-exported at the crate root.
//!
//! Fields the API may omit are `Option` (absent, never 0) or an empty `Vec`.
//! Unknown fields are ignored and unknown enum values keep their raw string,
//! so a newer API never breaks an older SDK. Times are RFC 3339 strings and
//! dates `YYYY-MM-DD` strings, exactly as the API sends them, so you can parse
//! them with the date library you already use.
//!
//! Field names are the wire's, except the stuttering ones BEHAVIOR.md 3.0
//! renames (`Forecast::series` is the JSON `forecast`, for example); each
//! such field's docs start with its JSON name. Models decode the wire names
//! and serialise the SDK names, which they also decode.

mod catalog;
mod explore;
mod forecast;
mod geo;
mod location;
mod misc;
mod pass;
mod plume;
mod season;

pub use catalog::*;
pub use explore::*;
pub use forecast::*;
pub use geo::*;
pub use location::*;
pub use misc::*;
pub use pass::*;
pub use plume::*;
pub use season::*;
