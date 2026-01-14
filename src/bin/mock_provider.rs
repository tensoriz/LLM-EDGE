use axum::{routing::post, Router, Json, extract::State};
use serde_json::Value;
use std::net::SocketAddr;
use std::time::Duration;
use rand::Rng;
use tokio::time::sleep;

#[derive(Clone)]
struct ServerConfig {
    latency_ms: u64,
    error_rate: f64,
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let port = args.get(1).unwrap_or(&"3000".to_string()).parse::<u16>().unwrap();
    let latency_ms = args.get(2).unwrap_or(&"500".to_string()).parse::<u64>().unwrap();
    let error_rate = args.get(3).unwrap_or(&"0.0".to_string()).parse::<f64>().unwrap();

    let config = ServerConfig { latency_ms, error_rate };
    
    let app = Router::new()
        .route("/chat/completions", post(handler))
        .with_state(config);

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    println!("Mock Provider running on localhost:{}. Latency: {}ms, Error Rate: {}", port, latency_ms, error_rate);
    
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn handler(State(config): State<ServerConfig>, Json(_req): Json<Value>) -> (axum::http::StatusCode, Json<Value>) {
    // Simulate Latency
    let jitter = rand::thread_rng().gen_range(0..=20);
    sleep(Duration::from_millis(config.latency_ms + jitter)).await;

    // Simulate Error
    if config.error_rate > 0.0 && rand::thread_rng().gen_bool(config.error_rate) {
        return (axum::http::StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "simulated failure"})));
    }

    (axum::http::StatusCode::OK, Json(serde_json::json!({
        "id": "mock-response",
        "object": "chat.completion",
        "created": 1677652288,
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": "Hello! This is a mock response from the provider."
            },
            "finish_reason": "stop"
        }],
        "usage": {
            "prompt_tokens": 10,
            "completion_tokens": 10,
            "total_tokens": 20
        }
    })))
}
