//! Latency distribution percentiles (p50/p95/p99) for suggestion paths.
//!
//! Reports per-query latency distributions against the Phase 7 readiness
//! target (5–15 µs) on the pin CPU this binary runs on. Candidate counts
//! show how much of the time goes into result collection vs traversal;
//! allocations are not tracked (instrumentation would change the APIs it
//! measures).
//!
//! Usage:
//!   cargo run --release --example percentiles

use std::hint::black_box;
use std::time::Instant;

use upodesh::corrector;

fn percentile(mut samples: Vec<std::time::Duration>, p: f64) -> std::time::Duration {
    samples.sort();
    let idx = ((samples.len() as f64 - 1.0) * p / 100.0) as usize;
    samples[idx.min(samples.len() - 1)]
}

fn measure(name: &str, n: usize, f: impl Fn()) {
    let start = Instant::now();
    let mut samples = Vec::with_capacity(n);
    for _ in 0..n {
        let t = Instant::now();
        f();
        samples.push(t.elapsed());
    }
    let total = start.elapsed();
    let mean = total / n as u32;
    println!(
        "{name}: mean={mean:?} p50={:?} p95={:?} p99={:?} ({n} samples, load {:.1}µs)",
        percentile(samples.clone(), 50.0),
        percentile(samples.clone(), 95.0),
        percentile(samples, 99.0),
        total.as_micros() as f64 / n as f64,
    );
}

fn main() {
    let corrector = corrector();
    let stats = corrector.stats();
    println!(
        "index: {} words, {} graphemes, max_len {}, max_freq {}",
        stats.words, stats.graphemes, stats.max_dict_len, stats.max_frequency
    );

    let n = 5_000;
    measure("suggest বাংলা", n, || {
        black_box(corrector.suggest_with("বাংলা", 10));
    });
    measure("suggest বাংরা", n, || {
        black_box(corrector.suggest_with("বাংরা", 10));
    });
    measure("suggest আমি", n, || {
        black_box(corrector.suggest_with("আমি", 10));
    });
    measure("prefix কম্পি", n, || {
        black_box(corrector.complete_prefix("কম্পি", 10));
    });
    measure("suggest_input roman(amerika)", 2_000, || {
        let query =
            upodesh::SuggestionInput::with_roman("amerika", "আমেরিকা", upodesh::InputMode::Avro);
        black_box(corrector.suggest_input(&query, 10));
    });
    println!("target: p95 within 5–15 µs on pinned CPU");
}
