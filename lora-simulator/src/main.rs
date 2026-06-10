use std::time::Duration;
use chrono::Utc;
use clap::Parser;
use rand::Rng;
use reqwest::Client;
use serde::Serialize;
use tracing::{info, warn, error};
use tracing_subscriber::{EnvFilter, fmt};

#[derive(Parser, Debug)]
#[command(name = "LoRa Simulator", version, about = "古代战地医院遗址腐蚀监测系统 LoRa 数据模拟器")]
struct Args {
    #[arg(long, default_value = "http://localhost:8080")]
    server_url: String,

    #[arg(long, default_value_t = 30)]
    interval_minutes: u64,

    #[arg(long, default_value_t = 40)]
    soil_sensors: usize,

    #[arg(long, default_value_t = 20)]
    corrosion_probes: usize,

    #[arg(long, default_value_t = false)]
    burst_mode: bool,
}

#[derive(Serialize)]
struct LoraPacket {
    device_type: String,
    device_id: String,
    zone: String,
    seq_id: u64,
    timestamp: String,
    data: LoraData,
}

#[derive(Serialize)]
#[serde(untagged)]
enum LoraData {
    Soil(SoilReading),
    Corrosion(CorrosionReading),
}

#[derive(Serialize)]
struct SoilReading {
    temperature: f64,
    humidity: f64,
    ph: f64,
    chloride: f64,
}

#[derive(Serialize)]
struct CorrosionReading {
    material_type: String,
    resistance: f64,
    polarization_resistance: f64,
}

fn zone_for_soil(idx: usize) -> String {
    format!("区域-{}", ((idx / 8) % 5) + 1)
}

fn zone_for_corrosion(idx: usize) -> String {
    format!("区域-{}", ((idx / 5) % 5) + 1)
}

fn generate_soil_data(sensor_idx: usize) -> SoilReading {
    let mut rng = rand::thread_rng();

    let zone = (sensor_idx / 8) % 5;
    let temp_base = match zone {
        0 => 14.0,
        1 => 16.5,
        2 => 18.0,
        3 => 15.5,
        _ => 20.0,
    };

    let hum_base = match zone {
        0 => 45.0,
        1 => 55.0,
        2 => 65.0,
        3 => 50.0,
        _ => 70.0,
    };

    let cl_base = match zone {
        0 => 35.0,
        1 => 50.0,
        2 => 85.0,
        3 => 40.0,
        _ => 120.0,
    };

    SoilReading {
        temperature: temp_base + rng.gen_range(-3.0..3.0),
        humidity: hum_base + rng.gen_range(-10.0..10.0),
        ph: 6.8 + rng.gen_range(-1.2..1.2),
        chloride: (cl_base + rng.gen_range(-15.0..30.0)).max(5.0),
    }
}

fn generate_corrosion_data(probe_idx: usize) -> CorrosionReading {
    let mut rng = rand::thread_rng();
    let material_type = if probe_idx % 2 == 0 { "iron".to_string() } else { "copper".to_string() };

    let zone = (probe_idx / 5) % 5;
    let (rp_base, r_base) = match (zone, material_type.as_str()) {
        (2, "iron") => (45.0, 120.0),
        (4, "iron") => (35.0, 100.0),
        (_, "iron") => (80.0, 180.0),
        (2, "copper") => (120.0, 250.0),
        (4, "copper") => (90.0, 200.0),
        (_, "copper") => (180.0, 350.0),
    };

    CorrosionReading {
        material_type,
        resistance: r_base + rng.gen_range(-20.0..20.0),
        polarization_resistance: (rp_base + rng.gen_range(-15.0..25.0)).max(20.0),
    }
}

async fn send_packet(client: &Client, url: &str, packet: &LoraPacket) -> bool {
    match client
        .post(url)
        .json(packet)
        .timeout(Duration::from_secs(10))
        .send()
        .await
    {
        Ok(resp) => {
            if resp.status().is_success() {
                true
            } else {
                warn!("发送失败, HTTP状态: {}", resp.status());
                false
            }
        }
        Err(e) => {
            error!("请求错误: {}", e);
            false
        }
    }
}

#[tokio::main]
async fn main() {
    fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .init();

    let args = Args::parse();
    let endpoint = format!("{}/api/lora/data", args.server_url);

    info!("LoRa 数据模拟器启动");
    info!("目标服务器: {}", args.server_url);
    info!("土壤传感器: {} 台", args.soil_sensors);
    info!("腐蚀探头: {} 台", args.corrosion_probes);
    if args.burst_mode {
        info!("模式: 突发模式 (一次性发送全部数据)");
    } else {
        info!("模式: 定时模式 (每 {} 分钟)", args.interval_minutes);
    }

    let client = Client::new();
    let mut soil_seq: std::collections::HashMap<String, u64> = std::collections::HashMap::new();
    let mut corrosion_seq: std::collections::HashMap<String, u64> = std::collections::HashMap::new();

    loop {
        info!("===== 开始新的一轮数据上报 =====");
        let mut success_count = 0;
        let mut fail_count = 0;

        for i in 0..args.soil_sensors {
            let device_id = format!("SOIL-{:03}", i + 1);
            let seq = soil_seq.entry(device_id.clone()).and_modify(|s| *s += 1).or_insert(1);
            let packet = LoraPacket {
                device_type: "soil_sensor".to_string(),
                device_id: device_id.clone(),
                zone: zone_for_soil(i),
                seq_id: *seq,
                timestamp: Utc::now().to_rfc3339(),
                data: LoraData::Soil(generate_soil_data(i)),
            };

            if send_packet(&client, &endpoint, &packet).await {
                success_count += 1;
            } else {
                fail_count += 1;
            }

            if !args.burst_mode {
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        }

        for i in 0..args.corrosion_probes {
            let device_id = format!("CORR-{:03}", i + 1);
            let seq = corrosion_seq.entry(device_id.clone()).and_modify(|s| *s += 1).or_insert(1);
            let packet = LoraPacket {
                device_type: "corrosion_probe".to_string(),
                device_id: device_id.clone(),
                zone: zone_for_corrosion(i),
                seq_id: *seq,
                timestamp: Utc::now().to_rfc3339(),
                data: LoraData::Corrosion(generate_corrosion_data(i)),
            };

            if send_packet(&client, &endpoint, &packet).await {
                success_count += 1;
            } else {
                fail_count += 1;
            }

            if !args.burst_mode {
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        }

        info!(
            "本轮上报完成: 成功 {}, 失败 {}, 总数 {}",
            success_count, fail_count, success_count + fail_count
        );

        if args.burst_mode {
            info!("突发模式完成，退出程序");
            break;
        }

        info!("等待 {} 分钟后进行下一轮上报...", args.interval_minutes);
        tokio::time::sleep(Duration::from_secs(args.interval_minutes * 60)).await;
    }
}
