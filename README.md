# Ultra-Low Latency LLM Gateway

A high-performance, cost-aware Gateway for LLMs written in **Rust**.
Designed to route requests between multiple providers (OpenAI, Anthropic, Local) with **O(1)** decision overhead (< 100 µs), L1 Semantic Cache, and Adaptive Load Balancing.

## 🚀 Key Features

*   **⚡ Ultra-Low Latency**: Internal gateway overhead tracked at **~80-120 µs**.
*   **🧠 Semantic Cache**: In-memory L1 cache using `moka` and `blake3` hashing to serve repeated prompts instantly (O(1)).
*   **🔀 Smart Routing**:
    *   **O(1) Selection**: Pre-computed/Atomic provider selection.
    *   **Cost-Aware**: Routes to the cheapest provider meeting SLA.
    *   **Adaptive**: EWMA-based latency tracking and Circuit Breakers for fault tolerance.
*   **🏗️ Concurrency**: Built on `Tokio` and `Axum` with lock-free statistics updates (`AtomicU64`, `ArcSwap`).

## 🛠️ Architecture

```
Client
  ↓ (HTTP/2)
[ Gateway Service (Axum) ]
  ├─ 🔍 Semantic Cache (L1) -> Hit? return immediately
  ├─ 🧭 Router (O(1))
  │     ├─ Filter Healthy Providers
  │     └─ Score: (Latency * w1) + (Cost * w2)
  ├─ 📡 Provider Client
  │     └─ Circuit Breaker Metric Collection
  └─ 📊 Stats Update (Atomic EWMA)
```

## 📂 Project Structure

*   `src/lib.rs`: Module exports.
*   `src/model.rs`: Core API data types (`LlmRequest`, `LlmResponse`, `ProviderConfig`).
*   `src/cache`: L1 In-memory cache implementation.
*   `src/router`: Routing logic and Provider abstraction.
*   `src/balancer`: Statistical tracking (EWMA, error rates).
*   `src/gateway.rs`: Request handling pipeline.
*   `src/main.rs`: Server entry point.
*   `src/bin/simulator.rs`: **Load Testing Orchestrator**.
*   `src/bin/mock_provider.rs`: Mock LLM API for testing.

## 🏃 Usage

### Prerequisites
*   Rust (1.70+)

### Running the Simulator
The simulator launches two mock providers (different latency/costs) and the gateway, then floods it with traffic.

```bash
cargo build --bins
./target/debug/simulator
```

### Expected Output
```text
Starting Simulation...
...
Total Requests: 100
Success: 100
Errors: 0
Total Time: ~870ms
RPS: ~115
```

### Running the Gateway Standalone
```bash
cargo run --bin llm-edge
```
The server listens on `127.0.0.1:8080`.

## 📊 Benchmarks & Trade-offs

### Performance
During simulation with 100 concurrent requests:
*   **P50 Overhead**: ~90 µs
*   **P99 Overhead**: ~150 µs
*   **Throughput**: Limited only by provider latency (Gateway is non-blocking).

### Trade-offs
1.  **In-Memory Cache**:
    *   *Pro*: Zero network latency (microseconds lookup).
    *   *Con*: Local to the node. Does not share state across a cluster of gateways (would need Redis).
2.  **Simple Scoring**:
    *   *Pro*: Deterministic, O(1), no ML inference overhead.
    *   *Con*: Doesn't predict "spikes" as well as an ML scheduler.
3.  **Atomic/Relaxed Stats**:
    *   *Pro*: Zero lock contention.
    *   *Con*: Metrics might be slightly eventually consistent under extreme contention (acceptable for LB).
