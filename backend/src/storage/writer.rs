use influxdb::{Client, WriteQuery};
use tokio::sync::mpsc;
use chrono::Utc;
use crate::common::*;
use std::sync::Arc;

pub enum WriteCommand {
    Soil(SoilData),
    Corrosion(CorrosionData),
    Alarm(AlarmEvent),
    Shutdown,
}

#[derive(Clone)]
pub struct BatchWriter {
    sender: mpsc::UnboundedSender<WriteCommand>,
}

fn escape_tag_value(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace(',', "\\,")
        .replace('=', "\\=")
        .replace(' ', "\\ ")
}

fn escape_field_string(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn soil_to_line(data: &SoilData) -> String {
    let ts = data
        .timestamp
        .map_or_else(|| Utc::now().timestamp_nanos(), |t| t.timestamp_nanos());
    format!(
        "soil,sensor_id={},zone={},sensor_type={} temperature={},humidity={},ph={},chloride={} {}",
        escape_tag_value(&data.sensor_id),
        escape_tag_value(&data.zone),
        escape_tag_value(&data.sensor_type),
        data.temperature,
        data.humidity,
        data.ph,
        data.chloride,
        ts
    )
}

fn corrosion_to_line(data: &CorrosionData) -> String {
    let ts = data
        .timestamp
        .map_or_else(|| Utc::now().timestamp_nanos(), |t| t.timestamp_nanos());
    format!(
        "corrosion,probe_id={},zone={},material_type={} resistance={},polarization_resistance={},corrosion_rate={} {}",
        escape_tag_value(&data.probe_id),
        escape_tag_value(&data.zone),
        escape_tag_value(&data.material_type),
        data.resistance,
        data.polarization_resistance,
        data.corrosion_rate,
        ts
    )
}

fn alarm_to_line(event: &AlarmEvent) -> String {
    let ts = event.timestamp.timestamp_nanos();
    format!(
        "alarm,probe_id={},zone={},material_type={},alert_type={},severity={} value={},threshold={},message=\"{}\" {}",
        escape_tag_value(&event.probe_id),
        escape_tag_value(&event.zone),
        escape_tag_value(&event.material_type),
        escape_tag_value(&event.alert_type),
        escape_tag_value(&event.severity),
        event.value,
        event.threshold,
        escape_field_string(&event.message),
        ts
    )
}

impl BatchWriter {
    pub fn new(
        client: Client,
        bucket: String,
        batch_size: usize,
        flush_interval_ms: u64,
        _max_pending: usize,
    ) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();

        let client = Arc::new(client);
        let bucket_clone = bucket.clone();

        tokio::spawn(async move {
            let mut batch: Vec<String> = Vec::with_capacity(batch_size);
            let mut flush_interval = tokio::time::interval(
                std::time::Duration::from_millis(flush_interval_ms)
            );
            let mut rx = rx;

            loop {
                tokio::select! {
                    cmd = rx.recv() => {
                        match cmd {
                            Some(WriteCommand::Soil(data)) => {
                                batch.push(soil_to_line(&data));
                                if batch.len() >= batch_size {
                                    Self::flush(&client, &bucket_clone, &mut batch).await;
                                }
                            }
                            Some(WriteCommand::Corrosion(data)) => {
                                batch.push(corrosion_to_line(&data));
                                if batch.len() >= batch_size {
                                    Self::flush(&client, &bucket_clone, &mut batch).await;
                                }
                            }
                            Some(WriteCommand::Alarm(event)) => {
                                batch.push(alarm_to_line(&event));
                                if batch.len() >= batch_size {
                                    Self::flush(&client, &bucket_clone, &mut batch).await;
                                }
                            }
                            Some(WriteCommand::Shutdown) => {
                                if !batch.is_empty() {
                                    Self::flush(&client, &bucket_clone, &mut batch).await;
                                }
                                break;
                            }
                            None => {
                                if !batch.is_empty() {
                                    Self::flush(&client, &bucket_clone, &mut batch).await;
                                }
                                break;
                            }
                        }
                    }
                    _ = flush_interval.tick() => {
                        if !batch.is_empty() {
                            Self::flush(&client, &bucket_clone, &mut batch).await;
                        }
                    }
                }
            }
        });

        Self { sender: tx }
    }

    async fn flush(client: &Arc<Client>, bucket: &str, batch: &mut Vec<String>) {
        let line_protocol = batch.join("\n");
        let write_query = WriteQuery::new(bucket.to_string(), line_protocol);
        match client.query(&write_query).await {
            Ok(_) => {}
            Err(e) => {
                tracing::error!("Batch write failed: {}", e);
            }
        }
        batch.clear();
    }

    pub async fn write_soil_data(&self, data: SoilData) -> Result<(), AppError> {
        self.sender
            .send(WriteCommand::Soil(data))
            .map_err(|_| AppError::Backpressure)
    }

    pub async fn write_corrosion_data(&self, data: CorrosionData) -> Result<(), AppError> {
        self.sender
            .send(WriteCommand::Corrosion(data))
            .map_err(|_| AppError::Backpressure)
    }

    pub async fn write_alarm_event(&self, event: AlarmEvent) -> Result<(), AppError> {
        self.sender
            .send(WriteCommand::Alarm(event))
            .map_err(|_| AppError::Backpressure)
    }

    pub async fn shutdown(&self) -> Result<(), AppError> {
        self.sender
            .send(WriteCommand::Shutdown)
            .map_err(|_| AppError::Backpressure)
    }
}
