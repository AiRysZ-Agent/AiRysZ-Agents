use std::sync::Arc;
use tokio::sync::RwLock;
use lru::LruCache;

pub struct DocumentCache {
    cache: Arc<RwLock<LruCache<String, Vec<insights::Insight>>>>,
}

impl DocumentCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            cache: Arc::new(RwLock::new(LruCache::new(capacity))),
        }
    }

    pub async fn get(&self, key: &str) -> Option<Vec<insights::Insight>> {
        self.cache.read().await.get(key).cloned()
    }

    pub async fn insert(&self, key: String, value: Vec<insights::Insight>) {
        self.cache.write().await.put(key, value);
    }
} 