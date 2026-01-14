use crate::model::LlmResponse;
use moka::future::Cache;
use blake3::Hash;
use std::time::Duration;

#[derive(Clone)]
pub struct SemanticCache {
    inner: Cache<String, LlmResponse>,
}

impl SemanticCache {
    pub fn new(max_capacity: u64, ttl_secs: u64) -> Self {
        let inner = Cache::builder()
            .max_capacity(max_capacity)
            .time_to_live(Duration::from_secs(ttl_secs))
            .build();
        Self { inner }
    }

    pub async fn get(&self, prompt: &str) -> Option<LlmResponse> {
        let key = self.hash_key(prompt);
        self.inner.get(&key).await
    }

    pub async fn put(&self, prompt: &str, response: LlmResponse) {
        let key = self.hash_key(prompt);
        self.inner.insert(key, response).await;
    }

    fn hash_key(&self, prompt: &str) -> String {
        // Normalize: trim, lowercase (optional, depending on strictness)
        // For now, strict hashing of the prompt content.
        // In a real semantic cache, we might want to use embeddings, 
        // but the requirement said "Hash determinístico do prompt".
        let hash = blake3::hash(prompt.as_bytes());
        hash.to_hex().to_string()
    }
}
