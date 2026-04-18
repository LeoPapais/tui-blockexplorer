//! In-memory TTL cache adapter.
//!
//! A thin `tokio::sync::RwLock<HashMap<K, (V, Instant)>>` keyed by any
//! hashable type. Entries are evicted lazily on `get`; there is no
//! background task. Time comes from an injected [`Clock`] so tests
//! can drive expiry via `FrozenClock::advance`.
//!
//! See `plan/2-search.md` section 12.3.

use std::{
    collections::HashMap,
    hash::Hash,
    sync::Arc,
    time::{Duration, Instant},
};

use tokio::sync::RwLock;

use crate::{adapters::clock::SystemClock, application::ports::Clock};

/// Generic TTL-scoped cache. Cheap to clone (`Arc` inside) so the same
/// cache instance can be shared across tasks.
pub struct TtlCache<K, V, C = SystemClock> {
    inner: Arc<Inner<K, V, C>>,
}

struct Inner<K, V, C> {
    ttl: Duration,
    clock: C,
    entries: RwLock<HashMap<K, Entry<V>>>,
}

struct Entry<V> {
    value: V,
    inserted_at: Instant,
}

impl<K, V> TtlCache<K, V, SystemClock>
where
    K: Hash + Eq + Clone + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    /// Construct a cache with `ttl` backed by the wall clock.
    #[must_use]
    pub fn with_ttl(ttl: Duration) -> Self {
        Self::with_ttl_and_clock(ttl, SystemClock::new())
    }
}

impl<K, V, C> TtlCache<K, V, C>
where
    K: Hash + Eq + Clone + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
    C: Clock + 'static,
{
    /// Construct a cache with `ttl` and a custom clock.
    #[must_use]
    pub fn with_ttl_and_clock(ttl: Duration, clock: C) -> Self {
        Self {
            inner: Arc::new(Inner {
                ttl,
                clock,
                entries: RwLock::new(HashMap::new()),
            }),
        }
    }

    /// Get the value for `key` if a non-expired entry exists. Expired
    /// entries are evicted as a side effect (best-effort).
    pub async fn get(&self, key: &K) -> Option<V> {
        let now = self.inner.clock.now();
        {
            let guard = self.inner.entries.read().await;
            if let Some(entry) = guard.get(key)
                && now.saturating_duration_since(entry.inserted_at) < self.inner.ttl
            {
                return Some(entry.value.clone());
            }
        }
        // Either missing or expired. Best-effort evict the expired key
        // to keep the map size bounded by active traffic.
        let mut guard = self.inner.entries.write().await;
        if let Some(entry) = guard.get(key)
            && now.saturating_duration_since(entry.inserted_at) >= self.inner.ttl
        {
            guard.remove(key);
        }
        None
    }

    /// Insert `value` under `key`, refreshing its inserted-at
    /// timestamp. Overwrites an existing entry if one is present.
    pub async fn insert(&self, key: K, value: V) {
        let now = self.inner.clock.now();
        let mut guard = self.inner.entries.write().await;
        guard.insert(
            key,
            Entry {
                value,
                inserted_at: now,
            },
        );
    }

    /// Number of entries currently in the map. Intended for tests and
    /// diagnostics only: the value may be stale by the time the caller
    /// inspects it.
    pub async fn len(&self) -> usize {
        self.inner.entries.read().await.len()
    }

    /// Whether the cache is empty. Like [`Self::len`], this is a
    /// diagnostic — use it only when a stale answer is acceptable.
    pub async fn is_empty(&self) -> bool {
        self.inner.entries.read().await.is_empty()
    }

    /// TTL this cache was configured with. Useful for observability.
    #[must_use]
    pub fn ttl(&self) -> Duration {
        self.inner.ttl
    }
}

impl<K, V, C> Clone for TtlCache<K, V, C> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}
