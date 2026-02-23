//! Internal LRU state: node structure and linked list operations.

use std::collections::HashMap;

use crate::domain::cache::{CacheEntry, CacheKey};

/// A node in the LRU doubly-linked list.
#[derive(Debug, Clone)]
pub(crate) struct LruNode {
    pub key: CacheKey,
    pub entry: CacheEntry,
    pub newer: Option<CacheKey>,
    pub older: Option<CacheKey>,
}

/// Internal LRU state.
#[derive(Debug)]
pub(crate) struct LruState {
    pub map: HashMap<CacheKey, LruNode>,
    pub newest: Option<CacheKey>,
    pub oldest: Option<CacheKey>,
    pub total_bytes: usize,
}

impl LruState {
    pub fn new() -> Self {
        Self { map: HashMap::new(), newest: None, oldest: None, total_bytes: 0 }
    }

    pub fn unlink(&mut self, key: &CacheKey) {
        let node = match self.map.get(key) {
            Some(n) => n.clone(),
            None => return,
        };
        if let Some(ref nk) = node.newer {
            if let Some(nn) = self.map.get_mut(nk) {
                nn.older = node.older.clone();
            }
        }
        if let Some(ref ok) = node.older {
            if let Some(on) = self.map.get_mut(ok) {
                on.newer = node.newer.clone();
            }
        }
        if self.newest.as_ref() == Some(key) {
            self.newest = node.older.clone();
        }
        if self.oldest.as_ref() == Some(key) {
            self.oldest = node.newer.clone();
        }
    }

    pub fn push_front(&mut self, key: &CacheKey) {
        if let Some(n) = self.map.get_mut(key) {
            n.newer = None;
            n.older = self.newest.clone();
        }
        if let Some(ref cn) = self.newest {
            if let Some(c) = self.map.get_mut(cn) {
                c.newer = Some(key.clone());
            }
        }
        self.newest = Some(key.clone());
        if self.oldest.is_none() {
            self.oldest = Some(key.clone());
        }
    }

    pub fn evict_oldest(&mut self) -> Option<usize> {
        let key = self.oldest.clone()?;
        self.unlink(&key);
        self.map.remove(&key).map(|n| n.entry.byte_size)
    }
}
