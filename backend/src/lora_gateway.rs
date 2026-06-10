use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use std::time::Duration;
use chrono::{DateTime, Utc};
use tokio::sync::RwLock;
use tokio::time::interval;
use tracing::{info, warn, debug};
use crate::error::AppError;
use crate::models::LoraPacket;

const WINDOW_SIZE: u64 = 100;
const FLUSH_INTERVAL_SECS: u64 = 5;
const MAX_OUT_OF_ORDER_DELTA: u64 = 50;

#[derive(Debug, Clone)]
struct BufferedPacket {
    pub packet: LoraPacket,
    pub received_at: DateTime<Utc>,
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
        DeviceWindow {
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

        if seq < self.lowest_seq.saturating_sub(MAX_OUT_OF_ORDER_DELTA) {
            warn!(
                "设备 {} 序列号 {} 过于陈旧，已丢弃 (最低: {}",
                self.device_id, seq, self.lowest_seq
            );
            return Vec::new();
        }

        self.buffer.insert(seq, BufferedPacket {
            packet: packet.clone(),
            received_at: now,
        });

        if seq > self.highest_seq {
            self.highest_seq = seq;
        }
        if seq < self.lowest_seq {
            self.lowest_seq = seq;
        }

        self.flush_ready()
    }

    fn flush_ready(&mut self) -> Vec<LoraPacket> {
        let mut flushed = Vec::new();

        while let Some(entry) = self.buffer.first_entry() {
            let seq = *entry.key();
            if seq == self.expected_seq {
                let pkt = entry.remove();
                flushed.push(pkt.packet);
                self.expected_seq += 1;
                self.lowest_seq = self.expected_seq;
            } else if seq > self.expected_seq {
                break;
            } else {
                entry.remove();
            }
        }

        self.flush_expired()
    }

    fn flush_expired(&mut self) -> Vec<LoraPacket> {
        let mut flushed = Vec::new();
        let now = Utc::now();
        let threshold = now - chrono::Duration::seconds(FLUSH_INTERVAL_SECS as i64 * 3);

        let mut to_remove = Vec::new();
        for (seq, bp) in self.buffer.iter() {
            if bp.received_at < threshold && *seq > self.expected_seq {
                to_remove.push(*seq);
            }
        }

        for seq in to_remove {
            if let Some(bp) = self.buffer.remove(&seq) {
                warn!(
                    "设备 {} 超时数据包 seq={} 已超时强制释放 (期望 seq={})",
                    self.device_id, seq, self.expected_seq
                );
                flushed.push(bp.packet);
                if seq >= self.expected_seq {
                    self.expected_seq = seq + 1;
                    self.lowest_seq = self.expected_seq;
                }
            }
        }

        if self.highest_seq.saturating_sub(self.lowest_seq) > WINDOW_SIZE {
            let gap_start = self.expected_seq;
            let gap_end = self.highest_seq;
            warn!(
                "设备 {} 滑动窗口溢出: 从 {} 到 {} 存在 {} 个丢失包，强制前移",
                self.device_id, gap_start, gap_end, gap_end.saturating_sub(gap_start)
            );
            self.expected_seq = self.highest_seq + 1;
            self.lowest_seq = self.expected_seq;
            self.buffer.clear();
        }

        flushed
    }

    fn force_flush_all(&mut self) -> Vec<LoraPacket> {
        let mut packets: Vec<LoraPacket> = self.buffer
            .values()
            .cloned()
            .map(|bp| bp.packet)
            .collect();
        packets.sort_by_key(|p| p.seq_id);
        self.buffer.clear();
        if let Some(max_seq) = packets.last().map(|p| p.seq_id) {
            self.expected_seq = max_seq + 1;
            self.lowest_seq = self.expected_seq;
            self.highest_seq = max_seq;
        }
        packets
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

        let flush_handle = tokio::spawn(async move {
            let mut ticker = interval(Duration::from_secs(FLUSH_INTERVAL_SECS));
            loop {
                ticker.tick().await;
                let mut windows = windows_clone.write().await;
                for window in windows.values_mut() {
                    let flushed = window.flush_expired();
                    if !flushed.is_empty() {
                        debug!("定时刷新设备 {} 缓冲区，释放 {} 个包", window.device_id, flushed.len());
                    }
                }
            }
        });

        LoraGateway {
            device_windows,
            flush_handle: Some(flush_handle),
        }
    }

    pub async fn receive_packet(&self, packet: LoraPacket) -> Result<Vec<LoraPacket>, AppError> {
        let device_id = packet.device_id.clone();
        let seq = packet.seq_id;

        let mut windows = self.device_windows.write().await;

        let window = windows.entry(device_id.clone())
            .or_insert_with(|| DeviceWindow::new(device_id.clone(), seq));

        let flushed = window.insert(packet);

        if !flushed.is_empty() {
            debug!(
                "设备 {} 释放 {} 个有序包 (seq: {:?})",
                device_id,
                flushed.len(),
                flushed.iter().map(|p| p.seq_id).collect::<Vec<_>>()
            );
        }

        Ok(flushed)
    }

