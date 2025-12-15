// Sample Rust code for testing editor with realistic content
// Good for testing syntax patterns, indentation, and navigation

use std::collections::HashMap;
use std::io::{self, Read, Write};

/// A simple key-value store
pub struct Store<K, V> {
    data: HashMap<K, V>,
    capacity: usize,
}

impl<K: Eq + std::hash::Hash, V> Store<K, V> {
    /// Create a new store with the capacity
    pub fn new(capacity: usize) -> Self {
        Self {
            data: HashMap::with_capacity(capacity),
            capacity,
        }
    }

    /// Insert a key-value pair
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        if self.data.len() >= self.capacity {
            return None;
        }
        self.data.insert(key, value)
    }

    /// Get a reference to a value
    pub fn get(&self, key: &K) -> Option<&V> {
        self.data.get(key)
    }

    /// Remove a key-value pair
    pub fn remove(&mut self, key: &K) -> Option<V> {
        self.data.remove(key)
    }

    /// Check if the store is empty
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Get the number of entries
    pub fn len(&self) -> usize {
        self.data.len()
    }
}

fn main() -> io::Result<()> {
    let mut store: Store<String, i32> = Store::new(100);

    // Insert some values
    store.insert("one".to_string(), 1);
    store.insert("two".to_string(), 2);
    store.insert("three".to_string(), 3);

    // Pattern matching example
    match store.get(&"two".to_string()) {
        Some(value) => println!("Found: {}", value),
        None => println!("Not found"),
    }

    // Iterator example with closures
    let numbers: Vec<i32> = (1..=10)
        .filter(|n| n % 2 == 0)
        .map(|n| n * n)
        .collect();

    println!("Even squares: {:?}", numbers);

    // Error handling example
    let result: Result<i32, &str> = Ok(42);
    let value = result.unwrap_or_else(|e| {
        eprintln!("Error: {}", e);
        0
    });

    // Nested structures for indentation testing
    if value > 0 {
        for i in 0..value {
            if i % 2 == 0 {
                if i % 4 == 0 {
                    println!("Divisible by 4: {}", i);
                } else {
                    println!("Even but not by 4: {}", i);
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_store_insert() {
        let mut store = Store::new(10);
        assert!(store.insert("key", "value").is_none());
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn test_store_get() {
        let mut store = Store::new(10);
        store.insert("key", 42);
        assert_eq!(store.get(&"key"), Some(&42));
        assert_eq!(store.get(&"missing"), None);
    }

    #[test]
    fn test_store_capacity() {
        let mut store: Store<i32, i32> = Store::new(2);
        store.insert(1, 1);
        store.insert(2, 2);
        // Should fail - at capacity
        assert!(store.insert(3, 3).is_none());
    }
}
