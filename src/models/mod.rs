//! Typed models of every schema the SDK's operations reach.
//!
//! Fields the API may omit are `Option` (absent, never 0) or an empty `Vec`.
//! Unknown fields are ignored and unknown enum values keep their raw string,
//! so a newer API never breaks an older SDK. Times are RFC 3339 strings and
//! dates `YYYY-MM-DD` strings, exactly as the API sends them, so you can parse
//! them with the date library you already use.

mod catalog;
mod forecast;
mod location;
mod misc;

pub use catalog::*;
pub use forecast::*;
pub use location::*;
pub use misc::*;
