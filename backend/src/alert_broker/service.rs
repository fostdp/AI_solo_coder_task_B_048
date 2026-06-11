use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use chrono::Utc;
use tokio::sync::RwLock;
use reqwest::Client;
use serde_json::json;
use uuid::Uuid;
use tracing::{info, warn, error};
use crate::common::*;
use crate::storage::BatchWriter;
use crate::metrics;

pub struct AlertService {
    config: AppConfig,
    store_writer: BatchWriter,
    http_client: Client,
    last_alarms: Arc<RwLock<HashMap<String, chrono::DateTime<Utc>>>>,
}

impl AlertService {
    pub fn new(config: &AppConfig, store_writer: BatchWriter) -> Self {
        Self {
            config: config.clone(),
            store_writer,
            http_client: Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .unwrap_or_default(),
            last_alarms: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    async fn should_alert(&self, key: &str, min: i64) -> bool {
        let last_alarms = self.last_alarms.read().await;
        if let Some(last_time) = last_alarms.get(key) {
            let elapsed = Utc::now() - *last_time;
            elapsed.num_minutes() >= min
        } else {
            true
        }
    }

    async fn mark_sent(&self, key: &str) {
        let mut last_alarms = self.last_alarms.write().await;
        last_alarms.insert(key.to_string(), Utc::now());
    }

    pub async fn check_and_alert_corrosion(
        &self,
        probe_id: &str,
        zone: &str,
        material_type: &str,
        corrosion_rate: f64,
    ) -> Result<bool, AppError> {
        let threshold = self.config.thresholds.corrosion_rate;
        if corrosion_rate < threshold {
            return Ok(false);
        }

        let key = format!("corrosion_{}_{}", probe_id, zone);
        if !self.should_alert(&key, self.config.thresholds.alert_cooldown_minutes).await {
            return Ok(false);
        }

        let event_id = Uuid::new_v4().to_string();
        let severity = if corrosion_rate > 1.0 { "critical".to_string() } else { "warning".to_string() };
        let message = format!(
            "> **Corrosion Alert**\n> Corrosion threshold exceeded at {}\n> Rate: **{:.4}** mm/y (threshold: {})",
            zone, corrosion_rate, threshold
        );

        let event = AlarmEvent {
            event_id,
            probe_id: probe_id.to_string(),
            zone: zone.to_string(),
            material_type: material_type.to_string(),
            alert_type: "corrosion".to_string(),
            value: corrosion_rate,
            threshold,
            message: message.clone(),
            timestamp: Utc::now(),
            severity,
        };

        self.store_writer.write_alarm_event(event).await?;

        self.send_wecom(&message).await?;
        self.send_sms(&message).await?;
        self.mark_sent(&key).await;

        metrics::inc_corrosion_alerts();

        Ok(true)
    }

    pub async fn check_and_alert_chloride(
        &self,
        sensor_id: &str,
        zone: &str,
        chloride: f64,
    ) -> Result<bool, AppError> {
        let threshold = self.config.thresholds.chloride;
        if chloride < threshold {
            return Ok(false);
        }

        let key = format!("chloride_{}_{}", sensor_id, zone);
        if !self.should_alert(&key, self.config.thresholds.alert_cooldown_minutes).await {
            return Ok(false);
        }

        let event_id = Uuid::new_v4().to_string();
        let severity = if chloride > threshold * 2.0 { "critical".to_string() } else { "warning".to_string() };
        let message = format!(
            "> **Chloride Alert**\n> Chloride threshold exceeded at {}\n> Concentration: **{:.4}** mg/L (threshold: {})",
            zone, chloride, threshold
        );

        let event = AlarmEvent {
            event_id,
            probe_id: sensor_id.to_string(),
            zone: zone.to_string(),
            material_type: String::new(),
            alert_type: "chloride".to_string(),
            value: chloride,
            threshold,
            message: message.clone(),
            timestamp: Utc::now(),
            severity,
        };

        self.store_writer.write_alarm_event(event).await?;

        self.send_wecom(&message).await?;
        self.send_sms(&message).await?;
        self.mark_sent(&key).await;

        Ok(true)
    }

    async fn send_wecom(&self, message: &str) -> Result<(), AppError> {
        let webhook = match self.config.alert.wecom.webhook_url.as_deref() {
            Some(url) if !url.is_empty() => url,
            _ => return Ok(()),
        };

        let payload = json!({
            "msgtype": "markdown",
            "markdown": {
                "content": message
            }
        });

        self.http_client
            .post(webhook)
            .json(&payload)
            .send()
            .await
            .map_err(|e| AppError::HttpClient(e.to_string()))?;

        Ok(())
    }

    async fn send_sms(&self, _message: &str) -> Result<(), AppError> {
        if self.config.alert.sms.access_key_id.is_none() || self.config.alert.sms.access_key_id.as_ref().map_or(true, |k| k.is_empty()) {
            info!("SMS not configured, skipping alert send");
            return Ok(());
        }

        info!("SMS would be sent with sign_name: {}", self.config.alert.sms.sign_name);
        Ok(())
    }
}
