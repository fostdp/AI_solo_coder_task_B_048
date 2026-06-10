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
pub struct LoraPacket {
    pub device_type: String,
    pub device_id: String,
    pub zone: String,
    pub seq_id: u64,
    pub timestamp: DateTime<Utc>,
    pub data: LoraData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum LoraData {
    Soil(SoilReading),
    Corrosion(CorrosionReading),
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
pub struct ProbeLocation {
    pub id: String,
    pub name: String,
    pub zone: String,
    pub device_type: String,
    pub lat: f64,
    pub lng: f64,
    pub material_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrosionTrendPoint {
    pub timestamp: DateTime<Utc>,
    pub corrosion_rate: f64,
    pub avg_temperature: Option<f64>,
    pub avg_humidity: Option<f64>,
    pub avg_chloride: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeatmapPoint {
    pub lat: f64,
    pub lng: f64,
    pub intensity: f64,
    pub probe_id: String,
    pub zone: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrosionPrediction {
    pub probe_id: String,
    pub material_type: String,
    pub current_rate: f64,
    pub predicted_rate_7d: f64,
    pub predicted_rate_30d: f64,
    pub predicted_rate_90d: f64,
    pub risk_level: String,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StabilityAssessment {
    pub probe_id: String,
    pub material_type: String,
    pub stability_index: f64,
    pub stability_level: String,
    pub remaining_lifetime_years: f64,
    pub recommendations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlarmEvent {
    pub device_id: String,
    pub device_type: String,
    pub zone: String,
    pub alarm_type: String,
    pub level: String,
    pub message: String,
    pub value: f64,
    pub threshold: f64,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub message: String,
}

impl<T> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        ApiResponse {
            success: true,
            data: Some(data),
            message: "success".to_string(),
        }
    }

    pub fn error(msg: &str) -> ApiResponse<()> {
        ApiResponse {
            success: false,
            data: None,
            message: msg.to_string(),
        }
    }
}

pub fn generate_device_locations() -> Vec<ProbeLocation> {
    let mut locations = Vec::new();
    let base_lat = 34.2658;
    let base_lng = 108.9542;

    for i in 1..=40 {
        let row = ((i - 1) / 8) as f64;
        let col = ((i - 1) % 8) as f64;
        locations.push(ProbeLocation {
            id: format!("SOIL-{:03}", i),
            name: format!("土壤传感器-{}", i),
            zone: format!("区域-{}", (row as usize % 5) + 1),
            device_type: "soil_sensor".to_string(),
            lat: base_lat + row * 0.00045 - 0.0011,
            lng: base_lng + col * 0.00035 - 0.0012,
            material_type: None,
        });
    }

    for i in 1..=20 {
        let row = ((i - 1) / 5) as f64;
        let col = ((i - 1) % 5) as f64;
        let material = if i % 2 == 0 { "iron" } else { "copper" };
        locations.push(ProbeLocation {
            id: format!("CORR-{:03}", i),
            name: format!("腐蚀探头-{}-{}", if material == "iron" { "铁" } else { "铜" }, i),
            zone: format!("区域-{}", (row as usize % 5) + 1),
            device_type: "corrosion_probe".to_string(),
            lat: base_lat + row * 0.0009 + 0.0001 - 0.0009,
            lng: base_lng + col * 0.00056 + 0.0001 - 0.0011,
            material_type: Some(material.to_string()),
        });
    }

    locations
}
