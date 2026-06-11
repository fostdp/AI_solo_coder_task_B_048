use crate::common::{LoraPacket, AppError};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use std::time::Duration;
use chrono::{DateTime, Utc};
use tokio::sync::RwLock;
use tokio::time::interval;
use tracing::{debug, warn};

struct BufferedPacket {
    packet: LoraPacket,
    received_at: DateTime<Utc>,
}

pub struct DeviceWindow {
    device_id: String,
    expected_seq: u64,
    buffer: BTreeMap<u64, BufferedPacket>,
    highest_seq: u64,
    lowest_seq: u64,
}

impl DeviceWindow {
    fn new(device_id: String, initial_seq: u64) -> Self {
        Self {
            device_id,
            expected_seq: initial_seq,
            buffer: BTreeMap::new(),
            highest_seq: initial_seq,
            lowest_seq: initial_seq,
        }
    }

    fn insert(&mut self, packet: LoraPacket) -> Vec<LoraPacket> {
        let seq = packet.seq_id;
        let now = Utc::now();

        if seq < self.lowest_seq.saturating_sub(50) {
            warn!(
                "Dropping stale packet seq={} for device {} (lowest={})",
                seq, self.device_id, self.lowest_seq
            );
            return vec![];
        }

        self.buffer.insert(
            seq,
            BufferedPacket {
                packet: packet.clone(),
                received_at: now,
            },
        );

        if seq > self.highest_seq {
            self.highest_seq = seq;
        }
        if seq < self.lowest_seq {
            self.lowest_seq = seq;
        }

        let mut result = self.flush_ready();
        result.append(&mut self.flush_expired());
        result
    }

    fn flush_ready(&mut self) -> Vec<LoraPacket> {
        let mut flushed = Vec::new();

        loop {
            let first_seq = match self.buffer.keys().next().copied() {
                Some(seq) => seq,
                None => break,
            };

            if first_seq == self.expected_seq {
                let bp = self.buffer.remove(&first_seq).unwrap();
                flushed.push(bp.packet);
                self.expected_seq += 1;
                self.lowest_seq = self.expected_seq;
            } else if first_seq > self.expected_seq {
                break;
            } else {
                self.buffer.remove(&first_seq);
            }
        }

        flushed
    }

    fn flush_expired(&mut self) -> Vec<LoraPacket> {
        let mut flushed = Vec::new();
        let now = Utc::now();
        let threshold = now - chrono::Duration::seconds(15);

        let expired_seqs: Vec<u64> = self
            .buffer
            .iter()
            .filter(|(seq, bp)| bp.received_at < threshold && **seq > self.expected_seq)
            .map(|(seq, _)| *seq)
            .collect();

        for seq in expired_seqs {
            if let Some(bp) = self.buffer.remove(&seq) {
                warn!(
                    "Expiring packet seq={} for device {} (expected={})",
                    seq, self.device_id, self.expected_seq
                );
                if seq >= self.expected_seq {
                    self.expected_seq = seq + 1;
                }
                flushed.push(bp.packet);
            }
        }

        if self.highest_seq.saturating_sub(self.lowest_seq) > 100 {
            warn!(
                "Window overflow for device {}: highest={}, lowest={}, resetting",
                self.device_id, self.highest_seq, self.lowest_seq
            );
            let all: Vec<LoraPacket> = self
                .buffer
                .values()
                .map(|bp| bp.packet.clone())
                .collect();
            flushed.extend(all);
            self.buffer.clear();
            self.expected_seq = self.highest_seq + 1;
            self.lowest_seq = self.expected_seq;
        }

        flushed
    }

    fn force_flush_all(&mut self) -> Vec<LoraPacket> {
        let mut result: Vec<LoraPacket> = self
            .buffer
            .values()
            .map(|bp| bp.packet.clone())
            .collect();
        result.sort_by_key(|p| p.seq_id);
        self.buffer.clear();
        if !result.is_empty() {
            self.expected_seq = self.highest_seq + 1;
            self.lowest_seq = self.expected_seq;
        }
        result
    }
}

pub struct LoraGateway {
    device_windows: Arc<RwLock<HashMap<String, DeviceWindow>>>,
    flush_handle: Option<tokio::task::JoinHandle<()>>,
}

impl LoraGateway {
    pub fn new() -> Self {
        let device_windows = Arc::new(RwLock::new(HashMap::new()));
        let windows_clone = device_windows.clone();

        let flush_handle = Some(tokio::spawn(async move {
            let mut ticker = interval(Duration::from_secs(5));
            loop {
                ticker.tick().await;
                let mut windows = windows_clone.write().await;
                let mut expired_any = false;
                for window in windows.values_mut() {
                    let expired = window.flush_expired();
                    if !expired.is_empty() {
                        expired_any = true;
                        debug!(
                            "Periodic flush expired {} packets for device {}",
                            expired.len(),
                            window.device_id
                        );
                    }
                }
                if expired_any {
                    debug!("Periodic flush cycle complete");
                }
            }
        }));

        Self {
            device_windows,
            flush_handle,
        }
    }

    pub async fn receive_packet(&self, packet: LoraPacket) -> Result<Vec<LoraPacket>, AppError> {
        let id = packet.device_id.clone();
        let seq = packet.seq_id;
        let mut windows = self.device_windows.write().await;
        let window = windows
            .entry(id.clone())
            .or_insert_with(|| DeviceWindow::new(id.clone(), seq));
        Ok(window.insert(packet))
    }

