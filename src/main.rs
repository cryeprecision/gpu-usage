mod bins;
mod config;
mod influx;
mod json_ptr;

use std::pin::Pin;

use anyhow::Result;
use async_channel::Receiver;
use futures::Future;
use influxdb2::models::DataPoint;
use serde_json::Value;
use structopt::StructOpt;
use tokio::task::JoinHandle;

use crate::config::Conf;
use crate::influx::Influx;

/// How many binaries are supported
const BIN_COUNT: usize = 2;

#[derive(Debug, StructOpt)]
#[structopt(name = "Logger")]
#[structopt(about = "Log temperature and usage stats to InfluxDB2.")]
struct Opt {
    /// Path to the config file (JSON)
    #[structopt(long)]
    #[structopt(default_value = "./config.json")]
    config: String,
}

fn init_logger() {
    use log::LevelFilter;
    use simplelog::{ColorChoice, ConfigBuilder, TermLogger, TerminalMode};

    TermLogger::init(
        LevelFilter::Info,
        ConfigBuilder::default()
            .set_time_format_rfc2822()
            .set_target_level(LevelFilter::Info)
            .set_time_offset_to_local()
            .unwrap()
            .build(),
        TerminalMode::Mixed,
        ColorChoice::Auto,
    )
    .unwrap();
}

#[derive(Debug, Clone, Copy)]
struct Avg {
    /// Sum of all values
    sum: f64,

    /// How many values were collected
    n: usize,
}

impl Avg {
    /// Create an empty average
    pub fn new() -> Avg {
        Avg { sum: 0.0, n: 0 }
    }

    /// Evaluate the average
    pub fn eval(self) -> f64 {
        self.sum / self.n.max(1) as f64
    }

    /// Add another value to the average
    pub fn add(&mut self, val: f64) {
        self.sum += val;
        self.n += 1;
    }
}

struct BackgroundJob {
    /// Handle to the future
    pub _handle: JoinHandle<Result<()>>,

    /// Receiver to collect the results
    pub rx: Receiver<Value>,
}
impl BackgroundJob {
    pub fn new(handle: JoinHandle<Result<()>>, rx: Receiver<Value>) -> BackgroundJob {
        BackgroundJob {
            _handle: handle,
            rx,
        }
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    init_logger();

    // Parse command line arguments
    let opts = Opt::from_args();

    // Load the JSON config
    let cfg = match Conf::load(&opts.config).await {
        Err(err) => {
            log::error!("couldn't load config file: {}", err);
            std::process::exit(1);
        }
        Ok(v) => match v.validate() {
            Some(err) => {
                log::error!("invalid config: {}", err);
                std::process::exit(1);
            }
            None => v,
        },
    };

    // Create a connection to InfluxDB
    let influx = if let Some(cfg) = cfg.influx.as_ref() {
        Some(Influx::new(&cfg.host, &cfg.org, &cfg.token, &cfg.bucket))
    } else {
        log::warn!("not connecting to db");
        None
    };

    let intel_gpu_top = cfg.intel_gpu_top.enabled.then(|| {
        let (tx, rx) = async_channel::unbounded();
        let handle = tokio::spawn(bins::intel_gpu_top_log(
            tx,
            cfg.sample_interval_ms,
            cfg.intel_gpu_top.device.clone(),
        ));
        BackgroundJob::new(handle, rx)
    });
    let sensors = cfg.sensors.enabled.then(|| {
        let (tx, rx) = async_channel::unbounded();
        let handle = tokio::spawn(bins::sensors(tx, cfg.sample_interval_ms));
        BackgroundJob::new(handle, rx)
    });

    loop {
        // Boxed future that returns a data point over sampled values
        type BoxFuture = Pin<Box<dyn Future<Output = DataPoint>>>;
        // List of futures that are actually needed.
        // If something is disabled, we won't generate a future for it.
        let mut futures = Vec::<BoxFuture>::with_capacity(BIN_COUNT);

        if let Some(job) = sensors.as_ref() {
            futures.push(Box::pin(async {
                let sample_count = cfg.sample_count;
                let cfg = &cfg.sensors;

                let mut avgs = vec![Avg::new(); cfg.values.len()];

                // Wait for enough samples from sensor output
                for i in 0..sample_count {
                    let val = job.rx.recv().await.unwrap();
                    log::info!("received sensors sample {}", i);
                    let pairs = cfg.values.iter().zip(avgs.iter_mut());
                    pairs.for_each(|(map, avg)| {
                        avg.add(map.path.get_f64(&val).unwrap());
                    });
                }

                // Prepare a data point with measurement and tags
                let mut point = DataPoint::builder("sensors");
                for tag in cfg.tags.iter() {
                    point = point.tag(tag.name.clone(), tag.value.clone());
                }

                // Insert all values into the data point
                let pairs = avgs.into_iter().zip(cfg.values.iter().map(|map| &map.name));
                for (avg, name) in pairs {
                    point = point.field(name.clone(), avg.eval());
                }

                log::info!("built sensors point");
                point.build().unwrap()
            }));
        }

        if let Some(job) = intel_gpu_top.as_ref() {
            futures.push(Box::pin(async {
                let sample_count = cfg.sample_count;
                let cfg = &cfg.intel_gpu_top;

                let mut avgs = vec![Avg::new(); cfg.values.len()];

                // Wait for enough samples from intel_gpu_top output
                for i in 0..sample_count {
                    let val = job.rx.recv().await.unwrap();
                    log::info!("received intel_gpu_top sample {}", i);
                    let pairs = cfg.values.iter().zip(avgs.iter_mut());
                    pairs.for_each(|(map, avg)| {
                        avg.add(map.path.get_f64(&val).unwrap());
                    });
                }

                // Prepare a data point with measurement and tags
                let mut point = DataPoint::builder("intel_gpu_top");
                for tag in cfg.tags.iter() {
                    point = point.tag(tag.name.clone(), tag.value.clone());
                }

                // Insert all values into the data point
                let pairs = avgs.into_iter().zip(cfg.values.iter().map(|map| &map.name));
                for (avg, name) in pairs {
                    point = point.field(name.clone(), avg.eval());
                }

                log::info!("built intel_gpu_top point");
                point.build().unwrap()
            }));
        }

        // Wait for all the jobs to collect enough samples
        let results = futures::future::join_all(futures).await;
        if let Some(influx) = influx.as_ref() {
            influx.write_points(results).await.unwrap();
            log::info!("wrote points to database");
        }
    }
}
