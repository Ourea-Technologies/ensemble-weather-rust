//! How a response was obtained: rate-limit state, request ID, revalidation.

/// A response's HTTP headers, in the order received. Names compare
/// case-insensitively.
///
/// ```
/// # use ensemble_weather::ResponseMeta;
/// fn etag(meta: &ResponseMeta) -> Option<&str> {
///     meta.headers.get("etag")
/// }
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Headers(Vec<(String, String)>);

impl Headers {
    pub(crate) fn new(pairs: Vec<(String, String)>) -> Self {
        Self(pairs)
    }

    /// The first value of the header `name`.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&str> {
        self.0
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    /// Every value of the header `name`.
    pub fn get_all<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a str> + 'a {
        self.0
            .iter()
            .filter(move |(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    /// Every header, as `(name, value)` pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.0.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    /// The pairs, for storing in a cache.
    pub(crate) fn pairs(&self) -> &[(String, String)] {
        &self.0
    }

    /// `self` with every header of `fresh` replacing the same-named ones.
    pub(crate) fn merged(&self, fresh: &Headers) -> Headers {
        let mut out: Vec<(String, String)> = self
            .0
            .iter()
            .filter(|(k, _)| fresh.get(k).is_none())
            .cloned()
            .collect();
        out.extend(fresh.0.iter().cloned());
        Headers(out)
    }
}

/// A response's rate-limit state, from its `RateLimit-*` headers.
///
/// Read it rather than hard-coding limits. Responses a shared cache may store
/// (anonymous forecasts, for example) carry none.
///
/// ```
/// # use ensemble_weather::Forecast;
/// fn report(forecast: &Forecast) {
///     match &forecast.meta.rate_limit {
///         Some(rl) => println!("{:?} of {:?} left", rl.remaining, rl.limit),
///         None => println!("no rate-limit headers (an edge-cacheable response)"),
///     }
/// }
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct RateLimit {
    /// The request budget of the current window (`RateLimit-Limit`).
    pub limit: Option<u64>,
    /// What is left of it (`RateLimit-Remaining`).
    pub remaining: Option<u64>,
    /// Seconds until the window resets (`RateLimit-Reset`).
    pub reset: Option<u64>,
    /// The raw `RateLimit-Policy`, such as `"60;w=60"`.
    pub policy: Option<String>,
}

impl RateLimit {
    pub(crate) fn from_headers(h: &Headers) -> Option<Self> {
        let num = |name: &str| h.get(name).and_then(|v| v.trim().parse().ok());
        let rl = Self {
            limit: num("RateLimit-Limit"),
            remaining: num("RateLimit-Remaining"),
            reset: num("RateLimit-Reset"),
            policy: h.get("RateLimit-Policy").map(str::to_owned),
        };
        (rl != Self::default()).then_some(rl)
    }
}

/// How a response was obtained. Every top-level result carries one in its
/// `meta` field.
///
/// ```
/// # use ensemble_weather::Forecast;
/// fn describe(forecast: &Forecast) {
///     if forecast.meta.revalidated {
///         println!("not modified: served from the local ETag cache");
///     }
///     if let Some(id) = &forecast.meta.request_id {
///         println!("request {id}");
///     }
/// }
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct ResponseMeta {
    /// The response's headers (for a revalidated response, the stored headers
    /// updated with the fresh ones of the 304).
    pub headers: Headers,
    /// The rate-limit state, or `None` when the response had none.
    pub rate_limit: Option<RateLimit>,
    /// The response's `X-Request-Id`, when present.
    pub request_id: Option<String>,
    /// Whether the server answered `304 Not Modified` and the body came from
    /// the client's ETag cache.
    pub revalidated: bool,
}

/// Implemented by every top-level result so the client can attach its meta.
pub(crate) trait WithMeta {
    fn set_meta(&mut self, meta: ResponseMeta);
}

macro_rules! with_meta {
    ($($t:ty),+ $(,)?) => {
        $(impl $crate::meta::WithMeta for $t {
            fn set_meta(&mut self, meta: $crate::meta::ResponseMeta) {
                self.meta = meta;
            }
        })+
    };
}
pub(crate) use with_meta;
