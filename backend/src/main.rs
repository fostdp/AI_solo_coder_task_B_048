mod alarm;
mod config;
mod corrosion_algorithm;
mod error;
mod handlers;
mod influxdb_store;
mod lora_gateway;
mod models;
mod nn_model;

use std::sync::Arc;
use actix_cors::Cors;
use actix_web::{web, App, HttpServer};
use tracing::info;
use tracing_subscriber::{EnvFilter, fmt};

use crate::alarm::AlarmService;
use crate::config::AppConfig;
use crate::handlers::AppState;
use crate::influxdb_store::InfluxDBStore;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .init();

    let config = AppConfig::load();
    info!("配置加载完成: {}:{}", config.server_host, config.server_port);

    let store = InfluxDBStore::new(&config);
    info!("InfluxDB 客户端初始化: {}", config.influxdb_url);

    let alarm = AlarmService::new(&config, &store);
    let app_state = Arc::new(AppState::new(store, alarm));

    info!("启动古代战地医院遗址腐蚀监测系统后端服务...");
    info!("站点: 宋代战地医院遗址 (2000㎡)");
    info!("传感器: 40台土壤环境传感器 + 20台金属腐蚀监测探头");
    info!("采集频率: 每30分钟上报一次");

    let bind_addr = format!("{}:{}", config.server_host, config.server_port);

    HttpServer::new(move || {
        let cors = Cors::permissive();

        App::new()
            .wrap(cors)
            .app_data(web::Data::new(app_state.clone()))
            .route("/api/health", web::get().to(handlers::health_check))
            .route("/api/stats", web::get().to(handlers::get_stats))
            .route("/api/locations", web::get().to(handlers::get_locations))
            .route("/api/lora/data", web::post().to(handlers::receive_lora_data))
            .route("/api/lora/gateway-stats", web::get().to(handlers::get_gateway_stats))
            .route("/api/corrosion/trend/{probe_id}", web::get().to(handlers::get_corrosion_trend))
            .route("/api/corrosion/heatmap", web::get().to(handlers::get_heatmap))
            .route("/api/corrosion/prediction/{probe_id}", web::get().to(handlers::get_prediction))
            .route("/api/corrosion/stability/{probe_id}", web::get().to(handlers::get_stability))
    })
    .bind(&bind_addr)?
    .run()
    .await
}
