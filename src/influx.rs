use anyhow::Result;
use influxdb2::{models::DataPoint, Client};

pub struct Influx {
    client: Client,
    bucket: String,
}

impl Influx {
    pub fn new(host: &str, org: &str, token: &str, bucket: &str) -> Influx {
        Influx {
            client: Client::new(host, org, token),
            bucket: bucket.to_string(),
        }
    }
    pub async fn write_points(&self, points: Vec<DataPoint>) -> Result<()> {
        Ok(self
            .client
            .write(&self.bucket, futures::stream::iter(points))
            .await?)
    }
}
