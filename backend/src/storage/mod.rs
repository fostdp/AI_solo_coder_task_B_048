pub mod writer;
pub mod reader;

pub use writer::BatchWriter;
pub use reader::StorageReader;

use crate::common::*;

pub struct StorageService {
    pub writer: BatchWriter,
    pub reader: StorageReader,
}

impl StorageService {
    pub fn new(config: &AppConfig) -> Self {
        let client = influxdb::Client::new(&config.influxdb.url, &config.influxdb.org);
        let client = client.with_token(&config.influxdb.token);

        let writer = BatchWriter::new(
            client.clone(),
            config.influxdb.bucket.clone(),
            config.influxdb.batch_size,
            config.influxdb.flush_interval_ms,
            config.influxdb.max_pending,
        );

        let reader = StorageReader::new(client, config.clone());

        Self { writer, reader }
    }
}
