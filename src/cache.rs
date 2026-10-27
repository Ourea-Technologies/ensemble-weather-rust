//! The client's ETag cache (BEHAVIOR 10).

use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::fmt::Write as _;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, SystemTime};

/// A store for responses, used to revalidate them with `If-None-Match`.
///
/// The client's default is a [`MemoryCache`] (256 entries, 32 MiB). Implement
/// this trait to share a cache, for example across processes, and pass it to
/// [`ClientBuilder::cache`](crate::ClientBuilder::cache); or turn caching off
/// with [`ClientBuilder::no_cache`](crate::ClientBuilder::no_cache).
///
/// Keys are opaque strings built from the full request URL and a SHA-256
/// hash of the API key the request carried (never the key itself), so keyless
/// and keyed entries never mix. A failing store should behave as a miss.
///
/// ```
/// use std::collections::HashMap;
/// use std::sync::Mutex;
/// use ensemble_weather::{Cache, CacheEntry};
///
/// /// An unbounded cache, for illustration.
/// #[derive(Debug, Default)]
/// struct MapCache(Mutex<HashMap<String, CacheEntry>>);
///
/// impl Cache for MapCache {
///     fn get(&self, key: &str) -> Option<CacheEntry> {
///         self.0.lock().ok()?.get(key).cloned()
///     }
///     fn set(&self, key: &str, entry: CacheEntry) {
///         if let Ok(mut map) = self.0.lock() {
///             map.insert(key.to_owned(), entry);
///         }
///     }
///     fn remove(&self, key: &str) {
///         if let Ok(mut map) = self.0.lock() {
///             map.remove(key);
///         }
///     }
/// }
///
/// let client = ensemble_weather::Client::builder().cache(MapCache::default()).build();
/// # let _ = client;
/// ```
pub trait Cache: Send + Sync + fmt::Debug {
    /// The entry stored under `key`.
    fn get(&self, key: &str) -> Option<CacheEntry>;
    /// Stores `entry` under `key`, replacing any entry there.
    fn set(&self, key: &str, entry: CacheEntry);
    /// Removes the entry stored under `key`.
    fn remove(&self, key: &str);
}

/// One stored response.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct CacheEntry {
    /// The validator exactly as the server sent it (including any `W/`).
    pub etag: Option<String>,
    /// The response body.
    pub body: Arc<[u8]>,
    /// The response's headers, as `(name, value)` pairs.
    pub headers: Vec<(String, String)>,
    /// When the response was stored or last revalidated.
    pub stored_at: SystemTime,
    /// The response's `Cache-Control: max-age` (the API sends 0, so every use
    /// revalidates).
    pub max_age: Duration,
}

impl CacheEntry {
    /// A new entry.
    #[must_use]
    pub fn new(
        etag: Option<String>,
        body: Arc<[u8]>,
        headers: Vec<(String, String)>,
        stored_at: SystemTime,
        max_age: Duration,
    ) -> Self {
        Self {
            etag,
            body,
            headers,
            stored_at,
            max_age,
        }
    }

    fn size(&self) -> usize {
        self.body.len()
            + self.etag.as_ref().map_or(0, String::len)
            + self
                .headers
                .iter()
                .map(|(k, v)| k.len() + v.len())
                .sum::<usize>()
    }
}

/// A size-bounded, thread-safe, in-memory LRU [`Cache`].
///
/// ```
/// use ensemble_weather::{Client, MemoryCache};
/// // Up to 1,000 entries and 64 MiB.
/// let client = Client::builder().cache(MemoryCache::new(1_000, 64 << 20)).build();
/// # let _ = client;
/// ```
#[derive(Debug)]
pub struct MemoryCache {
    max_entries: usize,
    max_bytes: usize,
    inner: Mutex<Lru>,
}

#[derive(Debug, Default)]
struct Lru {
    bytes: usize,
    /// Keys, least recently used first.
    order: VecDeque<String>,
    items: HashMap<String, (CacheEntry, usize)>,
}

impl Lru {
    fn touch(&mut self, key: &str) {
        if let Some(k) = self
            .order
            .iter()
            .position(|k| k == key)
            .and_then(|i| self.order.remove(i))
        {
            self.order.push_back(k);
        }
    }

    fn remove(&mut self, key: &str) {
        if let Some((_, size)) = self.items.remove(key) {
            self.bytes -= size;
            self.order.retain(|k| k != key);
        }
    }
}

