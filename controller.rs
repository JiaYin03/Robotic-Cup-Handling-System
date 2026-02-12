use tokio::time::{sleep, Duration, Instant};
use rand::{random_range};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use tokio::net::UdpSocket;
use serde::{Serialize, Deserialize};
use std::pin::Pin;
use std::future::Future;
use std::time::{SystemTime, UNIX_EPOCH};


#[derive(Debug, Clone)]
struct CupData {
    cup_id: u32,
    start_time: Instant,
    weight: f32,
    temperature: f32,
    full_status: String,
    temp_category: String,
    position_m: f32,
    belt_speed: f32,
}

#[derive(Serialize, Deserialize, Debug)]
struct SensorMessage {
    sensor: String,
    cup_id: u32,
    value: String,
    timestamp: u128,
}

#[derive(Deserialize, Debug)]
struct FeedbackMessage {
    cup_id: u32,
    status: String,
    timestamp: u128,
}


fn classify_temp(temp: f32) -> String {
    match temp {
        t if t <= 20.0 => "Cold".to_string(),
        t if t <= 30.0 => "Normal".to_string(),
        _ => "Hot".to_string(),
    }
}

fn detect_anomaly(weight: f32, temp: f32) -> Option<String> {
    if weight < 485.0 {
        Some("Low weight anomaly".to_string())
    } else if temp > 50.0 {
        Some("High temperature anomaly".to_string())
    } else {
        None
    }
}

fn moving_average(data: &[f32], window_size: usize) -> f32 {
    let len = data.len().min(window_size);
    data.iter().rev().take(len).sum::<f32>() / len as f32
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis()
}


async fn send_to_student_b(socket: &UdpSocket, message: &SensorMessage) {
    let json = serde_json::to_string(&message).unwrap();
    let start = Instant::now();
    let _ = socket.send_to(json.as_bytes(), "127.0.0.1:9001").await;
    let duration = start.elapsed();
    if duration > Duration::from_millis(1) {
        println!("⚠️ Warning: Data transmission exceeded 1ms ({:.2?})", duration);
    }
}

async fn feedback_listener() {
    println!("[DEBUG] Feedback listener launched!");
    let socket = UdpSocket::bind("127.0.0.1:9000").await.unwrap();
    let mut buf = [0u8; 1024];
    loop {
        println!("[DEBUG] Waiting for UDP...");
        let (amt, _src) = socket.recv_from(&mut buf).await.unwrap();
        println!("[DEBUG] Received {} bytes!", amt);
        if let Ok(text) = std::str::from_utf8(&buf[..amt]) {
            println!("[Raw Feedback UDP] {}", text);
            if let Ok(feedback) = serde_json::from_str::<FeedbackMessage>(text) {
                println!("\x1b[95m[Feedback] Cup {}: {} (at {})\x1b[0m",
                    feedback.cup_id, feedback.status, feedback.timestamp);
            }
        }
    }
}

async fn simulate_travel(sensor: &str, cup_id: u32, distance_m: f32, speed_mps: f32) {
    let eta_ms = (distance_m / speed_mps * 1000.0).max(5.0) as u64;
    println!();
    println!(
        "🛣️  Cup {:03} moving to {} — Speed: {:.2} m/s, Distance: {:.2} m, ETA: {} ms",
        cup_id, sensor, speed_mps, distance_m, eta_ms
    );
    sleep(Duration::from_millis(eta_ms)).await;
}


fn sensor_1_allocate<'a>(
    cup_id: u32,
    socket: &'a UdpSocket,
    // window_state: &'a Arc<AtomicBool>,
     temp_history: &'a mut Vec<f32>,
) -> Pin<Box<dyn Future<Output = Option<CupData>> + Send + 'a>> {
    Box::pin(async move {
        // Simulate a random temperature reading
        let raw_temp = random_range(20.0..60.0);
        temp_history.push(raw_temp);

        // Limit to the last 5 readings
        if temp_history.len() > 5 {
            temp_history.remove(0);
        }
        // Calculate moving average temperature
        let mut temperature = moving_average(temp_history, 5);

        let position_m = 0.0;

        // Randomized full status
        let status_roll = rand::random::<f32>();
        let full_status = if status_roll < 0.1 {
            "Empty"
        } else if status_roll < 0.4 {
            "Not Full"
        } else {
            "Full"
        }
        .to_string();

        let weight = match full_status.as_str() {
            "Full" => random_range(500.0..510.0),
            "Not Full" => random_range(490.0..499.9),
            _ => random_range(480.0..489.9),
        };

        if full_status == "Empty" && temperature > 35.0 {
            temperature = random_range(20.0..30.0);
        }

        println!(
              "\n\x1b[96mSensor 1 detected cup {:03}\nID: {:03}\nPosition: {:.1}m (Cup Allocation Area)\nTime since allocation: 0.00s\nWeight: {:.1}g ({})\x1b[0m",
            cup_id, cup_id, position_m, weight, full_status
        );

        // ❌ Skip processing if not full or empty
        if full_status != "Full" {
            println!("\x1b[93m[Notice] Cup {:03} is {}. Needs refill.\x1b[0m", cup_id, full_status);

            let msg = SensorMessage {
                sensor: "Sensor1".to_string(),
                cup_id,
                value: format!("Refill Request — Cup {} is {}", cup_id, full_status),
                timestamp: now_ms(),
            };
            send_to_student_b(socket, &msg).await;

            return None; // Skip to next cup
        }


        let temp_category = classify_temp(temperature);
        println!("\x1b[94mCup Temperature: {:.1}°C — {}\x1b[0m", temperature, temp_category);

        // Check for anomalies
        if let Some(alert) = detect_anomaly(weight, temperature) {
            println!("\x1b[1;91m[Anomaly] Cup {:03}: {}\x1b[0m", cup_id, alert);
        }

        if temperature > 50.0 {
            println!(
                "\x1b[1;91m[Cup Temperature] {:.1}°C — Too hot. Cooling required.\x1b[0m",
                temperature
            );
        }

        let msg = SensorMessage {
            sensor: "Sensor1".to_string(),
            cup_id,
            value: format!("Weight: {:.1}g, Temp: {:.1}°C ({})", weight, temperature, temp_category),
            timestamp: now_ms(),
        };
        send_to_student_b(socket, &msg).await;

        Some(CupData {
            cup_id,
            start_time: Instant::now(),
            weight,
            temperature,
            full_status,
            temp_category: temp_category.clone(),
            position_m,
            belt_speed: 0.0,
        })
    })
}

