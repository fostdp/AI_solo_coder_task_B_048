use std::time::Duration;
use chrono::Utc;
use clap::Parser;
use rand::{rngs::StdRng, SeedableRng, Rng};
use reqwest::Client;
use serde::Serialize;
use tracing::{info, warn, error};
use tracing_subscriber::{EnvFilter, fmt};

#[derive(Parser, Debug)]
#[command(name = "LoRa Simulator", version, about = "古代战地医院遗址腐蚀监测系统 LoRa 数据模拟器")]
struct Args {
    #[arg(long, env = "SIM_ENDPOINT", default_value = "http://localhost:8080/api/lora/data")]
    endpoint: String,

    #[arg(long, env = "SIM_INTERVAL_MINUTES", default_value_t = 30)]
    interval_minutes: u64,

    #[arg(long, env = "SIM_SOIL_SENSORS", default_value_t = 40)]
    soil_sensors: usize,

    #[arg(long, env = "SIM_CORROSION_PROBES", default_value_t = 20)]
    corrosion_probes: usize,

    #[arg(long, env = "SIM_EDS_PROBES", default_value_t = 15)]
    eds_probes: usize,

    #[arg(long, env = "SIM_MICROBIOME_SAMPLES", default_value_t = 10)]
    microbiome_samples: usize,

    #[arg(long, env = "SIM_GROUNDWATER_WELLS", default_value_t = 8)]
    groundwater_wells: usize,

    #[arg(long, env = "SIM_BURST_MODE", default_value_t = false)]
    burst_mode: bool,

    #[arg(long, env = "SIM_RANDOM_SEED", default_value_t = 42)]
    random_seed: u64,

    #[arg(long, env = "SIM_CHLORIDE_SPIKE_ENABLED", default_value_t = false)]
    chloride_spike_enabled: bool,

    #[arg(long, env = "SIM_CHLORIDE_SPIKE_ZONE", default_value = "区域1-主营区")]
    chloride_spike_zone: String,

    #[arg(long, env = "SIM_CHLORIDE_SPIKE_VALUE", default_value_t = 300.0)]
    chloride_spike_value: f64,

    #[arg(long, env = "SIM_CHLORIDE_SPIKE_DURATION_HOURS", default_value_t = 6)]
    chloride_spike_duration_hours: u64,

