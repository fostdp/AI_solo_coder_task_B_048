pub mod common;
pub mod ingress;
pub mod corrosion_engine;
pub mod storage;
pub mod alert_broker;
pub mod metrics;

use actix_cors::Cors;
use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use std::sync::Arc;
use std::time::Instant;

use common::*;
use ingress::{LoraGateway, receive_lora_data, get_gateway_stats};
use corrosion_engine::{CorrosionPredictor, StabilityAnalyzer, calculate_corrosion_rate_lpr};
use storage::StorageService;
use alert_broker::AlertService;
use heritage_vulnerability::{FuzzyEvaluator, EDSComposition, MaterialAlloy};
use protection_penetration::{PenetrationThreadPool, SimulationRequest, get_material, all_materials, MaterialType, ProtectiveMaterial};
use microbiome::{MicrobeCorrelationAnalyzer, default_microbe_dataset, MicrobiomeSample};
use groundwater::{GroundwaterModel, ChlorideTransport, default_simulation_params, default_sensitive_zones_list, GroundwaterTaskQueue};
use prometheus::Registry;

fn mock_eds_for_location(loc: &ProbeLocation) -> Option<EDSComposition> {
    use heritage_vulnerability::eds_data::{default_iron_eds, default_bronze_eds};

    if loc.device_type != "corrosion_probe" {
        return None;
    }
    let is_iron = loc.material_type.as_deref().unwrap_or("iron") == "iron";
    let artifact_id = format!("ART-{}", loc.device_id.replace("CORR-", ""));
    if is_iron {
        Some(default_iron_eds(&artifact_id, &loc.device_id))
    } else {
        Some(default_bronze_eds(&artifact_id, &loc.device_id))
    }
}

pub struct AppState {
    pub config: AppConfig,
    pub store: StorageService,
    pub alarm: AlertService,
    pub predictor: CorrosionPredictor,
    pub gateway: LoraGateway,
    pub locations: Vec<ProbeLocation>,
    pub registry: Registry,
    pub fuzzy_evaluator: FuzzyEvaluator,
    pub penetration_pool: PenetrationThreadPool,
    pub microbe_analyzer: MicrobeCorrelationAnalyzer,
    pub microbe_dataset: Vec<MicrobiomeSample>,
    pub groundwater_queue: GroundwaterTaskQueue,
}

async fn health_check() -> impl Responder {
    let start = Instant::now();
    let response = HttpResponse::Ok().json(ApiResponse::ok(serde_json::json!({
        "status": "healthy",
        "version": "3.0.0",
    })));
    let duration = start.elapsed().as_secs_f64();
    metrics::record_request("GET", "/api/health", "200", duration);
    response
}

async fn get_locations(data: web::Data<Arc<AppState>>) -> impl Responder {
    HttpResponse::Ok().json(ApiResponse::ok(data.locations.clone()))
}

async fn get_stats(data: web::Data<Arc<AppState>>) -> impl Responder {
    let start = Instant::now();
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

    let response = HttpResponse::Ok().json(ApiResponse::ok(SiteStats {
        total_soil_sensors: soil_count,
        total_corrosion_probes: corr_count,
        total_zones: zones.len(),
        high_risk_probes: high_risk,
        avg_corrosion_rate: avg_rate,
        avg_temperature: 15.0,
        avg_humidity: 45.0,
        avg_ph: 7.2,
        avg_chloride: 55.0,
    }));
    let duration = start.elapsed().as_secs_f64();
    metrics::record_request("GET", "/api/stats", "200", duration);
    response
}