async fn sensor_2_midpoint(cup: &CupData, socket: &UdpSocket) {
    let time_to_mid = cup.start_time.elapsed().as_millis() as f32;
    println!(
       "Sensor 2: Cup {:03} at midpoint — Full Status: {}, Temp Category: {}, Position: {:.1}m",
    cup.cup_id, cup.full_status, cup.temp_category, cup.position_m
);
    let msg = SensorMessage {
        sensor: "Sensor2".to_string(),
        cup_id: cup.cup_id,
        value: format!("Reached midpoint in {:.2}s", time_to_mid),
        timestamp: now_ms(),
    };
    send_to_student_b(socket, &msg).await;
}

async fn sensor_3_grip(cup: &CupData, socket: &UdpSocket) {
    // let time_now = cup.start_time.elapsed().as_millis() as u128;
    println!(
    "Sensor 3: Cup {:03} is ready to be gripped — Weight: {:.1}g, Temp: {:.1}°C\n",
        cup.cup_id, cup.weight, cup.temperature
    );
    
    let msg = SensorMessage {
        sensor: "Sensor3".to_string(),
        cup_id: cup.cup_id,
        value: "Action: Grip Cup".to_string(),
        timestamp: now_ms(),
    };
    
    send_to_student_b(socket, &msg).await;
}


async fn sensor_4_claw(cup: &CupData, socket: &UdpSocket) {
    // let pickup_time = cup.start_time.elapsed().as_millis() as f32;
    
    println!(
        "Sensor 4: Request to pick up cup {:03} with claw\n",
        cup.cup_id
    );
    
    let msg = SensorMessage {
        sensor: "Sensor4".to_string(),
        cup_id: cup.cup_id,
        value: "Action: Pick Up Cup".to_string(),
        timestamp: now_ms(),
    };
    
    send_to_student_b(socket, &msg).await;

}


async fn sensor_5_arm(cup: &CupData, retry: bool, socket: &UdpSocket) {
    // let rotate_time = cup.start_time.elapsed().as_millis() as f32;
    let direction = if retry { "Accumulator" } else { "Window" };

    println!(
        "Sensor 5: Request rotation of cup {:03} to {}\n",
        cup.cup_id, direction
    );

    let msg = SensorMessage {
        sensor: "Sensor5".to_string(),
        cup_id: cup.cup_id,
        value: format!("Action: Rotate to {}", direction),
        timestamp: now_ms(),
    };

    send_to_student_b(socket, &msg).await;
}


async fn sensor_6_window(
    cup: &CupData,
    window_state: &Arc<AtomicBool>,
    socket: &UdpSocket,
) -> bool {
   

    // Atomically check and lock the window (set it to occupied if it was free)
    let was_free = window_state
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok();

    if was_free {
        println!("Sensor 6: Window is free for cup {:03}", cup.cup_id);

        let msg = SensorMessage {
            sensor: "Sensor6".to_string(),
            cup_id: cup.cup_id,
            value: "Status: Window Free — Placement Success".to_string(),
            timestamp: now_ms(),
        };
        send_to_student_b(socket, &msg).await;
        


        // Hold the window for 1 second
        sleep(Duration::from_millis(1000)).await;

        // Then unlock the window
        window_state.store(false, Ordering::SeqCst);
        println!("[Window] Now free — restoring belt speed to 1.0 m/s");

        true
    } else {
        println!("Sensor 6: Window is occupied. Cup {:03} retrying...\n", cup.cup_id);

        let msg = SensorMessage {
            sensor: "Sensor6".to_string(),
            cup_id: cup.cup_id,
            value: "Status: Window Occupied".to_string(),
            timestamp: now_ms(),
        };
        send_to_student_b(socket, &msg).await;

        false
    }
}

