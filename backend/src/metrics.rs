use prometheus::{
    self, Histogram, HistogramOpts, HistogramVec, IntCounter, IntCounterVec, IntGauge, Opts,
    Registry, TextEncoder,
};
use std::sync::OnceLock;

fn http_requests_total() -> &'static IntCounterVec {
    static HTTP_REQUESTS_TOTAL: OnceLock<IntCounterVec> = OnceLock::new();
    HTTP_REQUESTS_TOTAL.get_or_init(|| {
        IntCounterVec::new(
            Opts::new("http_requests_total", "Total number of HTTP requests"),
            &["method", "path", "status"],
        )
        .unwrap()
    })
}

fn lora_packets_received() -> &'static IntCounter {
    static LORA_PACKETS_RECEIVED: OnceLock<IntCounter> = OnceLock::new();
    LORA_PACKETS_RECEIVED.get_or_init(|| {
        IntCounter::new("lora_packets_received", "Total number of LoRa packets received").unwrap()
    })
}

fn lora_packets_reordered() -> &'static IntCounter {
    static LORA_PACKETS_REORDERED: OnceLock<IntCounter> = OnceLock::new();
    LORA_PACKETS_REORDERED.get_or_init(|| {
        IntCounter::new("lora_packets_reordered", "Total number of reordered LoRa packets").unwrap()
    })
}

fn corrosion_alerts_total() -> &'static IntCounter {
    static CORROSION_ALERTS_TOTAL: OnceLock<IntCounter> = OnceLock::new();
    CORROSION_ALERTS_TOTAL.get_or_init(|| {
        IntCounter::new("corrosion_alerts_total", "Total number of corrosion alerts triggered")
            .unwrap()
    })
}

fn influx_write_bytes() -> &'static IntCounter {
    static INFLUX_WRITE_BYTES: OnceLock<IntCounter> = OnceLock::new();
    INFLUX_WRITE_BYTES
        .get_or_init(|| IntCounter::new("influx_write_bytes", "Total bytes written to InfluxDB").unwrap())
}

fn influx_write_batches() -> &'static IntCounter {
    static INFLUX_WRITE_BATCHES: OnceLock<IntCounter> = OnceLock::new();
    INFLUX_WRITE_BATCHES.get_or_init(|| {
        IntCounter::new("influx_write_batches", "Total number of InfluxDB write batches").unwrap()
    })
}

fn http_request_duration() -> &'static HistogramVec {
    static HTTP_REQUEST_DURATION: OnceLock<HistogramVec> = OnceLock::new();
    HTTP_REQUEST_DURATION.get_or_init(|| {
        HistogramVec::new(
            HistogramOpts::new("http_request_duration_seconds", "HTTP request duration in seconds"),
            &["method", "path"],
        )
        .unwrap()
    })
}

fn corrosion_predict_duration() -> &'static Histogram {
    static CORROSION_PREDICT_DURATION: OnceLock<Histogram> = OnceLock::new();
    CORROSION_PREDICT_DURATION.get_or_init(|| {
        Histogram::with_opts(HistogramOpts::new(
            "corrosion_predict_duration_seconds",
            "Corrosion prediction duration in seconds",
        ))
        .unwrap()
    })
}

fn active_connections() -> &'static IntGauge {
    static ACTIVE_CONNECTIONS: OnceLock<IntGauge> = OnceLock::new();
    ACTIVE_CONNECTIONS
        .get_or_init(|| IntGauge::new("active_connections", "Number of active connections").unwrap())
}

fn lora_buffer_size() -> &'static IntGauge {
    static LORA_BUFFER_SIZE: OnceLock<IntGauge> = OnceLock::new();
    LORA_BUFFER_SIZE
        .get_or_init(|| IntGauge::new("lora_buffer_size", "Current LoRa buffer size").unwrap())
}

pub fn init_metrics() {
    let registry = prometheus::default_registry();
    let _ = registry.register(Box::new(http_requests_total().clone()));
    let _ = registry.register(Box::new(lora_packets_received().clone()));
    let _ = registry.register(Box::new(lora_packets_reordered().clone()));
    let _ = registry.register(Box::new(corrosion_alerts_total().clone()));
    let _ = registry.register(Box::new(influx_write_bytes().clone()));
    let _ = registry.register(Box::new(influx_write_batches().clone()));
    let _ = registry.register(Box::new(http_request_duration().clone()));
    let _ = registry.register(Box::new(corrosion_predict_duration().clone()));
    let _ = registry.register(Box::new(active_connections().clone()));
    let _ = registry.register(Box::new(lora_buffer_size().clone()));
}

pub fn register_custom_metrics() -> Registry {
    let registry = Registry::new();
    registry.register(Box::new(http_requests_total().clone())).unwrap();
    registry.register(Box::new(lora_packets_received().clone())).unwrap();
    registry.register(Box::new(lora_packets_reordered().clone())).unwrap();
    registry.register(Box::new(corrosion_alerts_total().clone())).unwrap();
    registry.register(Box::new(influx_write_bytes().clone())).unwrap();
    registry.register(Box::new(influx_write_batches().clone())).unwrap();
    registry.register(Box::new(http_request_duration().clone())).unwrap();
    registry.register(Box::new(corrosion_predict_duration().clone())).unwrap();
    registry.register(Box::new(active_connections().clone())).unwrap();
    registry.register(Box::new(lora_buffer_size().clone())).unwrap();
    registry
}

pub fn record_request(method: &str, path: &str, status: &str, duration_secs: f64) {
    http_requests_total()
        .with_label_values(&[method, path, status])
        .inc();
    http_request_duration()
        .with_label_values(&[method, path])
        .observe(duration_secs);
}

pub fn inc_lora_packets(reordered: bool) {
    lora_packets_received().inc();
    if reordered {
        lora_packets_reordered().inc();
    }
}

pub fn set_buffer_size(size: i64) {
    lora_buffer_size().set(size);
}

pub fn inc_corrosion_alerts() {
    corrosion_alerts_total().inc();
}

pub fn inc_influx_write_bytes(bytes: i64) {
    influx_write_bytes().inc_by(bytes as u64);
}

pub fn inc_influx_write_batches() {
    influx_write_batches().inc();
}

pub fn observe_corrosion_predict_duration(duration_secs: f64) {
    corrosion_predict_duration().observe(duration_secs);
}

pub fn set_active_connections(count: i64) {
    active_connections().set(count);
}

pub fn encode_metrics() -> String {
    let encoder = TextEncoder::new();
    let metric_families = prometheus::default_registry().gather();
    let mut buffer = String::new();
    encoder.encode_utf8(&metric_families, &mut buffer).unwrap();
    buffer
}
