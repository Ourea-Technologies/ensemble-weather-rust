//! The canonical query string (BEHAVIOR 3.1): one spelling per request, so
//! every request is one edge-cache entry.

use std::fmt::Write as _;

/// Percent-encodes `s` with RFC 3986 rules: the unreserved characters
/// (`A-Z a-z 0-9 - . _ ~`) stay as they are and every other byte becomes
/// `%XX` in uppercase hex (space is `%20`, never `+`). Bytes in `keep` are
/// also left literal.
///
/// `url`'s form encoder is not used on purpose: it writes space as `+` and
/// leaves other characters unescaped, and neither is the one canonical
/// spelling the edge cache needs.
pub(crate) fn escape(s: &str, keep: &[u8]) -> String {
    let mut out = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') || keep.contains(&b)
        {
            out.push(char::from(b));
        } else {
            let _ = write!(out, "%{b:02X}");
        }
    }
    out
}

/// Prints a number as the shortest decimal that round-trips, never with an
/// exponent (Rust's `Display` for `f64` does exactly this).
pub(crate) fn number(v: f64) -> String {
    format!("{v}")
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Param {
    name: &'static str,
    value: String,
    keep: &'static [u8],
}

/// A canonical query: each parameter at most once (except the one declared
/// repeated parameter), none at its route default, sorted by name.
#[derive(Debug, Clone, Default)]
pub(crate) struct Query {
    params: Vec<Param>,
}

impl Query {
    pub(crate) fn set(&mut self, name: &'static str, value: impl Into<String>) {
        self.params.push(Param {
            name,
            value: value.into(),
            keep: b"",
        });
    }

    /// Sets a string unless it is absent or empty.
    pub(crate) fn opt(&mut self, name: &'static str, value: Option<&str>) {
        if let Some(v) = value.filter(|v| !v.is_empty()) {
            self.set(name, v);
        }
    }

    /// Sets a string unless it is absent, empty or the route's default.
    pub(crate) fn opt_default(&mut self, name: &'static str, value: Option<&str>, default: &str) {
        if let Some(v) = value.filter(|v| !v.is_empty() && *v != default) {
            self.set(name, v);
        }
    }

    /// Sets a comma-joined list unless it is empty.
    pub(crate) fn list(&mut self, name: &'static str, values: &[String]) {
        if !values.is_empty() {
            self.set(name, values.join(","));
        }
    }

    /// Sets an integer unless it is absent or the route's default.
    /// A comma-joined list, sorted and de-duplicated (the server's canonical
    /// form), unless it is empty.
    pub(crate) fn sorted_list(&mut self, name: &'static str, values: &[String]) {
        let mut values = values.to_vec();
        values.sort();
        values.dedup();
        self.list(name, &values);
    }

    pub(crate) fn int(&mut self, name: &'static str, value: Option<u32>, default: Option<u32>) {
        if let Some(v) = value.filter(|v| Some(*v) != default) {
            self.set(name, v.to_string());
        }
    }

    /// Sets a number unless it is absent or the route's default.
    #[allow(clippy::float_cmp)] // an exact default, not a computed value
    pub(crate) fn num(&mut self, name: &'static str, value: Option<f64>, default: Option<f64>) {
        if let Some(v) = value.filter(|v| Some(*v) != default) {
            self.set(name, number(v));
        }
    }

    /// Sets a boolean unless it is absent or the route's default.
    pub(crate) fn flag(&mut self, name: &'static str, value: Option<bool>, default: bool) {
        if let Some(v) = value.filter(|v| *v != default) {
            self.set(name, if v { "true" } else { "false" });
        }
    }

    /// Sets a value whose `:` and `,` are sent literally (`prob`, `totals`).
    pub(crate) fn literal(&mut self, name: &'static str, value: Option<&str>) {
        if let Some(v) = value.filter(|v| !v.is_empty()) {
            self.params.push(Param {
                name,
                value: v.to_owned(),
                keep: b":,",
            });
        }
    }

    /// The query string, without the leading `?`.
    pub(crate) fn encode(&self) -> String {
        let mut params = self.params.clone();
        params.sort_by(|a, b| a.name.cmp(b.name));
        params
            .iter()
            .map(|p| format!("{}={}", escape(p.name, b""), escape(&p.value, p.keep)))
            .collect::<Vec<_>>()
            .join("&")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_rfc3986() {
        assert_eq!(
            escape("Mt. Hood & co/ñ", b""),
            "Mt.%20Hood%20%26%20co%2F%C3%B1"
        );
        assert_eq!(escape("a:b,c", b":,"), "a:b,c");
        assert_eq!(escape("~_-.", b""), "~_-.");
    }

    #[test]
    fn numbers_are_shortest_round_trip_without_exponent() {
        assert_eq!(number(45.0), "45");
        assert_eq!(number(-121.6649), "-121.6649");
        assert_eq!(number(1e21), "1000000000000000000000");
        assert_eq!(number(0.000_001), "0.000001");
    }

    #[test]
    fn sorts_dedups_and_omits_defaults() {
        let mut q = Query::default();
        q.set("units", "imperial");
        q.int("max_hours", Some(24), Some(24));
        q.flag("nowcast", Some(false), true);
        q.sorted_list(
            "capability",
            &["b".to_owned(), "a".to_owned(), "a".to_owned()],
        );
        q.sorted_list("network", &[]);
        assert_eq!(q.encode(), "capability=a%2Cb&nowcast=false&units=imperial");
    }
}