/// The default bound on the number of entries.
pub const DEFAULT_CACHE_ENTRIES: usize = 256;
/// The default bound on the stored bytes.
pub const DEFAULT_CACHE_BYTES: usize = 32 << 20;

impl MemoryCache {
    /// A cache bounded to `max_entries` entries and `max_bytes` bytes.
    #[must_use]
    pub fn new(max_entries: usize, max_bytes: usize) -> Self {
        Self {
            max_entries,
            max_bytes,
            inner: Mutex::new(Lru::default()),
        }
    }

    /// The number of stored entries.
    ///
    /// ```
    /// assert_eq!(ensemble_weather::MemoryCache::default().len(), 0);
    /// ```
    #[must_use]
    pub fn len(&self) -> usize {
        self.lock().items.len()
    }

    /// Whether the cache is empty.
    ///
    /// ```
    /// assert!(ensemble_weather::MemoryCache::default().is_empty());
    /// ```
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Lru> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Default for MemoryCache {
    /// 256 entries and 32 MiB.
    fn default() -> Self {
        Self::new(DEFAULT_CACHE_ENTRIES, DEFAULT_CACHE_BYTES)
    }
}

impl Cache for MemoryCache {
    fn get(&self, key: &str) -> Option<CacheEntry> {
        let mut lru = self.lock();
        let entry = lru.items.get(key).map(|(e, _)| e.clone())?;
        lru.touch(key);
        Some(entry)
    }

    /// Stores `entry`, evicting the least recently used entries to stay
    /// within the bounds. An entry larger than the byte bound is not stored.
    fn set(&self, key: &str, entry: CacheEntry) {
        let size = entry.size() + key.len();
        let mut lru = self.lock();
        lru.remove(key);
        if size > self.max_bytes || self.max_entries == 0 {
            return;
        }
        lru.items.insert(key.to_owned(), (entry, size));
        lru.order.push_back(key.to_owned());
        lru.bytes += size;
        while lru.items.len() > self.max_entries || lru.bytes > self.max_bytes {
            let Some(oldest) = lru.order.front().cloned() else {
                break;
            };
            lru.remove(&oldest);
        }
    }

    fn remove(&self, key: &str) {
        self.lock().remove(key);
    }
}

/// The cache key: the request URL plus a hash of the credential it carried.
pub(crate) fn cache_key(url: &str, api_key: Option<&str>) -> String {
    match api_key {
        None => format!("{url} anonymous"),
        Some(key) => {
            let digest = sha256(key.as_bytes());
            let mut out = format!("{url} key:");
            for b in &digest[..8] {
                let _ = write!(out, "{b:02x}");
            }
            out
        }
    }
}

/// The parts of `Cache-Control` a private cache uses (RFC 9111). `s-maxage`
/// and `stale-while-revalidate` are for the edge and are ignored.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CacheControl {
    pub(crate) no_store: bool,
    pub(crate) max_age: Duration,
}

pub(crate) fn parse_cache_control(header: Option<&str>) -> CacheControl {
    let mut cc = CacheControl::default();
    let mut no_cache = false;
    for part in header.unwrap_or_default().split(',') {
        let (name, value) = part.trim().split_once('=').unwrap_or((part.trim(), ""));
        match name.to_ascii_lowercase().as_str() {
            "no-store" => cc.no_store = true,
            "no-cache" => no_cache = true,
            "max-age" => {
                if let Ok(n) = value.trim_matches('"').parse::<u64>() {
                    cc.max_age = Duration::from_secs(n);
                }
            }
            _ => {}
        }
    }
    if no_cache {
        cc.max_age = Duration::ZERO;
    }
    cc
}

