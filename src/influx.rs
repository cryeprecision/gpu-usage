use anyhow::Result;
use influxdb2::{models::DataPoint, Client, ClientBuilder};
use reqwest::tls::Version;

pub struct Influx {
    client: Client,
    bucket: String,
}

impl Influx {
    pub fn new(host: &str, org: &str, token: &str, bucket: &str) -> Influx {
        let reqwest = reqwest::Client::builder()
            .min_tls_version(Version::TLS_1_2)
            .danger_accept_invalid_certs(true);

        Influx {
            client: ClientBuilder::with_builder(reqwest, host, org, token)
                .build()
                .expect("valid influx client"),
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
