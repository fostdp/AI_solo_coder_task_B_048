use actix_web::{web, HttpResponse, Responder};
use chrono::Utc;
use crate::alarm::AlarmService;
use crate::corrosion_algorithm::{calculate_corrosion_rate_lpr, CorrosionPredictor, StabilityAnalyzer};
use crate::error::AppError;
use crate::influxdb_store::InfluxDBStore;
use crate::lora_gateway::LoraGateway;
use crate::models::{
    ApiResponse, CorrosionData, CorrosionPrediction, CorrosionTrendPoint, HeatmapPoint,
    LoraPacket, ProbeLocation, SoilData, StabilityAssessment, generate_device_locations,
};
use std::sync::Arc;

pub struct AppState {
    pub store: InfluxDBStore,
    pub alarm: AlarmService,
    pub predictor: CorrosionPredictor,
    pub gateway: LoraGateway,
    pub locations: Vec<ProbeLocation>,
}

impl AppState {
    pub fn new(store: InfluxDBStore, alarm: AlarmService) -> Self {
        AppState {
            store,
            alarm,
            predictor: CorrosionPredictor::new(),
            gateway: LoraGateway::new(),
            locations: generate_device_locations(),
        }
    }
}

pub async fn health_check() -> impl Responder {
    HttpResponse::Ok().json(ApiResponse::<()>::ok(()))
}

pub async fn get_locations(
    data: web::Data<Arc<AppState>>,
) -> Result<HttpResponse, AppError> {
    Ok(HttpResponse::Ok().json(ApiResponse::ok(data.locations.clone())))
}

async fn process_packet(
    data: &web::Data<Arc<AppState>>,
    packet: &LoraPacket,
) -> Result<(), AppError> {
    match &packet.data {
        crate::models::LoraData::Soil(soil) => {
            let soil_data = SoilData {
                sensor_id: packet.device_id.clone(),
                zone: packet.zone.clone(),
                sensor_type: "multi".to_string(),
                temperature: soil.temperature,
                humidity: soil.humidity,
                ph: soil.ph,
                chloride: soil.chloride,
                timestamp: Some(packet.timestamp),
            };

            data.store.write_soil_data(&soil_data).await?;
            data.alarm
                .check_and_alert_chloride(&packet.device_id, &packet.zone, soil.chloride)
                .await?;

            tracing::info!(
                "处理土壤数据 seq={}: {} | T:{:.2}C H:{:.1}% pH:{:.2} Cl:{:.1}ppm",
                packet.seq_id, packet.device_id, soil.temperature, soil.humidity, soil.ph, soil.chloride
            );
        }
        crate::models::LoraData::Corrosion(corr) => {
            let corrosion_rate = calculate_corrosion_rate_lpr(corr.polarization_resistance, &corr.material_type);
            let corr_data = CorrosionData {
                probe_id: packet.device_id.clone(),
                zone: packet.zone.clone(),
                material_type: corr.material_type.clone(),
                resistance: corr.resistance,
                polarization_resistance: corr.polarization_resistance,
                corrosion_rate,
                timestamp: Some(packet.timestamp),
            };

            data.store.write_corrosion_data(&corr_data).await?;
            data.alarm
                .check_and_alert_corrosion(&packet.device_id, &packet.zone, &corr.material_type, corrosion_rate)
                .await?;

            tracing::info!(
                "处理腐蚀数据 seq={}: {} | R:{:.2}Ω Rp:{:.2}Ω 速率:{:.4}mm/年",
                packet.seq_id, packet.device_id, corr.resistance, corr.polarization_resistance, corrosion_rate
            );
        }
    }
    Ok(())
}

pub async fn receive_lora_data(
    data: web::Data<Arc<AppState>>,
    packet: web::Json<LoraPacket>,
) -> Result<HttpResponse, AppError> {
    let packet = packet.into_inner();
    let device_id = packet.device_id.clone();
    let seq_id = packet.seq_id;

    let ordered_packets = data.gateway.receive_packet(packet).await?;

    if ordered_packets.is_empty() {
        tracing::debug!(
            "设备 {} seq={} 已缓存等待重排序",
            device_id, seq_id
        );
        return Ok(HttpResponse::Ok().json(ApiResponse::<()>::ok(())));
    }

    for pkt in ordered_packets {
        process_packet(&data, &pkt).await?;
    }

    Ok(HttpResponse::Ok().json(ApiResponse::<()>::ok(())))
}

pub async fn get_gateway_stats(
    data: web::Data<Arc<AppState>>,
) -> Result<HttpResponse, AppError> {
    let stats = data.gateway.get_stats().await;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(stats)))
}

