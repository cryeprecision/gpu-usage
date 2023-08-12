use anyhow::{Context, Result};
use serde::Deserialize;

use crate::json_ptr::JsonPtr;

#[derive(Deserialize, Debug, Default)]
pub struct ValueMapping {
    /// Name of the value
    pub name: String,

    /// Path to the value (see [`JsonPtr`])
    #[serde(deserialize_with = "JsonPtr::deserialize")]
    pub pointer: JsonPtr,
}

#[derive(Deserialize, Default, Debug)]
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

#[derive(Deserialize, Default, Debug)]
pub struct GpuUsageConf {
    /// Enable or disable collecting GPU usage
    pub enabled: bool,

    /// Device identifier (see `inte_gpu_top -h`)
    pub device: String,

    /// List of values to extract
    pub values: Vec<ValueMapping>,
}

#[derive(Deserialize, Default, Debug)]
pub struct CpuTempConf {
    /// Enable or disable collecting temperatures
    pub enabled: bool,

    /// List of values to extract
    pub values: Vec<ValueMapping>,
}

#[derive(Deserialize, Default, Debug)]
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
            serde_json::from_reader(&mut reader).context("couldn't deserialize config")
        }
        let path = path.to_string();
        tokio::task::spawn_blocking(move || inner(&path))
            .await
            .unwrap()
    }
}
