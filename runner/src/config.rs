use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct InfluxConf {
    /// Host where InfluxDB is running.
    pub host: String,

    /// Organisation for data insertion.
    pub org: String,

    /// Bucket for data insertion.
    pub bucket: String,

    /// Token for data insertion.
    pub token: String,
}

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct GpuUsageConf {
    pub device: String,
}

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct CpuTempConf {}

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct Conf {
    /// Enable collecting and submitting CPU temperature data.
    pub cpu_temp: Option<CpuTempConf>,

    /// Enable collecting and submitting GPU usage data.
    pub gpu_usage: Option<GpuUsageConf>,

    /// Delay between each run.
    pub interval_ms: u64,

    /// Connection to InfluxDB
    pub influx: Option<InfluxConf>,
}

impl Conf {
    pub async fn load(path: &str) -> Result<Conf> {
        pub fn inner(path: &str) -> Result<Conf> {
            let mut reader = std::io::BufReader::new(
                std::fs::OpenOptions::new()
                    .read(true)
                    .open(path)
                    .context("couldn't open config file for reading")?,
            );
            Ok(serde_json::from_reader(&mut reader).context("couldn't deserialize config")?)
        }
        let path = path.to_string();
        tokio::task::spawn_blocking(move || inner(&path))
            .await
            .unwrap()
    }
}
