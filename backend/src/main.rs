pub mod common;
pub mod ingress;
pub mod corrosion_engine;
pub mod storage;
pub mod alert_broker;

use actix_cors::Cors;
use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use std::sync::Arc;

use common::*;
use ingress::{LoraGateway, receive_lora_data, get_gateway_stats};
use corrosion_engine::{CorrosionPredictor, StabilityAnalyzer, calculate_corrosion_rate_lpr};
use storage::StorageService;
use alert_broker::AlertService;

pub struct AppState {
    pub config: AppConfig,
    pub store: StorageService,
    pub alarm: AlertService,
    pub predictor: CorrosionPredictor,
    pub gateway: LoraGateway,
    pub locations: Vec<ProbeLocation>,
}

async fn health_check() -> impl Responder {
    HttpResponse::Ok().json(ApiResponse::ok(serde_json::json!({
        "status": "healthy",
        "version": "2.0.0",
    })))
}

async fn get_locations(data: web::Data<Arc<AppState>>) -> impl Responder {
    HttpResponse::Ok().json(ApiResponse::ok(data.locations.clone()))
}

async fn get_stats(data: web::Data<Arc<AppState>>) -> impl Responder {
    let soil_count = data.locations.iter().filter(|l| l.device_type == "soil_sensor").count();
    let corr_count = data.locations.iter().filter(|l| l.device_type == "corrosion_probe").count();
    let zones: std::collections::HashSet<_> = data.locations.iter().map(|l| &l.zone).collect();

    let mut high_risk = 0usize;
    let mut rate_sum = 0.0f64;
    let mut count = 0usize;
    for loc in data.locations.iter().filter(|l| l.device_type == "corrosion_probe") {
        if let Some(rate) = data.store.reader.query_latest_corrosion_rate(&loc.device_id).await {
            if rate > 0.3 {
                high_risk += 1;
            }
            rate_sum += rate;
            count += 1;
        }
    }
    let avg_rate = if count > 0 { rate_sum / count as f64 } else { 0.0 };

    HttpResponse::Ok().json(ApiResponse::ok(SiteStats {
        total_soil_sensors: soil_count,
        total_corrosion_probes: corr_count,
        total_zones: zones.len(),
        high_risk_probes: high_risk,
        avg_corrosion_rate: avg_rate,
        avg_temperature: 15.0,
        avg_humidity: 45.0,
        avg_ph: 7.2,
        avg_chloride: 55.0,
    }))
}

async fn get_corrosion_trend(
    data: web::Data<Arc<AppState>>,
    path: web::Path<String>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> impl Responder {
    let probe_id = path.into_inner();
    let hours: i64 = query.get("hours").and_then(|s| s.parse().ok()).unwrap_or(168);
    let trend = data.store.reader.query_corrosion_trend(&probe_id, hours).await;
    HttpResponse::Ok().json(ApiResponse::ok(trend))
}

async fn get_heatmap(
    data: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> impl Responder {
    let hours: i64 = query.get("hours").and_then(|s| s.parse().ok()).unwrap_or(168);
    let points = data.store.reader.query_heatmap_intensities(&data.locations, hours).await;
    HttpResponse::Ok().json(ApiResponse::ok(points))
}

async fn get_prediction(
    data: web::Data<Arc<AppState>>,
    path: web::Path<String>,
) -> impl Responder {
    let probe_id = path.into_inner();
    let loc = match data.locations.iter().find(|l| l.device_id == probe_id) {
        Some(l) => l,
        None => return HttpResponse::NotFound().json(ApiResponse::<()>::error("Probe not found")),
    };
    let material = loc.material_type.clone().unwrap_or_else(|| "iron".to_string());
    let current = data.store.reader.query_latest_corrosion_rate(&probe_id).await.unwrap_or(0.1);

    let (_temp, hum, ph, cl) = data.store.reader.query_zone_avg_env(&loc.zone, 72).await;
    let (temp, _, _, _) = (15.0f64, hum, ph, cl);

    let prediction = data.predictor.predict(&probe_id, &material, current, temp, hum, ph, cl);
    let _ = calculate_corrosion_rate_lpr;
    HttpResponse::Ok().json(ApiResponse::ok(prediction))
}

async fn get_stability(
    data: web::Data<Arc<AppState>>,
    path: web::Path<String>,
) -> impl Responder {
    let probe_id = path.into_inner();
    let loc = match data.locations.iter().find(|l| l.device_id == probe_id) {
        Some(l) => l,
        None => return HttpResponse::NotFound().json(ApiResponse::<()>::error("Probe not found")),
    };
    let material = loc.material_type.clone().unwrap_or_else(|| "iron".to_string());
    let current = data.store.reader.query_latest_corrosion_rate(&probe_id).await.unwrap_or(0.1);

    let (temp, hum, ph, cl) = data.store.reader.query_zone_avg_env(&loc.zone, 72).await;
    let assessment = StabilityAnalyzer::assess(&probe_id, &material, current, temp, hum, ph, cl);
    HttpResponse::Ok().json(ApiResponse::ok(assessment))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let config = match AppConfig::load() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to load config: {}", e);
            std::process::exit(1);
        }
    };
    let listen_addr = config.listen_addr();
    tracing::info!("Configuration loaded: listening on {}", listen_addr);

    let store = StorageService::new(&config);
    let alarm = AlertService::new(&config, store.writer.clone());
    let predictor = CorrosionPredictor::new();
    let gateway = LoraGateway::new();
    let locations = generate_device_locations();

    let app_state = Arc::new(AppState {
        config,
        store,
        alarm,
        predictor,
        gateway,
        locations,
    });

    tracing::info!("Starting HTTP server at http://{}", listen_addr);
    HttpServer::new(move || {
        let cors = Cors::permissive();
        App::new()
            .wrap(cors)
            .app_data(web::Data::new(app_state.clone()))
            .route("/api/health", web::get().to(health_check))
            .route("/api/stats", web::get().to(get_stats))
            .route("/api/locations", web::get().to(get_locations))
            .route("/api/lora/data", web::post().to(receive_lora_data))
            .route("/api/lora/gateway-stats", web::get().to(get_gateway_stats))
            .route("/api/corrosion/trend/{probe_id}", web::get().to(get_corrosion_trend))
            .route("/api/corrosion/heatmap", web::get().to(get_heatmap))
            .route("/api/corrosion/prediction/{probe_id}", web::get().to(get_prediction))
            .route("/api/corrosion/stability/{probe_id}", web::get().to(get_stability))
    })
    .bind(&listen_addr)?
    .run()
    .await
}
