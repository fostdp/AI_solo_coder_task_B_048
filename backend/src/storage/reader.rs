use influxdb::{Client, ReadQuery};
use chrono::{DateTime, Utc};
use crate::common::*;
use std::sync::Arc;

pub struct StorageReader {
    client: Arc<Client>,
    bucket: String,
}

impl StorageReader {
    pub fn new(client: Client, _config: AppConfig) -> Self {
        Self {
            client: Arc::new(client),
            bucket: _config.influxdb.bucket,
        }
    }

    pub async fn query_corrosion_trend(
        &self,
        probe_id: &str,
        hours: i64,
    ) -> Result<Vec<CorrosionTrendPoint>, AppError> {
        let flux = format!(
            r#"from(bucket:"{}") |> range(start: -{}h) |> filter(fn:(r)=>r._measurement=="corrosion" and r.probe_id=="{}") |> filter(fn:(r)=>r._field=="corrosion_rate") |> keep(columns:["_time","_value"]) |> sort(columns:["_time"])"#,
            self.bucket,
            hours,
            probe_id
        );

        let result = self.client
            .query(ReadQuery::new(&flux))
            .await;

        match result {
            Ok(response) => {
                Ok(parse_trend_response(&response))
            }
            Err(e) => {
                tracing::error!("Query corrosion trend failed: {}", e);
                Ok(Vec::new())
            }
        }
    }

    pub async fn query_heatmap_intensities(
        &self,
        locations: &[ProbeLocation],
        hours: i64,
    ) -> Result<Vec<HeatmapPoint>, AppError> {
        let mut points = Vec::new();

        for loc in locations {
            if loc.device_type != "corrosion_probe" {
                continue;
            }
            let intensity = self.query_latest_corrosion_rate(&loc.device_id).await?;
            points.push(HeatmapPoint {
                device_id: loc.device_id.clone(),
                lat: loc.lat,
                lng: loc.lng,
                intensity: intensity.unwrap_or(0.0),
            });
        }

        Ok(points)
    }

    pub async fn query_latest_corrosion_rate(
        &self,
        probe_id: &str,
    ) -> Result<Option<f64>, AppError> {
        let flux = format!(
            r#"from(bucket:"{}") |> range(start: -{}h) |> filter(fn:(r)=>r._measurement=="corrosion" and r.probe_id=="{}") |> filter(fn:(r)=>r._field=="corrosion_rate") |> last() |> keep(columns:["_value"])"#,
            self.bucket,
            24,
            probe_id
        );

        let result = self.client
            .query(ReadQuery::new(&flux))
            .await;

        match result {
            Ok(response) => {
                Ok(parse_latest_rate(&response))
            }
            Err(e) => {
                tracing::error!("Query latest corrosion rate failed: {}", e);
                Ok(None)
            }
        }
    }

    pub async fn query_zone_avg_env(
        &self,
        _zone: &str,
        _hours: i64,
    ) -> Result<(f64, f64, f64, f64), AppError> {
        Ok((0.0, 0.0, 0.0, 0.0))
    }
}

fn parse_trend_response(response: &str) -> Vec<CorrosionTrendPoint> {
    let mut points = Vec::new();
    for line in response.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        let cols: Vec<&str> = line.split(',').collect();
        if cols.len() < 3 {
            continue;
        }
        let time_str = cols[cols.len() - 2];
        let value_str = cols[cols.len() - 1];
        if let Ok(dt) = DateTime::parse_from_rfc3339(time_str) {
            let dt_utc: DateTime<Utc> = dt.with_timezone(&Utc);
            if let Ok(val) = value_str.parse::<f64>() {
                points.push(CorrosionTrendPoint {
                    time: dt_utc.timestamp_millis(),
                    corrosion_rate: val,
                });
            }
        }
    }
    points
}

fn parse_latest_rate(response: &str) -> Option<f64> {
    for line in response.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        let cols: Vec<&str> = line.split(',').collect();
        if cols.len() >= 2 {
            if let Ok(val) = cols[cols.len() - 1].parse::<f64>() {
                return Some(val);
            }
        }
    }
    None
}
