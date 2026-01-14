use crate::model::{LlmRequest, ProviderConfig, LlmResponse};
use crate::balancer::stats::ProviderStats;
use std::sync::Arc;
use arc_swap::ArcSwap;
use tracing::warn;

#[derive(Debug)]
pub struct Provider {
    pub config: ProviderConfig,
    pub stats: Arc<ProviderStats>,
}

impl Provider {
    pub fn new(config: ProviderConfig) -> Self {
        Self {
            config,
            stats: Arc::new(ProviderStats::new()),
        }
    }

    pub fn is_healthy(&self) -> bool {
        // Simple circuit breaker check
        // If consecutive errors > 5, consider unhealthy. 
        // Real implementation would have half-open state and recovery timeout.
        self.stats.consec_errors.load(std::sync::atomic::Ordering::Relaxed) < 5
    }

    pub fn supports_model(&self, model: &str) -> bool {
        // Check if the provider maps the client model to something
        self.config.model_map.contains_key(model)
    }

    pub async fn call(&self, req: &LlmRequest) -> Result<LlmResponse, String> {
        // Real HTTP call
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .unwrap_or_default();
        
        let target_model = self.config.model_map.get(&req.model).unwrap_or(&req.model).clone();
        
        // Forwarding request - in real app, we'd transform the body
        let mut body = serde_json::to_value(req).unwrap_or(serde_json::Value::Null);
        if let serde_json::Value::Object(ref mut map) = body {
            map.insert("model".to_string(), serde_json::Value::String(target_model));
        }

        let resp = client.post(&self.config.endpoint)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .json(&body)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !resp.status().is_success() {
            return Err(format!("HTTP {}", resp.status()));
        }

        // Just consuming body to ensure complete request
        let _text = resp.text().await.map_err(|e| e.to_string())?;

        // Return a generic response for the prototype
        Ok(crate::model::LlmResponse {
            content: "Content from provider".to_string(),
            usage: crate::model::TokenUsage {
                prompt_tokens: 0,
                completion_tokens: 0,
                total_tokens: 0,
            },
            provider: self.config.name.clone(),
            latency_ms: 0, // Placeholder, set by caller
        })
    }

}

pub struct Router {
    // Shared list of providers, swappable atomically.
    providers: ArcSwap<Vec<Arc<Provider>>>,
}

impl Router {
    pub fn new(configs: Vec<ProviderConfig>) -> Self {
        let providers_vec: Vec<Arc<Provider>> = configs
            .into_iter()
            .map(|c| Arc::new(Provider::new(c)))
            .collect();
        Self {
            providers: ArcSwap::from(Arc::new(providers_vec)),
        }
    }

    pub fn select(&self, req: &LlmRequest) -> Option<Arc<Provider>> {
        // Snapshot the current list of providers
        let list = self.providers.load();

        // 1. Filter candidates
        let candidates = list.iter().filter(|p| {
            p.supports_model(&req.model) && p.is_healthy()
        });

        // 2. Score candidates
        // Scoring strategy: Normalize(Cost) + Normalize(Latency_EWMA)
        // For O(1) we iterate once and keep the best.
        
        // This is a simplified "lowest score wins" strategy.
        // We can tune weights.
        let mut best_candidate: Option<Arc<Provider>> = None;
        let mut best_score = f64::MAX;

        for provider in candidates {
            // Latency in seconds (approx) for scoring
            let latency_score = provider.stats.ewma_latency_us.load(std::sync::atomic::Ordering::Relaxed) as f64 / 1000.0;
            
            // Cost per 1k input tokens (as a proxy for generic cost)
            let cost_score = provider.config.cost_per_1k_input * 1000.0; // Weight cost heavily?

            // Total Score formula needs tuning. 
            // Let's say: Score = Latency (ms) + Cost ($ * 100000)
            // Example: 100ms + $0.001*100000 (100) = 200
            let score = latency_score + (cost_score * 100.0);

            if score < best_score {
                best_score = score;
                best_candidate = Some(provider.clone());
            }
        }
        
        // If no healthy provider found, maybe try unhealthy ones (fallback)? 
        // For now, adhere to strict health check.
        
        best_candidate
    }
    
    pub fn update_providers(&self, new_configs: Vec<ProviderConfig>) {
        // In a real app we might want to preserve stats for existing providers.
        // This simple replacement resets stats, which might be bad.
        // TODO: Merge stats.
        let new_list: Vec<Arc<Provider>> = new_configs
            .into_iter()
            .map(|c| Arc::new(Provider::new(c))) // Resets stats
            .collect();
        self.providers.store(Arc::new(new_list));
    }
}