    pub async fn get_stats(&self) -> serde_json::Value {
        let windows = self.device_windows.read().await;
        let total_devices = windows.len();
        let devices: Vec<serde_json::Value> = windows
            .iter()
            .map(|(id, w)| {
                serde_json::json!({
                    "device_id": id,
                    "expected_seq": w.expected_seq,
                    "highest_seq": w.highest_seq,
                    "lowest_seq": w.lowest_seq,
                    "buffer_size": w.buffer.len(),
                    "window_gap": w.highest_seq.saturating_sub(w.lowest_seq),
                })
            })
            .collect();

        serde_json::json!({
            "total_devices": total_devices,
            "devices": devices,
        })
    }

    pub async fn force_flush(
        &self,
        device_id: Option<&str>,
    ) -> Result<Vec<LoraPacket>, AppError> {
        let mut windows = self.device_windows.write().await;
        let mut result = Vec::new();

        match device_id {
            Some(id) => {
                if let Some(window) = windows.get_mut(id) {
                    result = window.force_flush_all();
                }
            }
            None => {
                for window in windows.values_mut() {
                    result.append(&mut window.force_flush_all());
                }
                result.sort_by_key(|p| (p.device_id.clone(), p.seq_id));
            }
        }

        Ok(result)
    }
}

impl Default for LoraGateway {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for LoraGateway {
    fn drop(&mut self) {
        if let Some(handle) = self.flush_handle.take() {
            handle.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use crate::common::{LoraData, SoilReading};

    fn make_packet(device_id: &str, seq: u64) -> LoraPacket {
        LoraPacket {
            device_type: "soil_sensor".to_string(),
            device_id: device_id.to_string(),
            zone: "zone1".to_string(),
            seq_id: seq,
            timestamp: Utc::now(),
            data: LoraData::Soil(SoilReading {
                temperature: 25.0,
                humidity: 50.0,
                ph: 7.0,
                chloride: 100.0,
            }),
        }
    }

    #[test]
    fn test_in_order() {
        let mut window = DeviceWindow::new("dev1".to_string(), 1);
        let p1 = make_packet("dev1", 1);
        let p2 = make_packet("dev1", 2);
        let p3 = make_packet("dev1", 3);

        let r1 = window.insert(p1);
        assert_eq!(r1.len(), 1);
        assert_eq!(r1[0].seq_id, 1);

        let r2 = window.insert(p2);
        assert_eq!(r2.len(), 1);
        assert_eq!(r2[0].seq_id, 2);

        let r3 = window.insert(p3);
        assert_eq!(r3.len(), 1);
        assert_eq!(r3[0].seq_id, 3);
    }

    #[test]
    fn test_out_of_order() {
        let mut window = DeviceWindow::new("dev1".to_string(), 1);
        let p1 = make_packet("dev1", 1);
        let p2 = make_packet("dev1", 2);
        let p3 = make_packet("dev1", 3);

        let r3 = window.insert(p3);
        assert!(r3.is_empty());

        let r1 = window.insert(p1);
        assert_eq!(r1.len(), 1);
        assert_eq!(r1[0].seq_id, 1);

        let r2 = window.insert(p2);
        assert_eq!(r2.len(), 2);
        assert_eq!(r2[0].seq_id, 2);
        assert_eq!(r2[1].seq_id, 3);
    }

    #[test]
    fn test_duplicate() {
        let mut window = DeviceWindow::new("dev1".to_string(), 1);
        let p1 = make_packet("dev1", 1);
        let p1_dup = make_packet("dev1", 1);
        let p2 = make_packet("dev1", 2);

        let r1 = window.insert(p1);
        assert_eq!(r1.len(), 1);
        assert_eq!(r1[0].seq_id, 1);

        let r_dup = window.insert(p1_dup);
        assert!(r_dup.is_empty());

        let r2 = window.insert(p2);
        assert_eq!(r2.len(), 1);
        assert_eq!(r2[0].seq_id, 2);
    }

    #[test]
    fn test_gap_filling() {
        let mut window = DeviceWindow::new("dev1".to_string(), 1);
        let p1 = make_packet("dev1", 1);
        let p3 = make_packet("dev1", 3);
        let p4 = make_packet("dev1", 4);
        let p2 = make_packet("dev1", 2);

        let r1 = window.insert(p1);
        assert_eq!(r1.len(), 1);
        assert_eq!(r1[0].seq_id, 1);

        let r3 = window.insert(p3);
        assert!(r3.is_empty());

        let r4 = window.insert(p4);
        assert!(r4.is_empty());

        let r2 = window.insert(p2);
        assert_eq!(r2.len(), 3);
        assert_eq!(r2[0].seq_id, 2);
        assert_eq!(r2[1].seq_id, 3);
        assert_eq!(r2[2].seq_id, 4);
    }

    #[test]
    fn test_force_flush_all() {
        let mut window = DeviceWindow::new("dev1".to_string(), 1);
        let p3 = make_packet("dev1", 3);
        let p5 = make_packet("dev1", 5);

        window.insert(p3);
        window.insert(p5);

        let flushed = window.force_flush_all();
        assert_eq!(flushed.len(), 2);
        assert_eq!(flushed[0].seq_id, 3);
        assert_eq!(flushed[1].seq_id, 5);
        assert!(window.buffer.is_empty());
    }
}