    #[arg(long, env = "SIM_CHLORIDE_SPIKE_INTERVAL_HOURS", default_value_t = 48)]
    chloride_spike_interval_hours: u64,
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
    EDS(EDSReading),
    Microbiome(MicrobiomeReading),
    Groundwater(GroundwaterReading),
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

#[derive(Serialize)]
struct EDSReading {
    artifact_id: String,
    alloy_type: String,
    fe_pct: f64,
    cu_pct: f64,
    sn_pct: f64,
    pb_pct: f64,
    zn_pct: f64,
    au_pct: f64,
    ag_pct: f64,
    c_pct: f64,
    o_pct: f64,
    p_pct: f64,
    s_pct: f64,
    cl_pct: f64,
    heterogeneity: f64,
}

#[derive(Serialize)]
struct MicrobiomeReading {
    sample_id: String,
    shannon_index: f64,
    biomass: f64,
    acid_generating: f64,
    sulfur_oxidizing: f64,
    iron_reducing: f64,
    iron_oxidizing: f64,
    eps_producing: f64,
    antibiotic_resistance: f64,
    other_functional: f64,
    evenness: f64,
    toc: f64,
}

#[derive(Serialize)]
struct GroundwaterReading {
    well_id: String,
    well_type: String,
    water_level: f64,
    hydraulic_head: f64,
    chloride_ppm: f64,
    temperature: f64,
    ph: f64,
    conductivity: f64,
    flow_velocity_x: f64,
    flow_velocity_y: f64,
}

const ZONE_NAMES: &[&str] = &[
    "区域1-主营区",
    "区域2-东翼",
    "区域3-西翼",
    "区域4-南侧",
    "区域5-北侧",
];

fn zone_name(idx: usize) -> String {
    ZONE_NAMES[idx % ZONE_NAMES.len()].to_string()
}

fn zone_for_soil(idx: usize) -> String {
    zone_name((idx / 8) % 5)
}

fn zone_for_corrosion(idx: usize) -> String {
    zone_name((idx / 5) % 5)
}

fn zone_for_eds(idx: usize) -> String {
    zone_name((idx / 4) % 5)
}

fn zone_for_microbiome(idx: usize) -> String {
    zone_name((idx / 3) % 5)
}

fn zone_for_groundwater(idx: usize) -> String {
    zone_name((idx / 2) % 5)
}

fn zone_index_for_soil(idx: usize) -> usize {
    (idx / 8) % 5
}

fn zone_index_for_eds(idx: usize) -> usize {
    (idx / 4) % 5
}

fn zone_index_for_microbiome(idx: usize) -> usize {
    (idx / 3) % 5
}

fn zone_index_for_groundwater(idx: usize) -> usize {
    (idx / 2) % 5
}

fn get_adjacent_zones(target_zone: &str) -> Vec<usize> {
    let target_idx = ZONE_NAMES.iter().position(|z| *z == target_zone).unwrap_or(0);
    let mut adjacent = Vec::new();
    if target_idx > 0 {
        adjacent.push(target_idx - 1);
    }
    if target_idx < ZONE_NAMES.len() - 1 {
        adjacent.push(target_idx + 1);
    }
    adjacent
}

fn is_spike_active(elapsed_hours: f64, interval_hours: u64, duration_hours: u64) -> bool {
    let cycle = elapsed_hours % interval_hours as f64;
    cycle < duration_hours as f64
}

fn generate_soil_data(sensor_idx: usize, rng: &mut StdRng, spike_enabled: bool, spike_zone_idx: usize, spike_value: f64, spike_active: bool) -> SoilReading {
    let zone_idx = zone_index_for_soil(sensor_idx);
    let zone = zone_idx;
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

    let mut chloride = f64::max(cl_base + rng.gen_range(-15.0_f64..30.0_f64), 5.0);

    if spike_enabled && spike_active {
        let adjacent_zones = get_adjacent_zones(&ZONE_NAMES[spike_zone_idx]);
        if zone_idx == spike_zone_idx {
            chloride = spike_value;
        } else if adjacent_zones.contains(&zone_idx) {
            chloride = (cl_base + spike_value) / 2.0;
        }
    }

    SoilReading {
        temperature: temp_base + rng.gen_range(-3.0..3.0),
        humidity: hum_base + rng.gen_range(-10.0..10.0),
        ph: 6.8 + rng.gen_range(-1.2..1.2),
        chloride,
    }
}

fn generate_corrosion_data(probe_idx: usize, rng: &mut StdRng) -> CorrosionReading {
    let material_type = if probe_idx % 2 == 0 { "iron".to_string() } else { "copper".to_string() };

    let zone = (probe_idx / 5) % 5;
    let (rp_base, r_base) = match (zone, material_type.as_str()) {
        (2, "iron") => (45.0, 120.0),
        (4, "iron") => (35.0, 100.0),
        (_, "iron") => (80.0, 180.0),
        (2, "copper") => (120.0, 250.0),
        (4, "copper") => (90.0, 200.0),
        (_, "copper") => (180.0, 350.0),
        _ => (80.0, 180.0),
    };

    CorrosionReading {
        material_type,
        resistance: r_base + rng.gen_range(-20.0..20.0),
        polarization_resistance: f64::max(rp_base + rng.gen_range(-15.0_f64..25.0_f64), 20.0),
    }
}

const ALLOY_TYPES: &[&str] = &["iron", "red_copper", "bronze", "brass", "silver", "gold", "other"];

fn generate_eds_data(probe_idx: usize, rng: &mut StdRng) -> EDSReading {
    let artifact_id = format!("ART-{:04}", probe_idx + 1);
    let alloy_idx = probe_idx % ALLOY_TYPES.len();
    let alloy_type = ALLOY_TYPES[alloy_idx].to_string();
    let zone = zone_index_for_eds(probe_idx);

    let (fe, cu, sn, pb, zn, au, ag) = match alloy_type.as_str() {
        "iron" => (85.0 + rng.gen_range(-5.0..5.0), 1.0, 0.5, 0.3, 0.2, 0.0, 0.0),
        "red_copper" => (0.5, 95.0 + rng.gen_range(-3.0..3.0), 0.8, 0.5, 0.3, 0.0, 1.0),
        "bronze" => (0.3, 80.0 + rng.gen_range(-3.0..3.0), 12.0 + rng.gen_range(-2.0..2.0), 3.0, 0.5, 0.0, 0.5),
        "brass" => (0.2, 70.0 + rng.gen_range(-3.0..3.0), 1.0, 1.5, 25.0 + rng.gen_range(-2.0..2.0), 0.0, 0.3),
        "silver" => (0.1, 2.0, 0.5, 0.3, 0.2, 0.5, 92.0 + rng.gen_range(-2.0..2.0)),
        "gold" => (0.0, 1.0, 0.3, 0.2, 0.1, 75.0 + rng.gen_range(-5.0..5.0), 10.0),
        _ => (5.0, 5.0, 2.0, 2.0, 1.0, 0.5, 1.0),
    };

    let zone_factor = match zone {
        0 => 1.0,
        1 => 1.2,
        2 => 1.5,
        3 => 1.1,
        _ => 1.8,
    };

    EDSReading {
        artifact_id,
        alloy_type,
        fe_pct: f64::max(fe, 0.0),
        cu_pct: f64::max(cu, 0.0),
        sn_pct: f64::max(sn, 0.0),
        pb_pct: f64::max(pb, 0.0),
        zn_pct: f64::max(zn, 0.0),
        au_pct: f64::max(au, 0.0),
        ag_pct: f64::max(ag, 0.0),
        c_pct: 2.0 + rng.gen_range(-0.5..0.5),
        o_pct: 3.0 + rng.gen_range(-1.0..1.0) * zone_factor,
        p_pct: 0.3 + rng.gen_range(0.0..0.2) * zone_factor,
        s_pct: 0.5 + rng.gen_range(0.0..0.5) * zone_factor,
        cl_pct: 0.2 + rng.gen_range(0.0..0.8) * zone_factor,
        heterogeneity: 0.1 + rng.gen_range(0.0..0.15) + if zone == 2 || zone == 4 { 0.1 } else { 0.0 },
    }
}

fn generate_microbiome_data(sample_idx: usize, rng: &mut StdRng) -> MicrobiomeReading {
    let sample_id = format!("MICRO-{:03}", sample_idx + 1);
    let zone = zone_index_for_microbiome(sample_idx);

    let base_risk = match zone {
        0 => 0.3,
        1 => 0.45,
        2 => 0.7,
        3 => 0.35,
        _ => 0.8,
    };

    let shannon = 2.5 + rng.gen_range(-0.8..0.8) + (1.0 - base_risk) * 1.5;
    let biomass = 1e6 * (0.5 + base_risk + rng.gen_range(-0.2..0.2));
    let acid = 0.1 + base_risk * 0.6 + rng.gen_range(-0.05..0.05);
    let sulfur = 0.05 + base_risk * 0.4 + rng.gen_range(-0.03..0.03);
    let iron_red = 0.08 + base_risk * 0.5 + rng.gen_range(-0.03..0.03);
    let iron_ox = 0.03 + base_risk * 0.2 + rng.gen_range(-0.02..0.02);
    let eps = 0.1 + base_risk * 0.35 + rng.gen_range(-0.03..0.03);
    let resistance = 0.02 + base_risk * 0.15 + rng.gen_range(-0.01..0.01);
    let other = 1.0 - (acid + sulfur + iron_red + iron_ox + eps + resistance);

    MicrobiomeReading {
        sample_id,
        shannon_index: shannon.clamp(0.5, 5.0),
        biomass: biomass.max(1e4),
        acid_generating: acid.clamp(0.0, 1.0),
        sulfur_oxidizing: sulfur.clamp(0.0, 1.0),
        iron_reducing: iron_red.clamp(0.0, 1.0),
        iron_oxidizing: iron_ox.clamp(0.0, 1.0),
        eps_producing: eps.clamp(0.0, 1.0),
        antibiotic_resistance: resistance.clamp(0.0, 1.0),
        other_functional: other.max(0.0),
        evenness: 0.4 + rng.gen_range(-0.2..0.2) + (1.0 - base_risk) * 0.3,
        toc: 0.5 + base_risk * 2.0 + rng.gen_range(-0.2..0.2),
    }
}

const WELL_TYPES: &[&str] = &["recharge", "pumping", "contamination", "monitoring", "monitoring", "monitoring"];

fn generate_groundwater_data(well_idx: usize, rng: &mut StdRng, spike_enabled: bool, spike_zone_idx: usize, spike_value: f64, spike_active: bool) -> GroundwaterReading {
    let well_id = format!("GW-{:03}", well_idx + 1);
    let zone_idx = zone_index_for_groundwater(well_idx);
    let well_type = WELL_TYPES[well_idx % WELL_TYPES.len()].to_string();

    let head_base = 12.0 + zone_idx as f64 * 1.5;
    let flow_x_base = -0.5e-5 * (zone_idx as f64 - 2.0);
    let flow_y_base = 0.8e-5;

    let mut chloride = match zone_idx {
        0 => 25.0,
        1 => 40.0,
        2 => 70.0,
        3 => 30.0,
        _ => 100.0,
    } + rng.gen_range(-10.0..20.0);

    if spike_enabled && spike_active {
        let adjacent_zones = get_adjacent_zones(&ZONE_NAMES[spike_zone_idx]);
        if zone_idx == spike_zone_idx {
            chloride = spike_value;
        } else if adjacent_zones.contains(&zone_idx) {
            chloride = (chloride + spike_value) / 2.0;
        }
    }

    GroundwaterReading {
        well_id,
        well_type,
        water_level: head_base + rng.gen_range(-0.5..0.5),
        hydraulic_head: head_base + rng.gen_range(-0.3..0.3),
        chloride_ppm: chloride.max(5.0),
        temperature: 14.0 + zone_idx as f64 * 0.5 + rng.gen_range(-1.0..1.0),
        ph: 7.0 + rng.gen_range(-0.8..0.8),
        conductivity: 400.0 + chloride * 3.0 + rng.gen_range(-50.0..50.0),
        flow_velocity_x: flow_x_base + rng.gen_range(-0.2e-5..0.2e-5),
        flow_velocity_y: flow_y_base + rng.gen_range(-0.1e-5..0.1e-5),
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

    info!("LoRa 数据模拟器启动");
    info!("目标端点: {}", args.endpoint);
    info!("土壤传感器: {} 台", args.soil_sensors);
    info!("腐蚀探头: {} 台", args.corrosion_probes);
    info!("EDS能谱探头: {} 台", args.eds_probes);
    info!("微生物采样点: {} 个", args.microbiome_samples);
    info!("地下水监测井: {} 口", args.groundwater_wells);
    info!("随机种子: {}", args.random_seed);
    if args.burst_mode {
        info!("模式: 突发模式 (一次性发送全部数据)");
    } else {
        info!("模式: 定时模式 (每 {} 分钟)", args.interval_minutes);
    }

    if args.chloride_spike_enabled {
        info!("氯化物尖峰注入: 已启用");
        info!("  目标区域: {}", args.chloride_spike_zone);
        info!("  尖峰浓度: {:.1} ppm", args.chloride_spike_value);
        info!("  持续时间: {} 小时", args.chloride_spike_duration_hours);
        info!("  间隔周期: {} 小时", args.chloride_spike_interval_hours);
    } else {
        info!("氯化物尖峰注入: 已禁用");
    }

    let spike_zone_idx = ZONE_NAMES.iter().position(|z| *z == args.chloride_spike_zone).unwrap_or(0);

    let mut rng = StdRng::seed_from_u64(args.random_seed);
    let client = Client::new();
    let mut soil_seq: std::collections::HashMap<String, u64> = std::collections::HashMap::new();
    let mut corrosion_seq: std::collections::HashMap<String, u64> = std::collections::HashMap::new();
    let mut eds_seq: std::collections::HashMap<String, u64> = std::collections::HashMap::new();
    let mut microbiome_seq: std::collections::HashMap<String, u64> = std::collections::HashMap::new();
    let mut groundwater_seq: std::collections::HashMap<String, u64> = std::collections::HashMap::new();
    let mut cycle_count: u64 = 0;
    let mut elapsed_hours: f64 = 0.0;
    let mut prev_spike_active = false;

    loop {
        cycle_count += 1;
        let spike_active = if args.chloride_spike_enabled {
            is_spike_active(elapsed_hours, args.chloride_spike_interval_hours, args.chloride_spike_duration_hours)
        } else {
            false
        };

        if args.chloride_spike_enabled {
            if spike_active && !prev_spike_active {
                info!("===== 氯化物尖峰事件开始 =====");
                info!("目标区域: {}, 浓度: {:.1} ppm", args.chloride_spike_zone, args.chloride_spike_value);
                let adjacent = get_adjacent_zones(&args.chloride_spike_zone);
                info!("相邻区域 (50% 效果): {:?}", adjacent.iter().map(|i| ZONE_NAMES[*i]).collect::<Vec<_>>());
            } else if !spike_active && prev_spike_active {
                info!("===== 氯化物尖峰事件结束 =====");
            }
        }

        info!("===== 第 {} 轮数据上报 =====", cycle_count);
        info!("已运行时长: {:.2} 小时", elapsed_hours);
        info!("氯化物尖峰状态: {}", if spike_active { "激活中" } else { "未激活" });

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
                data: LoraData::Soil(generate_soil_data(i, &mut rng, args.chloride_spike_enabled, spike_zone_idx, args.chloride_spike_value, spike_active)),
            };

            if send_packet(&client, &args.endpoint, &packet).await {
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
                data: LoraData::Corrosion(generate_corrosion_data(i, &mut rng)),
            };

            if send_packet(&client, &args.endpoint, &packet).await {
                success_count += 1;
            } else {
                fail_count += 1;
            }

            if !args.burst_mode {
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        }

        for i in 0..args.eds_probes {
            let device_id = format!("EDS-{:03}", i + 1);
            let seq = eds_seq.entry(device_id.clone()).and_modify(|s| *s += 1).or_insert(1);
            let packet = LoraPacket {
                device_type: "eds_probe".to_string(),
                device_id: device_id.clone(),
                zone: zone_for_eds(i),
                seq_id: *seq,
                timestamp: Utc::now().to_rfc3339(),
                data: LoraData::EDS(generate_eds_data(i, &mut rng)),
            };

            if send_packet(&client, &args.endpoint, &packet).await {
                success_count += 1;
            } else {
                fail_count += 1;
            }

            if !args.burst_mode {
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        }

        for i in 0..args.microbiome_samples {
            let device_id = format!("MICRO-{:03}", i + 1);
            let seq = microbiome_seq.entry(device_id.clone()).and_modify(|s| *s += 1).or_insert(1);
            let packet = LoraPacket {
                device_type: "microbiome_sampler".to_string(),
                device_id: device_id.clone(),
                zone: zone_for_microbiome(i),
                seq_id: *seq,
                timestamp: Utc::now().to_rfc3339(),
                data: LoraData::Microbiome(generate_microbiome_data(i, &mut rng)),
            };

            if send_packet(&client, &args.endpoint, &packet).await {
                success_count += 1;
            } else {
                fail_count += 1;
            }

            if !args.burst_mode {
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        }

        for i in 0..args.groundwater_wells {
            let device_id = format!("GW-{:03}", i + 1);
            let seq = groundwater_seq.entry(device_id.clone()).and_modify(|s| *s += 1).or_insert(1);
            let packet = LoraPacket {
                device_type: "groundwater_sensor".to_string(),
                device_id: device_id.clone(),
                zone: zone_for_groundwater(i),
                seq_id: *seq,
                timestamp: Utc::now().to_rfc3339(),
                data: LoraData::Groundwater(generate_groundwater_data(i, &mut rng, args.chloride_spike_enabled, spike_zone_idx, args.chloride_spike_value, spike_active)),
            };

            if send_packet(&client, &args.endpoint, &packet).await {
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

        prev_spike_active = spike_active;
        elapsed_hours += args.interval_minutes as f64 / 60.0;

        info!("等待 {} 分钟后进行下一轮上报...", args.interval_minutes);
        tokio::time::sleep(Duration::from_secs(args.interval_minutes * 60)).await;
    }
}