pub async fn get_corrosion_trend(
    data: web::Data<Arc<AppState>>,
    path: web::Path<String>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> Result<HttpResponse, AppError> {
    let probe_id = path.into_inner();
    let hours: i64 = query.get("hours").and_then(|s| s.parse().ok()).unwrap_or(168);

    let mut points = data.store.query_corrosion_trend(&probe_id, hours).await?;

    if points.is_empty() {
        points = generate_mock_trend(hours);
    }

    Ok(HttpResponse::Ok().json(ApiResponse::ok(points)))
}

pub async fn get_heatmap(
    data: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> Result<HttpResponse, AppError> {
    let hours: i64 = query.get("hours").and_then(|s| s.parse().ok()).unwrap_or(24);

    let mut points = data
        .store
        .query_heatmap_data(&data.locations, hours)
        .await?;

    if points.is_empty() {
        points = generate_mock_heatmap(&data.locations);
    }

    Ok(HttpResponse::Ok().json(ApiResponse::ok(points)))
}

pub async fn get_prediction(
    data: web::Data<Arc<AppState>>,
    path: web::Path<String>,
) -> Result<HttpResponse, AppError> {
    let probe_id = path.into_inner();

    let loc = data
        .locations
        .iter()
        .find(|l| l.id == probe_id)
        .ok_or_else(|| AppError::NotFound(format!("探头 {} 不存在", probe_id)))?;

    let material_type = loc.material_type.clone().unwrap_or_else(|| "iron".to_string());

    let current_rate = data
        .store
        .query_latest_corrosion_rate(&probe_id)
        .await?
        .unwrap_or_else(|| if material_type == "iron" { 0.32 } else { 0.18 });

    let (temp, hum, ph, cl) = data
        .store
        .query_zone_avg_environment(&loc.zone, 24)
        .await
        .unwrap_or((16.5, 55.0, 7.2, 45.0));

    let prediction = data.predictor.predict(
        &probe_id,
        &material_type,
        current_rate,
        temp,
        hum,
        ph,
        cl,
    );

    Ok(HttpResponse::Ok().json(ApiResponse::ok(prediction)))
}

pub async fn get_stability(
    data: web::Data<Arc<AppState>>,
    path: web::Path<String>,
) -> Result<HttpResponse, AppError> {
    let probe_id = path.into_inner();

    let loc = data
        .locations
        .iter()
        .find(|l| l.id == probe_id)
        .ok_or_else(|| AppError::NotFound(format!("探头 {} 不存在", probe_id)))?;

    let material_type = loc.material_type.clone().unwrap_or_else(|| "iron".to_string());

    let current_rate = data
        .store
        .query_latest_corrosion_rate(&probe_id)
        .await?
        .unwrap_or_else(|| if material_type == "iron" { 0.32 } else { 0.18 });

    let (temp, hum, ph, cl) = data
        .store
        .query_zone_avg_environment(&loc.zone, 24)
        .await
        .unwrap_or((16.5, 55.0, 7.2, 45.0));

    let assessment = StabilityAnalyzer::assess(
        &probe_id,
        &material_type,
        current_rate,
        temp,
        hum,
        ph,
        cl,
    );

    Ok(HttpResponse::Ok().json(ApiResponse::ok(assessment)))
}

pub async fn get_stats(data: web::Data<Arc<AppState>>) -> Result<HttpResponse, AppError> {
    let mut soil_count = 0;
    let mut corrosion_count = 0;
    let mut high_risk_count = 0;
    let mut avg_corrosion = 0.0;

    for loc in &data.locations {
        if loc.device_type == "soil_sensor" {
            soil_count += 1;
        } else if loc.device_type == "corrosion_probe" {
            corrosion_count += 1;
            if let Ok(Some(rate)) = data.store.query_latest_corrosion_rate(&loc.id).await {
                avg_corrosion += rate;
                if rate > 0.5 {
                    high_risk_count += 1;
                }
            } else {
                avg_corrosion += 0.3;
                if rand::random::<f64>() > 0.7 {
                    high_risk_count += 1;
                }
            }
        }
    }

    if corrosion_count > 0 {
        avg_corrosion /= corrosion_count as f64;
    }

    let stats = serde_json::json!({
        "total_devices": soil_count + corrosion_count,
        "soil_sensors": soil_count,
        "corrosion_probes": corrosion_count,
        "high_risk_probes": high_risk_count,
        "avg_corrosion_rate": format!("{:.4}", avg_corrosion),
        "last_update": Utc::now().to_rfc3339(),
        "site_area": "2000㎡",
        "dynasty": "宋代",
    });

    Ok(HttpResponse::Ok().json(ApiResponse::ok(stats)))
}

fn generate_mock_trend(hours: i64) -> Vec<CorrosionTrendPoint> {
    let mut points = Vec::new();
    let now = Utc::now();
    let n = (hours).min(720) as usize;

    for i in 0..n {
        use chrono::Duration;
        let ts = now - Duration::hours((n - i) as i64);
        let base = 0.25 + (i as f64 * 0.0003);
        let noise: f64 = rand::random::<f64>() * 0.08 - 0.04;
        points.push(CorrosionTrendPoint {
            timestamp: ts,
            corrosion_rate: (base + noise).max(0.05),
            avg_temperature: Some(15.0 + (i as f64 % 24.0 - 12.0).abs() * 0.4),
            avg_humidity: Some(50.0 + rand::random::<f64>() * 20.0),
            avg_chloride: Some(40.0 + rand::random::<f64>() * 30.0),
        });
    }

    points
}

fn generate_mock_heatmap(locations: &[ProbeLocation]) -> Vec<HeatmapPoint> {
    locations
        .iter()
        .filter(|l| l.device_type == "corrosion_probe")
        .map(|l| {
            let base_intensity: f64 = if l.material_type.as_deref() == Some("iron") { 0.5 } else { 0.3 };
            let noise: f64 = rand::random::<f64>() * 0.4;
            HeatmapPoint {
                lat: l.lat,
                lng: l.lng,
                intensity: (base_intensity + noise).min(1.0).max(0.0),
                probe_id: l.id.clone(),
                zone: l.zone.clone(),
            }
        })
        .collect()
}