async fn get_metrics(data: web::Data<Arc<AppState>>) -> impl Responder {
    use prometheus::TextEncoder;
    let encoder = TextEncoder::new();
    let metric_families = data.registry.gather();
    let mut buffer = String::new();
    encoder.encode_utf8(&metric_families, &mut buffer).unwrap();
    HttpResponse::Ok()
        .content_type("text/plain; version=0.0.4; charset=utf-8")
        .body(buffer)
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

async fn get_vulnerability(
    data: web::Data<Arc<AppState>>,
    path: web::Path<String>,
) -> impl Responder {
    let probe_id = path.into_inner();
    let loc = match data.locations.iter().find(|l| l.device_id == probe_id) {
        Some(l) => l,
        None => return HttpResponse::NotFound().json(ApiResponse::<()>::error("Probe not found")),
    };
    let eds = match mock_eds_for_location(loc) {
        Some(e) => e,
        None => return HttpResponse::BadRequest().json(ApiResponse::<()>::error("Only corrosion probes have EDS data")),
    };
    let current = data.store.reader.query_latest_corrosion_rate(&probe_id).await.unwrap_or(0.15);
    let (temp, hum, ph, cl) = data.store.reader.query_zone_avg_env(&loc.zone, 72).await;
    let temp = if temp == 0.0 { 15.0 } else { temp };
    let hum = if hum == 0.0 { 50.0 } else { hum };
    let ph = if ph == 0.0 { 7.0 } else { ph };
    let cl = if cl == 0.0 { 50.0 } else { cl };
    let result = data.fuzzy_evaluator.evaluate(&eds, current, temp, hum, ph, cl);
    HttpResponse::Ok().json(ApiResponse::ok(result))
}

async fn get_all_vulnerabilities(
    data: web::Data<Arc<AppState>>,
) -> impl Responder {
    let mut results = Vec::new();
    for loc in data.locations.iter().filter(|l| l.device_type == "corrosion_probe") {
        if let Some(eds) = mock_eds_for_location(loc) {
            let current = data.store.reader.query_latest_corrosion_rate(&loc.device_id).await.unwrap_or(0.15);
            let (temp, hum, ph, cl) = data.store.reader.query_zone_avg_env(&loc.zone, 72).await;
            let temp = if temp == 0.0 { 15.0 } else { temp };
            let hum = if hum == 0.0 { 50.0 } else { hum };
            let ph = if ph == 0.0 { 7.0 } else { ph };
            let cl = if cl == 0.0 { 50.0 } else { cl };
            results.push(data.fuzzy_evaluator.evaluate(&eds, current, temp, hum, ph, cl));
        }
    }
    HttpResponse::Ok().json(ApiResponse::ok(results))
}

async fn simulate_penetration(
    data: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> impl Responder {
    let material_name = query.get("material").cloned().unwrap_or_else(|| "Silicone".to_string());
    let material = match material_name.as_str() {
        "Silicone" | "有机硅" => MaterialType::Silicone,
        "Fluoropolymer" | "氟聚合物" => MaterialType::Fluoropolymer,
        "Acrylate" | "丙烯酸酯" => MaterialType::Acrylate,
        "Epoxy" | "环氧树脂" => MaterialType::Epoxy,
        "Paraffin" | "石蜡" => MaterialType::Paraffin,
        "NanoSiO2" | "纳米SiO2" => MaterialType::NanoSiO2,
        _ => MaterialType::Silicone,
    };
    let temp: f64 = query.get("temperature").and_then(|s| s.parse().ok()).unwrap_or(20.0);
    let hum: f64 = query.get("humidity").and_then(|s| s.parse().ok()).unwrap_or(50.0);
    let porosity: f64 = query.get("porosity").and_then(|s| s.parse().ok()).unwrap_or(0.15);
    let conc: f64 = query.get("concentration").and_then(|s| s.parse().ok()).unwrap_or(1.0);
    let hours: f64 = query.get("hours").and_then(|s| s.parse().ok()).unwrap_or(24.0);
    let mat = get_material(material);
    let request = SimulationRequest {
        material: mat,
        temp,
        humidity: hum,
        porosity,
        concentration: conc,
        hours,
    };
    let rx = data.penetration_pool.submit(request);
    let result = tokio::task::spawn_blocking(move || {
        rx.recv().expect("penetration worker panicked")
    }).await;
    match result {
        Ok(r) => HttpResponse::Ok().json(ApiResponse::ok(r)),
        Err(_) => HttpResponse::InternalServerError().json(ApiResponse::<()>::error("Penetration simulation failed")),
    }
}

async fn get_protection_materials() -> impl Responder {
    HttpResponse::Ok().json(ApiResponse::ok(all_materials()))
}

async fn get_microbiome_analysis(data: web::Data<Arc<AppState>>) -> impl Responder {
    let result = data.microbe_analyzer.analyze(&data.microbe_dataset);
    HttpResponse::Ok().json(ApiResponse::ok(result))
}

async fn get_microbiome_samples(data: web::Data<Arc<AppState>>) -> impl Responder {
    HttpResponse::Ok().json(ApiResponse::ok(data.microbe_dataset.clone()))
}

async fn submit_groundwater_task(
    data: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> impl Responder {
    let task_type = query.get("type").cloned().unwrap_or_else(|| "groundwater".to_string());
    let days: f64 = query.get("days").and_then(|s| s.parse().ok()).unwrap_or(90.0);
    let threshold: f64 = query.get("threshold").and_then(|s| s.parse().ok()).unwrap_or(100.0);
    let params = serde_json::json!({
        "days": days,
        "threshold": threshold,
        "top_head": 15.0,
        "bottom_head": 10.0,
    });
    let task_id = data.groundwater_queue.submit_task(&task_type, params);
    HttpResponse::Ok().json(ApiResponse::ok(serde_json::json!({
        "task_id": task_id,
        "status": "submitted",
    })))
}

async fn get_groundwater_result(
    data: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> impl Responder {
    let task_id = match query.get("task_id") {
        Some(id) => id.clone(),
        None => return HttpResponse::BadRequest().json(ApiResponse::<()>::error("task_id required")),
    };
    match data.groundwater_queue.get_result(&task_id) {
        Some(result) => HttpResponse::Ok().json(ApiResponse::ok(result)),
        None => HttpResponse::Ok().json(ApiResponse::ok(serde_json::json!({
            "task_id": task_id,
            "status": "pending",
        }))),
    }
}

async fn simulate_groundwater(
    query: web::Query<std::collections::HashMap<String, String>>,
) -> impl Responder {
    let days: f64 = query.get("days").and_then(|s| s.parse().ok()).unwrap_or(90.0);
    let threshold: f64 = query.get("threshold").and_then(|s| s.parse().ok()).unwrap_or(100.0);
    let (rows, cols, size, wells) = default_simulation_params();
    let model = GroundwaterModel::new(rows, cols, size);
    let flow = model.solve_steady_state(
        15.0,
        10.0,
        None,
        None,
        1e-5,
        None,
        &wells,
        0.0,
        0.0,
    );
    let zones = default_sensitive_zones_list();
    let transport = ChlorideTransport::new();
    let diffusion = transport.simulate(&flow, days, threshold, &zones);
    HttpResponse::Ok().json(ApiResponse::ok(serde_json::json!({
        "flow_field": flow,
        "diffusion": diffusion,
    })))
}

async fn get_groundwater_sensitive_zones() -> impl Responder {
    HttpResponse::Ok().json(ApiResponse::ok(default_sensitive_zones_list()))
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
    let registry = metrics::register_custom_metrics();
    let fuzzy_evaluator = FuzzyEvaluator::new();
    let penetration_pool = PenetrationThreadPool::new(4);
    let microbe_analyzer = MicrobeCorrelationAnalyzer::new();
    let microbe_dataset = default_microbe_dataset();
    let groundwater_queue = GroundwaterTaskQueue::new();

    let app_state = Arc::new(AppState {
        config,
        store,
        alarm,
        predictor,
        gateway,
        locations,
        registry,
        fuzzy_evaluator,
        penetration_pool,
        microbe_analyzer,
        microbe_dataset,
        groundwater_queue,
    });

    tracing::info!("Starting HTTP server at http://{}", listen_addr);
    HttpServer::new(move || {
        let cors = Cors::permissive();
        App::new()
            .wrap(cors)
            .app_data(web::Data::new(app_state.clone()))
            .route("/metrics", web::get().to(get_metrics))
            .route("/api/health", web::get().to(health_check))
            .route("/api/stats", web::get().to(get_stats))
            .route("/api/locations", web::get().to(get_locations))
            .route("/api/lora/data", web::post().to(receive_lora_data))
            .route("/api/lora/gateway-stats", web::get().to(get_gateway_stats))
            .route("/api/corrosion/trend/{probe_id}", web::get().to(get_corrosion_trend))
            .route("/api/corrosion/heatmap", web::get().to(get_heatmap))
            .route("/api/corrosion/prediction/{probe_id}", web::get().to(get_prediction))
            .route("/api/corrosion/stability/{probe_id}", web::get().to(get_stability))
            .route("/api/vulnerability/{probe_id}", web::get().to(get_vulnerability))
            .route("/api/vulnerability/all", web::get().to(get_all_vulnerabilities))
            .route("/api/protection/materials", web::get().to(get_protection_materials))
            .route("/api/protection/simulate", web::get().to(simulate_penetration))
            .route("/api/microbiome/samples", web::get().to(get_microbiome_samples))
            .route("/api/microbiome/analysis", web::get().to(get_microbiome_analysis))
            .route("/api/groundwater/sensitive-zones", web::get().to(get_groundwater_sensitive_zones))
            .route("/api/groundwater/simulate", web::get().to(simulate_groundwater))
            .route("/api/groundwater/task", web::post().to(submit_groundwater_task))
            .route("/api/groundwater/result", web::get().to(get_groundwater_result))
    })
    .bind(&listen_addr)?
    .run()
    .await
}
