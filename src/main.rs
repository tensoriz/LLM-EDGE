use axum::{routing::post, Router as AxumRouter};
use std::net::SocketAddr;
use std::sync::Arc;
use llm_edge::model::ProviderConfig;
use llm_edge::router::Router;
use llm_edge::cache::SemanticCache;
use llm_edge::gateway::{AppState, handle_chat_completions};
use std::collections::HashMap;

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Mock Configuration
    let mut model_map = HashMap::new();
    model_map.insert("gpt-4".to_string(), "gpt-4-turbo".to_string());
    
    let p1 = ProviderConfig {
        id: "p1".to_string(),
        name: "MockOpenAI".to_string(),
        endpoint: "http://localhost:3001/chat/completions".to_string(),
        api_key: "sk-xxx".to_string(),
        cost_per_1k_input: 0.01,
        cost_per_1k_output: 0.03,
        model_map: model_map.clone(),
    };

    let p2 = ProviderConfig {
        id: "p2".to_string(),
        name: "MockAnthropic".to_string(),
        endpoint: "http://localhost:3002/chat/completions".to_string(),
        api_key: "ant-xxx".to_string(),
        cost_per_1k_input: 0.012, // Slightly more expensive
        cost_per_1k_output: 0.035,
        model_map: model_map,
    };

    let router = Router::new(vec![p1, p2]);
    let cache = SemanticCache::new(10_000, 60 * 5); // 10k items, 5 min TTL

    let app_state = Arc::new(AppState {
        router: Arc::new(router),
        cache: Arc::new(cache),
    });

    let app = AxumRouter::new()
        .route("/v1/chat/completions", post(handle_chat_completions))
        .with_state(app_state);

    let addr = SocketAddr::from(([127, 0, 0, 1], 8080));
    println!("LLM Gateway listening on {}", addr);
    
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
