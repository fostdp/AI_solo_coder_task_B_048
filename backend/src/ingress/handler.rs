use actix_web::{web, HttpResponse};
use std::sync::Arc;
use crate::common::*;
use crate::ingress::LoraGateway;
use crate::storage::StorageService;
use crate::alert_broker::AlertService;
use crate::corrosion_engine::calculate_corrosion_rate_lpr;
use crate::metrics;

pub async fn receive_lora_data(
    data: web::Data<Arc<crate::AppState>>,
    packet: web::Json<LoraPacket>,
) -> Result<HttpResponse, AppError> {
    let packet = packet.into_inner();
    let _id = packet.device_id.clone();
    let _seq = packet.seq_id;
    let ordered = data.gateway.receive_packet(packet).await?;
    let reordered = ordered.is_empty();
    metrics::inc_lora_packets(reordered);
    if ordered.is_empty() {
        tracing::debug!("Packet cached in reorder window, id={}, seq={}", _id, _seq);
    }
    for pkt in ordered {
        process_one(&data, &pkt).await?;
    }
    Ok(HttpResponse::Ok().json(ApiResponse::<()>::ok(())))
}

async fn process_one(
    data: &web::Data<Arc<crate::AppState>>,
    pkt: &LoraPacket,
) -> Result<(), AppError> {
    match &pkt.data {
        LoraData::Soil(soil) => {
            let sd = SoilData {
                sensor_id: pkt.device_id.clone(),
                zone: pkt.zone.clone(),
                sensor_type: "multi".into(),
                temperature: soil.temperature,
                humidity: soil.humidity,
                ph: soil.ph,
                chloride: soil.chloride,
                timestamp: Some(pkt.timestamp),
            };
            data.store.writer.write_soil_data(sd).await?;
            data.alarm
                .check_and_alert_chloride(&pkt.device_id, &pkt.zone, soil.chloride)
                .await?;
        }
        LoraData::Corrosion(corr) => {
            let rate = calculate_corrosion_rate_lpr(corr.polarization_resistance, &corr.material_type);
            let cd = CorrosionData {
                probe_id: pkt.device_id.clone(),
                zone: pkt.zone.clone(),
                material_type: corr.material_type.clone(),
                resistance: corr.resistance,
                polarization_resistance: corr.polarization_resistance,
                corrosion_rate: rate,
                timestamp: Some(pkt.timestamp),
            };
            data.store.writer.write_corrosion_data(cd).await?;
            data.alarm
                .check_and_alert_corrosion(&pkt.device_id, &pkt.zone, &corr.material_type, rate)
                .await?;
        }
    }
    Ok(())
}

pub async fn get_gateway_stats(
    data: web::Data<Arc<crate::AppState>>,
) -> Result<HttpResponse, AppError> {
    Ok(HttpResponse::Ok().json(ApiResponse::ok(data.gateway.get_stats().await)))
}
