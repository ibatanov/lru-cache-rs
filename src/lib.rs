use std::collections::HashMap;
use std::ptr::NonNull;
use std::time::{Duration, Instant};
use std::hash::Hash;

struct Node<K, V> {
    key: K,
    value: V,
    expires_at: Option<Instant>,
    next: Option<NonNull<Node<K, V>>>,
    prev: Option<NonNull<Node<K, V>>>,
}

pub struct LruCache<K, V> {
    map: HashMap<K, NonNull<Node<K, V>>>,
    head: Option<NonNull<Node<K, V>>>,
    tail: Option<NonNull<Node<K, V>>>,
    capacity: usize,
}

impl<K: Eq + Hash + Clone, V> LruCache<K, V> {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0);
        LruCache {
            map: HashMap::with_capacity(capacity),
            head: None,
            tail: None,
            capacity,
        }
    }

    pub fn put(&mut self, key: K, value: V, ttl: Option<Duration>) {
        self.evict_expired();
        let expires_at = ttl.map(|d| Instant::now() + d);

        if let Some(node_ptr) = self.map.get(&key).cloned() {
            unsafe {
                let node = node_ptr.as_ptr().as_mut().unwrap();
                node.value = value;
                node.expires_at = expires_at;
                self.remove_node(node_ptr);
                self.push_front(node_ptr);
            }
            return;
        }

        if self.map.len() >= self.capacity {
            self.remove_last();
        }

        let node = Box::new(Node {
            key: key.clone(),
            value,
            expires_at,
            next: self.head,
            prev: None,
        });

        let node_ptr = unsafe { NonNull::new_unchecked(Box::into_raw(node)) };

        if let Some(mut head) = self.head {
            unsafe { head.as_mut().prev = Some(node_ptr) };
        } else {
            self.tail = Some(node_ptr);
        }

        self.head = Some(node_ptr);
        self.map.insert(key, node_ptr);
    }

    pub fn get(&mut self, key: &K) -> Option<&V> {
        self.evict_expired();

        let node_ptr = self.map.get(key).cloned()?;

        unsafe {
            let node = node_ptr.as_ptr().as_ref().unwrap();

            if node.expired() {
                self.map.remove(key);
                self.remove_node(node_ptr);
                let _ = Box::from_raw(node_ptr.as_ptr());
                return None;
            }

            self.remove_node(node_ptr);
            self.push_front(node_ptr);

            Some(&(*node_ptr.as_ptr()).value)
        }
    }

    pub fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        self.evict_expired();

        let node_ptr = self.map.get(key).cloned()?;

        unsafe {
            let node = node_ptr.as_ptr().as_mut().unwrap();

            if node.expired() {
                self.map.remove(key);
                self.remove_node(node_ptr);
                let _ = Box::from_raw(node_ptr.as_ptr());
                return None;
            }

            self.remove_node(node_ptr);
            self.push_front(node_ptr);

            Some(&mut (*node_ptr.as_ptr()).value)
        }
    }

    fn remove_node(&mut self, node_ptr: NonNull<Node<K, V>>) {
        unsafe {
            match (*node_ptr.as_ptr()).prev {
                Some(prev) => {
                    let prev_mut = prev.as_ptr() as *mut Node<K, V>;
                    (*prev_mut).next = (*node_ptr.as_ptr()).next;
                },
                None => self.head = (*node_ptr.as_ptr()).next,
            }

            match (*node_ptr.as_ptr()).next {
                Some(next) => {
                    let next_mut = next.as_ptr() as *mut Node<K, V>;
                    (*next_mut).prev = (*node_ptr.as_ptr()).prev;
                },
                None => self.tail = (*node_ptr.as_ptr()).prev,
            }
        }
    }

    fn push_front(&mut self, node_ptr: NonNull<Node<K, V>>) {
        unsafe {
            (*node_ptr.as_ptr()).next = self.head;
            (*node_ptr.as_ptr()).prev = None;

            if let Some(head) = self.head {
                let head_mut = head.as_ptr() as *mut Node<K, V>;
                (*head_mut).prev = Some(node_ptr);
            } else {
                self.tail = Some(node_ptr);
            }

            self.head = Some(node_ptr);
        }
    }

    fn remove_last(&mut self) {
        if let Some(tail_ptr) = self.tail {
            unsafe {
                let key = (*tail_ptr.as_ptr()).key.clone();
                let prev = (*tail_ptr.as_ptr()).prev;

                self.map.remove(&key);

                match prev {
                    Some(prev) => {
                        let prev_mut = prev.as_ptr() as *mut Node<K, V>;
                        (*prev_mut).next = None;
                        self.tail = Some(prev);
                    },
                    None => {
                        self.head = None;
                        self.tail = None;
                    }
                }

                let _ = Box::from_raw(tail_ptr.as_ptr());
            }
        }
    }

    fn evict_expired(&mut self) {
        let now = Instant::now();
        let mut expired_keys = Vec::new();

        for (key, &node_ptr) in &self.map {
            unsafe {
                if (*node_ptr.as_ptr()).expired_at(now) {
                    expired_keys.push(key.clone());
                }
            }
        }

        for key in expired_keys {
            if let Some(node_ptr) = self.map.remove(&key) {
                self.remove_node(node_ptr);
                unsafe { let _ = Box::from_raw(node_ptr.as_ptr()); }
            }
        }
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }
}

impl<K, V> Drop for LruCache<K, V> {
    fn drop(&mut self) {
        let mut current = self.head;
        while let Some(node_ptr) = current {
            unsafe {
                current = (*node_ptr.as_ptr()).next;
                let _ = Box::from_raw(node_ptr.as_ptr());
            }
        }
    }
}

impl<K, V> Node<K, V> {
    fn expired(&self) -> bool {
        self.expires_at.map_or(false, |e| e <= Instant::now())
    }

    fn expired_at(&self, now: Instant) -> bool {
        self.expires_at.map_or(false, |e| e <= now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_basic_operations() {
        let mut cache = LruCache::new(2);
        cache.put("a", 1, None);
        cache.put("b", 2, None);

        assert_eq!(cache.get(&"a"), Some(&1));
        assert_eq!(cache.get(&"b"), Some(&2));
        assert_eq!(cache.get(&"c"), None);

        cache.put("c", 3, None);
        assert_eq!(cache.get(&"a"), None);
        assert_eq!(cache.get(&"b"), Some(&2));
        assert_eq!(cache.get(&"c"), Some(&3));
    }

    #[test]
    fn test_ttl_expiration() {
        let mut cache = LruCache::new(2);
        cache.put("a", 1, Some(Duration::from_millis(100)));
        cache.put("b", 2, None);

        assert_eq!(cache.get(&"a"), Some(&1));
        assert_eq!(cache.get(&"b"), Some(&2));

        thread::sleep(Duration::from_millis(150));

        assert_eq!(cache.get(&"a"), None);
        assert_eq!(cache.get(&"b"), Some(&2));
    }

    #[test]
    fn test_lru_eviction() {
        let mut cache = LruCache::new(3);
        cache.put("a", 1, None);
        cache.put("b", 2, None);
        cache.put("c", 3, None);

        cache.get(&"a");
        cache.put("d", 4, None);

        assert_eq!(cache.get(&"b"), None);
        assert_eq!(cache.get(&"a"), Some(&1));
        assert_eq!(cache.get(&"c"), Some(&3));
        assert_eq!(cache.get(&"d"), Some(&4));
    }

    #[test]
    fn test_no_memory_leaks() {
        let mut cache = LruCache::new(2);
        for i in 0..1000 {
            cache.put(i, Box::new([0u8; 1024]), None);
        }
    }
}
