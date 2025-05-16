use std::collections::HashMap;
use std::hash::Hash;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

pub struct SafeLRUCache<K, V> {
    inner: Arc<RwLock<InnerLRUCache<K, V>>>,
}

struct InnerLRUCache<K, V> {
    capacity: usize,
    max_size: Option<usize>,
    current_size: usize,
    map: HashMap<Arc<K>, CacheItem<V>>,
    order: Vec<Arc<K>>,
}

#[derive(Clone)]
struct CacheItem<V> {
    value: V,
    size: usize,
    expires_at: Option<Instant>,
}

impl<K, V> SafeLRUCache<K, V>
where
    K: Eq + Hash + Clone + 'static,
    V: Clone + 'static,
{
    pub fn new(capacity: usize, max_size: Option<usize>) -> Self {
        Self {
            inner: Arc::new(RwLock::new(InnerLRUCache {
                capacity,
                max_size,
                current_size: 0,
                map: HashMap::with_capacity(capacity),
                order: Vec::with_capacity(capacity),
            })),
        }
    }

    pub fn put(&self, key: K, value: V, ttl: Option<Duration>, size: usize) {
        let mut inner = self.inner.write().unwrap();
        let expires_at = ttl.map(|d| Instant::now() + d);

        let item = CacheItem {
            value,
            size,
            expires_at,
        };

        // Check size limit
        if let Some(max) = inner.max_size {
            while inner.current_size + item.size > max && !inner.map.is_empty() {
                inner.remove_oldest();
            }
        }

        // Check capacity limit
        while inner.map.len() >= inner.capacity && !inner.map.is_empty() {
            inner.remove_oldest();
        }

        let key_arc = Arc::new(key);
        if inner.map.insert(Arc::clone(&key_arc), item).is_none() {
            inner.current_size += size;
        }
        inner.order.push(key_arc);
    }

    pub fn get(&self, key: &K) -> Option<V> {
        // First check existence and check expired with write lock
        let exists = {
            let inner = self.inner.read().unwrap();
            inner.map.contains_key(key)
        };

        if !exists {
            return None;
        }

        // Upgrade to write lock to modify order
        let mut inner = self.inner.write().unwrap();
        inner.clear_expired();

        if let Some(pos) = inner.order.iter().position(|k| k.as_ref() == key) {
            let key_clone = Arc::new(key.clone());
            inner.order.remove(pos);
            inner.order.push(key_clone);
            inner.map.get(key).map(|item| item.value.clone())
        } else {
            None
        }
    }

    pub fn clear_expired(&self) {
        let mut inner = self.inner.write().unwrap();
        inner.clear_expired();
    }
}

impl<K, V> InnerLRUCache<K, V>
where
    K: Eq + Hash,
{
    fn remove_oldest(&mut self) {
        if let Some(oldest_key) = self.order.first() {
            if let Some(item) = self.map.remove(oldest_key) {
                self.current_size -= item.size;
            }
            self.order.remove(0);
        }
    }

    fn clear_expired(&mut self) {
        let now = Instant::now();
        let mut i = 0;
        while i < self.order.len() {
            let key = &self.order[i];
            if let Some(item) = self.map.get(key) {
                if let Some(expiry) = item.expires_at {
                    if expiry <= now {
                        if let Some(removed_item) = self.map.remove(key) {
                            self.current_size -= removed_item.size;
                        }
                        self.order.remove(i);
                        continue;
                    }
                }
            }
            i += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_basic_operations() {
        let cache = SafeLRUCache::new(2, None);

        cache.put("key1", "value1", None, 1);
        cache.put("key2", "value2", None, 1);
        assert_eq!(cache.get(&"key1"), Some("value1"));

        // LRU eviction
        cache.put("key3", "value3", None, 1);
        assert_eq!(cache.get(&"key2"), None);
    }

    #[test]
    fn test_ttl() {
        let cache = SafeLRUCache::new(10, None);

        cache.put("key1", "value1", Some(Duration::from_millis(100)), 1);
        assert_eq!(cache.get(&"key1"), Some("value1"));

        thread::sleep(Duration::from_millis(150));
        assert_eq!(cache.get(&"key1"), None);
    }

    #[test]
    fn test_size_limit() {
        let cache = SafeLRUCache::new(10, Some(100));

        cache.put("key1", vec![0u8, 1, 2], None, 50);
        cache.put("key2", vec![3u8, 4, 5], None, 60);

        // Both items should be evicted when adding a large one
        cache.put("key3", vec![6u8, 7, 8, 9], None, 90);
        assert_eq!(cache.get(&"key1"), None);
        assert_eq!(cache.get(&"key2"), None);
        assert_eq!(cache.get(&"key3"), Some(vec![6u8, 7, 8, 9]));
    }

    #[test]
    fn test_update_existing() {
        let cache = SafeLRUCache::new(2, None);

        cache.put("key1", "value1", None, 1);
        cache.put("key1", "value1_new", None, 1);
        assert_eq!(cache.get(&"key1"), Some("value1_new"));
    }

    #[test]
    fn test_concurrent_access() {
        let cache = Arc::new(SafeLRUCache::new(100, None));
        let mut handles = vec![];

        for i in 0..10 {
            let cache = Arc::clone(&cache);
            handles.push(thread::spawn(move || {
                cache.put(i, i*2, None, 1);
                assert_eq!(cache.get(&i), Some(i*2));
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }
}
