use criterion::{criterion_group, criterion_main, Criterion};

// --- Benchmark Target 1: classify_temp ---
fn classify_temp(temp: f32) -> String {
    match temp {
        t if t <= 20.0 => "Cold".to_string(),
        t if t <= 30.0 => "Normal".to_string(),
        _ => "Hot".to_string(),
    }
}

fn bench_classify_temp(c: &mut Criterion) {
    c.bench_function("classify_temp_hot", |b| b.iter(|| classify_temp(55.0)));
}

// --- Benchmark Target 2: moving_average ---
fn moving_average(data: &[f32], window_size: usize) -> f32 {
    let len = data.len().min(window_size);
    data.iter().rev().take(len).sum::<f32>() / len as f32
}

fn bench_moving_average(c: &mut Criterion) {
    let data = vec![22.0, 24.0, 26.0, 28.0, 30.0];
    c.bench_function("moving_average_5", |b| {
        b.iter(|| moving_average(&data, 5))
    });
}

// --- Benchmark Target 3: detect_anomaly ---
fn detect_anomaly(weight: f32, temp: f32) -> Option<String> {
    if weight < 485.0 {
        Some("Low weight anomaly".to_string())
    } else if temp > 50.0 {
        Some("High temperature anomaly".to_string())
    } else {
        None
    }
}

fn bench_detect_anomaly(c: &mut Criterion) {
    c.bench_function("detect_anomaly_high_temp", |b| {
        b.iter(|| detect_anomaly(500.0, 55.0))
    });
}

// --- Benchmark Target 4: simulate_eta ---
fn simulate_eta(distance: f32, speed: f32) -> u64 {
    (distance / speed * 1000.0).max(5.0) as u64
}

fn bench_simulate_eta(c: &mut Criterion) {
    c.bench_function("simulate_eta_0.5m_100mps", |b| {
        b.iter(|| simulate_eta(0.5, 100.0))
    });
}

// --- Benchmark Target 5: generate_cup ---
fn generate_cup(temp: f32, status: &str) -> (f32, String) {
    let weight = match status {
        "Full" => 505.0,
        "Not Full" => 495.0,
        _ => 485.0,
    };
    let temp_category = classify_temp(temp);
    (weight, temp_category)
}

fn bench_generate_cup(c: &mut Criterion) {
    c.bench_function("generate_cup_full", |b| {
        b.iter(|| generate_cup(28.5, "Full"))
    });
}

// Register all benchmarks
criterion_group!(benches,
    bench_classify_temp,
    bench_moving_average,
    bench_detect_anomaly,
    bench_simulate_eta,
    bench_generate_cup
);
criterion_main!(benches);