/// SHA-256 (FIPS 180-4), for hashing the API key into cache keys without a
/// dependency. Not used for anything security-critical beyond keeping the key
/// itself out of the cache.
#[allow(clippy::many_single_char_names, clippy::too_many_lines)] // the standard's names and table
fn sha256(data: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a_2f98,
        0x7137_4491,
        0xb5c0_fbcf,
        0xe9b5_dba5,
        0x3956_c25b,
        0x59f1_11f1,
        0x923f_82a4,
        0xab1c_5ed5,
        0xd807_aa98,
        0x1283_5b01,
        0x2431_85be,
        0x550c_7dc3,
        0x72be_5d74,
        0x80de_b1fe,
        0x9bdc_06a7,
        0xc19b_f174,
        0xe49b_69c1,
        0xefbe_4786,
        0x0fc1_9dc6,
        0x240c_a1cc,
        0x2de9_2c6f,
        0x4a74_84aa,
        0x5cb0_a9dc,
        0x76f9_88da,
        0x983e_5152,
        0xa831_c66d,
        0xb003_27c8,
        0xbf59_7fc7,
        0xc6e0_0bf3,
        0xd5a7_9147,
        0x06ca_6351,
        0x1429_2967,
        0x27b7_0a85,
        0x2e1b_2138,
        0x4d2c_6dfc,
        0x5338_0d13,
        0x650a_7354,
        0x766a_0abb,
        0x81c2_c92e,
        0x9272_2c85,
        0xa2bf_e8a1,
        0xa81a_664b,
        0xc24b_8b70,
        0xc76c_51a3,
        0xd192_e819,
        0xd699_0624,
        0xf40e_3585,
        0x106a_a070,
        0x19a4_c116,
        0x1e37_6c08,
        0x2748_774c,
        0x34b0_bcb5,
        0x391c_0cb3,
        0x4ed8_aa4a,
        0x5b9c_ca4f,
        0x682e_6ff3,
        0x748f_82ee,
        0x78a5_636f,
        0x84c8_7814,
        0x8cc7_0208,
        0x90be_fffa,
        0xa450_6ceb,
        0xbef9_a3f7,
        0xc671_78f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09_e667,
        0xbb67_ae85,
        0x3c6e_f372,
        0xa54f_f53a,
        0x510e_527f,
        0x9b05_688c,
        0x1f83_d9ab,
        0x5be0_cd19,
    ];
    let mut msg = data.to_vec();
    let bit_len = (data.len() as u64).wrapping_mul(8);
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());
    for block in msg.as_chunks::<64>().0 {
        let mut w = [0u32; 64];
        for (i, word) in block.as_chunks::<4>().0.iter().enumerate() {
            w[i] = u32::from_be_bytes(*word);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ (!e & g);
            let t1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (x, y) in h.iter_mut().zip([a, b, c, d, e, f, g, hh]) {
            *x = x.wrapping_add(y);
        }
    }
    let mut out = [0u8; 32];
    for (chunk, word) in out.as_chunks_mut::<4>().0.iter_mut().zip(h) {
        chunk.copy_from_slice(&word.to_be_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(b: &[u8]) -> String {
        b.iter().fold(String::new(), |mut s, x| {
            let _ = write!(s, "{x:02x}");
            s
        })
    }

    #[test]
    fn sha256_matches_known_digests() {
        assert_eq!(
            hex(&sha256(b"")),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            hex(&sha256(b"abc")),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            hex(&sha256(
                b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"
            )),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
    }

    #[test]
    fn keys_never_contain_the_api_key_and_never_mix() {
        let a = cache_key("https://x/a", Some("ow_secret"));
        assert!(!a.contains("ow_secret"));
        assert_ne!(a, cache_key("https://x/a", Some("ow_other")));
        assert_ne!(a, cache_key("https://x/a", None));
    }

    #[test]
    fn lru_evicts_by_count_and_bytes() {
        let entry = |n: usize| {
            CacheEntry::new(
                None,
                vec![0u8; n].into(),
                Vec::new(),
                SystemTime::UNIX_EPOCH,
                Duration::ZERO,
            )
        };
        let c = MemoryCache::new(2, 100);
        c.set("a", entry(10));
        c.set("b", entry(10));
        assert!(c.get("a").is_some()); // a is now most recent
        c.set("c", entry(10));
        assert!(c.get("b").is_none());
        assert_eq!(c.len(), 2);
        c.set("big", entry(200));
        assert!(c.get("big").is_none());
        c.remove("a");
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn cache_control_ignores_edge_directives() {
        let cc = parse_cache_control(Some(
            "public, max-age=0, s-maxage=600, stale-while-revalidate=60",
        ));
        assert_eq!(
            cc,
            CacheControl {
                no_store: false,
                max_age: Duration::ZERO
            }
        );
        assert!(parse_cache_control(Some("no-store")).no_store);
        assert_eq!(
            parse_cache_control(Some("max-age=30, no-cache")).max_age,
            Duration::ZERO
        );
        assert_eq!(
            parse_cache_control(Some("max-age=30")).max_age,
            Duration::from_secs(30)
        );
    }
}