    pub async fn get_stats(&self) -> serde_json::Value {
        let windows = self.device_windows.read().await;
        let mut stats = Vec::new();
        for (id, win) in windows.iter() {
            stats.push(serde_json::json!({
                "device_id": id,
                "expected_seq": win.expected_seq,
                "highest_seq": win.highest_seq,
                "lowest_seq": win.lowest_seq,
                "buffer_size": win.buffer.len(),
            }));
        }
        serde_json::json!({
            "total_devices": windows.len(),
            "devices": stats,
        })
    }

    pub async fn force_flush(&self, device_id: Option<&str>) -> Result<Vec<LoraPacket>, AppError> {
        let mut windows = self.device_windows.write().await;
        let mut all_flushed = Vec::new();

        if let Some(id) = device_id {
            if let Some(window) = windows.get(id) {
                all_flushed.extend(window.force_flush_all());
            }
        } else {
            for window in windows.values() {
                all_flushed.extend(window.force_flush_all());
            }
        }

        Ok(all_flushed)
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
    use crate::models::LoraData;
    use crate::models::SoilReading;

    fn make_packet(seq: u64) -> LoraPacket {
        LoraPacket {
            device_type: "soil_sensor".to_string(),
            device_id: "TEST-001".to_string(),
            zone: "区域-1".to_string(),
            seq_id: seq,
            timestamp: Utc::now(),
            data: LoraData::Soil(SoilReading {
                temperature: 15.0,
                humidity: 50.0,
                ph: 7.0,
                chloride: 30.0,
            }),
        }
    }

    #[test]
    fn test_in_order_delivery() {
        let mut window = DeviceWindow::new("TEST".to_string(), 1);
        let flushed1 = window.insert(make_packet(1));
        assert_eq!(flushed1.len(), 1);
        assert_eq!(flushed1[0].seq_id, 1);
        assert_eq!(window.expected_seq, 2);

        let flushed2 = window.insert(make_packet(2));
        assert_eq!(flushed2.len(), 1);
        assert_eq!(flushed2[0].seq_id, 2);
    }

    #[test]
    fn test_out_of_order_reordering() {
        let mut window = DeviceWindow::new("TEST".to_string(), 1);

        let flushed3 = window.insert(make_packet(3));
        assert_eq!(flushed3.len(), 0);
        assert_eq!(window.expected_seq, 1);

        let flushed1 = window.insert(make_packet(1));
        assert_eq!(flushed1.len(), 1);
        assert_eq!(flushed1[0].seq_id, 1);
        assert_eq!(window.expected_seq, 2);

        let flushed2 = window.insert(make_packet(2));
        assert_eq!(flushed2.len(), 2);
        assert_eq!(flushed2[0].seq_id, 2);
        assert_eq!(flushed2[1].seq_id, 3);
    }

    #[test]
    fn test_duplicate_packet_dropped() {
        let mut window = DeviceWindow::new("TEST".to_string(), 1);
        window.insert(make_packet(1));
        let flushed = window.insert(make_packet(1));
        assert_eq!(flushed.len(), 0);
    }

    #[test]
    fn test_gap_filling() {
        let mut window = DeviceWindow::new("TEST".to_string(), 1);

        window.insert(make_packet(1));
        window.insert(make_packet(4));

        let flushed = window.insert(make_packet(2));
        assert_eq!(flushed.len(), 2);
        assert_eq!(flushed[0].seq_id, 2);
        assert_eq!(window.expected_seq, 3);

        let flushed3 = window.insert(make_packet(3));
        assert_eq!(flushed3.len(), 2);
        assert_eq!(flushed3[0].seq_id, 3);
        assert_eq!(flushed3[1].seq_id, 4);
    }

    #[test]
    fn test_force_flush() {
        let mut window = DeviceWindow::new("TEST".to_string(), 1);
        window.insert(make_packet(1));
        window.insert(make_packet(3));
        window.insert(make_packet(5));
        assert_eq!(window.buffer.len(), 2);

        let flushed = window.force_flush_all();
        assert_eq!(flushed.len(), 2);
        assert_eq!(flushed[0].seq_id, 3);
        assert_eq!(flushed[1].seq_id, 5);
    }

    #[tokio::test]
    async fn test_gateway_basic() {
        let gateway = LoraGateway::new();
        let flushed1 = gateway.receive_packet(make_packet(1)).await.unwrap();
        assert_eq!(flushed1.len(), 1);
        assert_eq!(flushed1[0].seq_id, 1);
    }
}
