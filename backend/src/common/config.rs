use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InfluxDBConfig {
    pub url: String,
    pub token: String,
    pub org: String,
    pub bucket: String,
    pub batch_size: usize,
    pub flush_interval_ms: u64,
    pub max_pending: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThresholdConfig {
    pub corrosion_rate: f64,
    pub chloride: f64,
    pub alert_cooldown_minutes: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeComConfig {
    pub webhook_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmsConfig {
    pub access_key_id: Option<String>,
    pub access_key_secret: Option<String>,
    pub sign_name: String,
    pub template_code: Option<String>,
    pub phone_numbers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertConfig {
    pub wecom: WeComConfig,
    pub sms: SmsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrosionEngineConfig {
    pub hidden_size: usize,
    pub dropout_rate: f64,
    pub l2_lambda: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub influxdb: InfluxDBConfig,
    pub thresholds: ThresholdConfig,
    pub alert: AlertConfig,
    pub corrosion_engine: CorrosionEngineConfig,
}

impl AppConfig {
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let content = fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read config file: {}", e))?;
        let config: AppConfig = toml::from_str(&content)
            .map_err(|e| format!("Failed to parse TOML config: {}", e))?;
        Ok(config)
    }

    pub fn load() -> Result<Self, String> {
        let paths = vec!["config.toml", "backend/config.toml"];
        for p in paths {
            if Path::new(p).exists() {
                return Self::load_from_file(p);
            }
        }
        Err("config.toml not found in current directory or backend/".to_string())
    }

    pub fn listen_addr(&self) -> String {
        format!("{}:{}", self.server.host, self.server.port)
    }

    pub fn corrosion_threshold(&self) -> f64 {
        self.thresholds.corrosion_rate
    }

    pub fn chloride_threshold(&self) -> f64 {
        self.thresholds.chloride
    }

    pub fn wecom_webhook(&self) -> Option<String> {
        self.alert.wecom.webhook_url.clone()
    }

    pub fn influxdb_url(&self) -> &str {
        &self.influxdb.url
    }

    pub fn influxdb_org(&self) -> &str {
        &self.influxdb.org
    }

    pub fn influxdb_token(&self) -> &str {
        &self.influxdb.token
    }

    pub fn influxdb_bucket(&self) -> &str {
        &self.influxdb.bucket
    }
}
