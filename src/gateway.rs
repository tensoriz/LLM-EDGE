use crate::model::{LlmRequest, LlmResponse, TokenUsage};
use crate::router::Router;
use crate::cache::SemanticCache;
use axum::{
    extract::{State, Json},
    response::{IntoResponse, Response},
    http::StatusCode,
};
use std::sync::Arc;
use std::time::Instant;
use tracing::{info, error};

pub struct AppState {
    pub router: Arc<Router>,
    pub cache: Arc<SemanticCache>,
}

pub async fn handle_chat_completions(
    State(state): State<Arc<AppState>>,
    Json(req): Json<LlmRequest>,
) -> Response {
    let start = Instant::now();

    // 1. Cache Lookup (O(1))
    if let Some(cached_resp) = state.cache.get(&req.prompt).await {
        info!("Cache hit for prompt");
        return (StatusCode::OK, Json(cached_resp)).into_response();
    }

    // 2. Router Selection (O(1))
    let provider_opt = state.router.select(&req);
    
    match provider_opt {
        Some(provider) => {
            // 3. Provider Call
            let call_start = Instant::now();
            
            let call_result = provider.call(&req).await;
            
            let latency_duration = call_start.elapsed();
            
            match call_result {
                Ok(mut resp) => {
                    // 4. Update Stats
                    provider.stats.record_success(latency_duration);
                    
                    resp.latency_ms = latency_duration.as_millis() as u64;
                    
                    // 5. Update Cache (async/background in real impl)
                    // For prototype, we wait or spawn. Moka is fast.
                    state.cache.put(&req.prompt, resp.clone()).await;
                    
                    let total_time = start.elapsed();
                    // Overhead = Total - Latency
                    let overhead = total_time.saturating_sub(latency_duration);
                    
                    info!(
                        "Request processed in {:?} (Latency: {:?}, Overhead: {:?}) Provider: {}", 
                        total_time, latency_duration, overhead, provider.config.name
                    );

                    (StatusCode::OK, Json(resp)).into_response()
                },
                Err(e) => {
                    provider.stats.record_failure();
                    error!("Provider call failed: {}", e);
                    (StatusCode::BAD_GATEWAY, format!("Provider error: {}", e)).into_response()
                }
            }
        }
        None => {
            error!("No healthy provider found for model {}", req.model);
            (StatusCode::SERVICE_UNAVAILABLE, "No providers available").into_response()
        }
    }
}