#[tokio::main]
async fn main() {
    let socket = Arc::new(UdpSocket::bind("127.0.0.1:9003").await.unwrap());
    let window_state = Arc::new(AtomicBool::new(false));
    let counter = Arc::new(AtomicU64::new(0));

     tokio::spawn(async {
        feedback_listener().await;
    });

    let mut cup_id = 1;
    let start_time = Instant::now();
    let generated_25 = Arc::new(AtomicU64::new(0));
    let processed_25 = Arc::new(AtomicU64::new(0));
    let generated_45 = Arc::new(AtomicU64::new(0));
    let processed_45 = Arc::new(AtomicU64::new(0));
    let mut temp_history: Vec<f32> = Vec::new();

let mut interval = tokio::time::interval(Duration::from_millis(5));

loop {
    interval.tick().await;

    if start_time.elapsed() > Duration::from_secs(60) {
        break;
    }

    let elapsed = start_time.elapsed().as_secs();
    if elapsed == 25 {
        generated_25.store(cup_id as u64 - 1, Ordering::SeqCst);
        processed_25.store(counter.load(Ordering::SeqCst), Ordering::SeqCst);
    }
    if elapsed == 45 {
        generated_45.store(cup_id as u64 - 1, Ordering::SeqCst);
        processed_45.store(counter.load(Ordering::SeqCst), Ordering::SeqCst);
    }

    if let Some(mut cup) = sensor_1_allocate(cup_id, &socket, &mut temp_history).await {

        cup.belt_speed = if window_state.load(Ordering::SeqCst) {
            println!("\x1b[93m[Window] Occupied — reducing belt speed to 50.0 m/s\x1b[0m");
            50.0
        } else {
            println!("\x1b[92m[Window] Free — maintaining belt speed at 100.0 m/s\x1b[0m");
            100.0
        };

         let t2 = Instant::now();
            simulate_travel("Sensor 2", cup.cup_id, 0.5, cup.belt_speed).await;
            println!("⏱️ Travel to Sensor 2 took {:.2?}", t2.elapsed());
            sensor_2_midpoint(&cup, &socket).await;

            let t3 = Instant::now();
            simulate_travel("Sensor 3", cup.cup_id, 0.5, cup.belt_speed).await;
            println!("⏱️ Travel to Sensor 3 took {:.2?}", t3.elapsed());
            sensor_3_grip(&cup, &socket).await;

            let t4 = Instant::now();
            simulate_travel("Sensor 4", cup.cup_id, 0.3, cup.belt_speed).await;
            println!("⏱️ Travel to Sensor 4 took {:.2?}", t4.elapsed());
            sensor_4_claw(&cup, &socket).await;

            let t5 = Instant::now();
            simulate_travel("Sensor 5", cup.cup_id, 0.2, cup.belt_speed).await;
            println!("⏱️ Travel to Sensor 5 took {:.2?}", t5.elapsed());
            sensor_5_arm(&cup, false, &socket).await;

            loop {
                let t6 = Instant::now();
                simulate_travel("Sensor 6", cup.cup_id, 0.2, cup.belt_speed).await;
                println!("⏱️ Travel to Sensor 6 took {:.2?}", t6.elapsed());

                let success = sensor_6_window(&cup, &window_state, &socket).await;
                if success {
                    break;
                }

                println!("[Retry] Cup {:03} will retry placement...\n", cup.cup_id);

                let tretry = Instant::now();
                simulate_travel("Sensor 5 (Retry)", cup.cup_id, 0.2, cup.belt_speed).await;
                println!("⏱️ Retry Travel to Sensor 5 took {:.2?}", tretry.elapsed());
                sensor_5_arm(&cup, true, &socket).await;
            }

            counter.fetch_add(1, Ordering::SeqCst);
        }

        cup_id += 1;
    }

    let total_processed = counter.load(Ordering::SeqCst);
    let total_generated = cup_id - 1;
    let elapsed_secs = start_time.elapsed().as_secs_f32();
    let avg_speed = (total_processed as f32 / elapsed_secs).clamp(0.0, f32::MAX);
    let avg_time_per_cup = if total_processed > 0 {
        elapsed_secs / total_processed as f32
    } else {
        0.0
    };

    println!("\n================= 📊 FINAL STATISTICS =================");
    println!("🔢 Total Cups Generated     : {}", total_generated);
    println!("✅ Total Cups Processed     : {}", total_processed);
    println!("⚡ Average Processing Speed : {:.2} cups/sec", avg_speed);
    println!("⏱️  Avg Time per Cup        : {:.2} sec", avg_time_per_cup);

    println!("\n📍 At 25s → Generated: {}, Processed: {}",
        generated_25.load(Ordering::SeqCst),
        processed_25.load(Ordering::SeqCst)
    );

    println!("📍 At 45s → Generated: {}, Processed: {}",
        generated_45.load(Ordering::SeqCst),
        processed_45.load(Ordering::SeqCst)
    );

    println!("======================================================\n");
}



