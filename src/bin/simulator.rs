use std::process::{Command, Child};
use std::thread;
use std::time::{Duration, Instant};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::task;

// Helper to kill children on exit
struct ProcessGuard(Child);
impl Drop for ProcessGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
    }
}

#[tokio::main]
async fn main() {
    println!("Starting Simulation...");

    // 1. Start Mock Providers
    // Assumes binaries are already built by a previous `cargo build` or `cargo run`
    let _p1 = ProcessGuard(Command::new("./target/debug/mock_provider")
        .args(&["3001", "50", "0.0"])
        .spawn()
        .expect("Failed to start p1"));
    
    let _p2 = ProcessGuard(Command::new("./target/debug/mock_provider")
        .args(&["3002", "200", "0.2"]) 
        .spawn()
        .expect("Failed to start p2"));

    println!("Providers started (P1: 3001, P2: 3002). Waiting 5s for compilation/startup...");
    thread::sleep(Duration::from_secs(5));

    // 2. Start Gateway
    let _gw = ProcessGuard(Command::new("./target/debug/llm-edge")
        .spawn()
        .expect("Failed to start gateway"));

    println!("Gateway started on 8080. Waiting 5s...");
    thread::sleep(Duration::from_secs(5));

    // 3. Run Load Generator
    println!("Starting Load Test (100 concurrent requests)...");
    
    let client = reqwest::Client::new();
    let counter = Arc::new(AtomicUsize::new(0));
    let errors = Arc::new(AtomicUsize::new(0));
    let start_time = Instant::now();

    let mut tasks = Vec::new();
    for i in 0..100 {
        let client = client.clone();
        let counter = counter.clone();
        let errors = errors.clone();
        
        let prompt = if i % 2 == 0 { "repeat_prompt" } else { "unique_prompt" };
        let prompt_str = format!("{} {}", prompt, i); // Uniqueish to test cache miss? 
        // Actually let's test Cache HIT heavily.
        let prompt_final = if i < 50 { "common_prompt".to_string() } else { format!("unique_{}", i) };

        tasks.push(task::spawn(async move {
            let body = serde_json::json!({
                "model": "gpt-4",
                "prompt": prompt_final,
                "temperature": 0.7
            });

            match client.post("http://localhost:8080/v1/chat/completions")
                .json(&body)
                .send()
                .await {
                Ok(resp) => {
                    if resp.status().is_success() {
                        counter.fetch_add(1, Ordering::Relaxed);
                    } else {
                        errors.fetch_add(1, Ordering::Relaxed);
                        // println!("Err status: {}", resp.status());
                    }
                },
                Err(e) => {
                    errors.fetch_add(1, Ordering::Relaxed);
                    // println!("Req error: {}", e);
                }
            }
        }));
    }

    for t in tasks {
        let _ = t.await;
    }

    let duration = start_time.elapsed();
    let requests = counter.load(Ordering::Relaxed);
    let error_count = errors.load(Ordering::Relaxed);

    println!("--- Results ---");
    println!("Total Requests: 100");
    println!("Success: {}", requests);
    println!("Errors: {}", error_count);
    println!("Total Time: {:?}", duration);
    println!("RPS: {:.2}", 100.0 / duration.as_secs_f64());
    
    println!("Simulation finished. Press Ctrl+C to stop servers (or wait 2s and I'll kill them).");
    thread::sleep(Duration::from_secs(2));
}
