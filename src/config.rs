use anyhow::{Context, Result};
use serde::Deserialize;

use crate::json_ptr::JsonPtr;

#[derive(Deserialize, Debug, Default)]
pub struct ValueMapping {
    /// Name of the value
    pub name: String,

    /// Path to the value (see [`JsonPtr`])
    #[serde(deserialize_with = "JsonPtr::deserialize")]
    pub path: JsonPtr,
}

#[derive(Deserialize, Debug, Default)]
pub struct Tag {
    /// Tag name
    pub name: String,

    /// Tag value
    pub value: String,
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
pub struct IntelGpuTopConf {
    /// Enable or disable collecting GPU usage
    pub enabled: bool,

    /// Tags to use when writing to Influx
    pub tags: Vec<Tag>,

    /// Device identifier (see `inte_gpu_top -h`)
    pub device: String,

    /// List of values to extract
    pub values: Vec<ValueMapping>,
}

#[derive(Deserialize, Default, Debug)]
pub struct SensorsConf {
    /// Enable or disable collecting temperatures
    pub enabled: bool,

    /// Tags to use when writing to Influx
    pub tags: Vec<Tag>,

    /// List of values to extract
    pub values: Vec<ValueMapping>,
}

#[derive(Deserialize, Default, Debug)]
pub struct Conf {
    /// Delay between each sample.
    pub sample_interval_ms: u64,

    /// Number of samples to average over before sending to Influx.
    pub sample_count: u64,

    /// Connection to InfluxDB
    pub influx: Option<InfluxConf>,

    /// Enable collecting and submitting CPU temperature data.
    pub sensors: SensorsConf,

    /// Enable collecting and submitting GPU usage data.
    pub intel_gpu_top: IntelGpuTopConf,
}

impl Conf {
    /// Load the config from the specified path.
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

    /// Check for invalid configuration.
    ///
    /// Returns a error message if the config is invalid.
    pub fn validate(&self) -> Option<&'static str> {
        if !(self.sensors.enabled || self.intel_gpu_top.enabled) {
            Some("all collectors disabled")
        } else if self.sample_count == 0 {
            Some("sample count is zero")
        } else if self.sample_interval_ms == 0 {
            Some("sample interval is zero")
        } else {
            None
        }
    }
}
