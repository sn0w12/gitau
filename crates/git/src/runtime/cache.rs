use std::collections::{HashMap, VecDeque};
use std::hash::{Hash, Hasher};
use std::sync::Mutex;

use serde::Serialize;

/// Minimal LRU used for generation-scoped caches. Entries never outlive the
/// generation they were computed in, so invalidation is a single clear.
pub struct Lru<K, V> {
    capacity: usize,
    inner: Mutex<LruInner<K, V>>,
}

struct LruInner<K, V> {
    map: HashMap<K, V>,
    order: VecDeque<K>,
}

impl<K: Eq + Hash + Clone, V> Lru<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            inner: Mutex::new(LruInner {
                map: HashMap::new(),
                order: VecDeque::new(),
            }),
        }
    }

    pub fn get(&self, key: &K) -> Option<V>
    where
        V: Clone,
    {
        let mut guard = self.inner.lock().unwrap();
        let value = guard.map.get(key).cloned()?;
        guard.order.retain(|k| k != key);
        guard.order.push_back(key.clone());
        Some(value)
    }

    pub fn insert(&self, key: K, value: V) {
        let mut guard = self.inner.lock().unwrap();
        if guard.map.contains_key(&key) {
            guard.order.retain(|k| k != &key);
        }
        guard.order.push_back(key.clone());
        guard.map.insert(key, value);
        while guard.map.len() > self.capacity {
            if let Some(oldest) = guard.order.pop_front() {
                guard.map.remove(&oldest);
            } else {
                break;
            }
        }
    }

    pub fn clear(&self) {
        let mut guard = self.inner.lock().unwrap();
        guard.map.clear();
        guard.order.clear();
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.inner.lock().unwrap().map.len()
    }

    #[cfg(test)]
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Stable hash for cache keys derived from serialized parameters.
pub fn params_hash(kind: &'static str, params: &impl Serialize) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    kind.hash(&mut hasher);
    match serde_json::to_vec(params) {
        Ok(bytes) => bytes.hash(&mut hasher),
        Err(_) => 0u64.hash(&mut hasher),
    }
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evicts_least_recently_used() {
        let cache = Lru::new(2);
        cache.insert("a", 1);
        cache.insert("b", 2);
        assert_eq!(cache.get(&"a"), Some(1));
        cache.insert("c", 3);
        assert_eq!(cache.get(&"b"), None);
        assert_eq!(cache.get(&"a"), Some(1));
        assert_eq!(cache.get(&"c"), Some(3));
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn clear_empties_everything() {
        let cache = Lru::new(4);
        cache.insert(1, "x");
        cache.clear();
        assert_eq!(cache.get(&1), None);
    }

    #[test]
    fn same_params_same_key() {
        let a = params_hash("status", &serde_json::json!({"includeIgnored": false}));
        let b = params_hash("status", &serde_json::json!({"includeIgnored": false}));
        let c = params_hash("history", &serde_json::json!({"limit": 100}));
        assert_eq!(a, b);
        assert_ne!(a, c);
    }
}
