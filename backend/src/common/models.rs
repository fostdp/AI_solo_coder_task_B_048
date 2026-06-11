use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoilData {
    pub sensor_id: String,
    pub zone: String,
    pub sensor_type: String,
    pub temperature: f64,
    pub humidity: f64,
    pub ph: f64,
    pub chloride: f64,
    pub timestamp: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrosionData {
    pub probe_id: String,
    pub zone: String,
    pub material_type: String,
    pub resistance: f64,
    pub polarization_resistance: f64,
    pub corrosion_rate: f64,
    pub timestamp: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoilReading {
    pub temperature: f64,
    pub humidity: f64,
    pub ph: f64,
    pub chloride: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrosionReading {
    pub material_type: String,
    pub resistance: f64,
    pub polarization_resistance: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LoraData {
    Soil(SoilReading),
    Corrosion(CorrosionReading),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoraPacket {
    pub device_type: String,
    pub device_id: String,
    pub zone: String,
    pub seq_id: u64,
    pub timestamp: DateTime<Utc>,
    pub data: LoraData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbeLocation {
    pub device_id: String,
    pub device_name: String,
    pub zone: String,
    pub device_type: String,
    pub material_type: Option<String>,
    pub lat: f64,
    pub lng: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrosionTrendPoint {
    pub time: i64,
    pub corrosion_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeatmapPoint {
    pub device_id: String,
    pub lat: f64,
    pub lng: f64,
    pub intensity: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrosionPrediction {
    pub probe_id: String,
    pub material_type: String,
    pub current_rate: f64,
    pub risk_level: String,
    pub confidence: f64,
    pub predicted_rate_7d: f64,
    pub predicted_rate_30d: f64,
    pub predicted_rate_90d: f64,
    pub predicted_avg_30d: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StabilityAssessment {
    pub probe_id: String,
    pub material_type: String,
    pub stability_index: f64,
    pub stability_level: String,
    pub env_score: f64,
    pub corrosion_factor: f64,
    pub remaining_lifetime_years: f64,
    pub recommendations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlarmEvent {
    pub event_id: String,
    pub probe_id: String,
    pub zone: String,
    pub material_type: String,
    pub alert_type: String,
    pub value: f64,
    pub threshold: f64,
    pub message: String,
    pub timestamp: DateTime<Utc>,
    pub severity: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T: Serialize> {
    pub success: bool,
    pub data: Option<T>,
    pub message: Option<String>,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        ApiResponse {
            success: true,
            data: Some(data),
            message: None,
        }
    }

    pub fn error(msg: impl Into<String>) -> Self {
        ApiResponse {
            success: false,
            data: None,
            message: Some(msg.into()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiteStats {
    pub total_soil_sensors: usize,
    pub total_corrosion_probes: usize,
    pub total_zones: usize,
    pub high_risk_probes: usize,
    pub avg_corrosion_rate: f64,
    pub avg_temperature: f64,
    pub avg_humidity: f64,
    pub avg_ph: f64,
    pub avg_chloride: f64,
}

fn zone_for_idx(idx: usize) -> &'static str {
    let zones = [
        "区域1-主营区", "区域2-药房区", "区域3-伤兵区",
        "区域4-器械库", "区域5-手术区", "区域6-外围",
    ];
    zones[idx % zones.len()]
}

pub fn generate_device_locations() -> Vec<ProbeLocation> {
    let center_lat = 34.2658;
    let center_lng = 108.9542;
    let mut locations = Vec::new();

    for i in 0..40 {
        let angle = (i as f64) * 0.157;
        let radius = 0.0005 + (i as f64 % 10.0) * 0.00007;
        locations.push(ProbeLocation {
            device_id: format!("SOIL-{:03}", i + 1),
            device_name: format!("土壤温湿度pH传感器 #{}", i + 1),
            zone: zone_for_idx(i).to_string(),
            device_type: "soil_sensor".to_string(),
            material_type: None,
            lat: center_lat + radius * angle.cos(),
            lng: center_lng + radius * angle.sin(),
        });
    }

    for i in 0..20 {
        let angle = (i as f64) * 0.314;
        let radius = 0.00035 + (i as f64 % 7.0) * 0.000055;
        let material = if i % 2 == 0 { "iron" } else { "copper" };
        locations.push(ProbeLocation {
            device_id: format!("CORR-{:03}", i + 1),
            device_name: format!("腐蚀监测探头 #{} ({})", i + 1, if material == "iron" { "铁器" } else { "铜器" }),
            zone: zone_for_idx(i).to_string(),
            device_type: "corrosion_probe".to_string(),
            material_type: Some(material.to_string()),
            lat: center_lat + radius * angle.cos(),
            lng: center_lng + radius * angle.sin(),
        });
    }

    locations
}
