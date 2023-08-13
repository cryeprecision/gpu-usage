mod bins;
mod config;
mod influx;
mod json_ptr;

use std::pin::Pin;

use anyhow::Context;
use futures::Future;
use influxdb2::models::DataPoint;
use structopt::StructOpt;
use tokio::time::{Duration, MissedTickBehavior};

use crate::config::{Conf, IntelGpuTopConf, SensorsConf};
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
        self.sum / self.n as f64
    }

    /// Add another value to the average
    pub fn add(&mut self, val: f64) {
        self.sum += val;
        self.n += 1;
    }
}

async fn sample_sensors(cfg: &SensorsConf, avgs: &mut [Avg]) {
    assert_eq!(avgs.len(), cfg.values.len());

    let start = std::time::Instant::now();

    let sensors = bins::sensors()
        .await
        .context("couldn't sample sensors")
        .unwrap();

    let pairs = cfg.values.iter().zip(avgs.iter_mut());
    pairs.for_each(|(map, avg)| {
        avg.add(map.path.get_f64(&sensors).unwrap());
    });

    let elapsed_ms = start.elapsed().as_secs_f64() * 1e3;
    log::info!("sensors sample took: {:.1}ms", elapsed_ms);
}
async fn sample_sensors_n(cfg: &SensorsConf, avgs: &mut [Avg], n: usize) {
    for _ in 0..n {
        sample_sensors(cfg, avgs).await;
    }
}

async fn sample_intel_gpu_top(cfg: &IntelGpuTopConf, avgs: &mut [Avg]) {
    assert_eq!(avgs.len(), cfg.values.len());

    let start = std::time::Instant::now();

    let intel_gpu_top = bins::intel_gpu_top(&cfg.device)
        .await
        .context("couldn't sample intel_gpu_top")
        .unwrap();

    let pairs = cfg.values.iter().zip(avgs.iter_mut());
    pairs.for_each(|(map, avg)| {
        avg.add(map.path.get_f64(&intel_gpu_top).unwrap());
    });

    let elapsed_ms = start.elapsed().as_secs_f64() * 1e3;
    log::info!("intel_gpu_top sample took: {:.1}ms", elapsed_ms);
}
async fn sample_intel_gpu_top_n(cfg: &IntelGpuTopConf, avgs: &mut [Avg], n: usize) {
    for _ in 0..n {
        sample_intel_gpu_top(cfg, avgs).await;
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
        Ok(v) => v,
    };

    // Check for invalid configuration
    if !(cfg.sensors.enabled || cfg.intel_gpu_top.enabled) {
        log::error!("all collectors disabled in config");
        std::process::exit(1);
    }

    // Create a connection to InfluxDB
    let influx = if let Some(cfg) = cfg.influx.as_ref() {
        Some(Influx::new(&cfg.host, &cfg.org, &cfg.token, &cfg.bucket))
    } else {
        log::warn!("not connecting to db");
        None
    };

    // Setup the timer to sample with the correct amount of delay
    let mut ticker = tokio::time::interval(Duration::from_millis(cfg.sample_interval_ms));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

    loop {
        ticker.tick().await;

        // Boxed future that returns a data point over sampled values
        type BoxFuture = Pin<Box<dyn Future<Output = DataPoint>>>;
        // List of futures that are actually needed, if the
        let mut futures = Vec::<BoxFuture>::with_capacity(BIN_COUNT);

        if cfg.sensors.enabled {
            futures.push(Box::pin(async {
                let cfg = &cfg.sensors;

                let mut avgs = vec![Avg::new(); cfg.values.len()];
                sample_sensors_n(cfg, &mut avgs, cfg.values.len()).await;

                let mut point = DataPoint::builder("sensors");
                for tag in cfg.tags.iter() {
                    point = point.tag(tag.name.clone(), tag.value.clone());
                }

                let pairs = avgs.into_iter().zip(cfg.values.iter().map(|map| &map.name));
                for (avg, name) in pairs {
                    point = point.field(name.clone(), avg.eval());
                }

                point.build().unwrap()
            }));
        }

        if cfg.intel_gpu_top.enabled {
            futures.push(Box::pin(async {
                let cfg = &cfg.intel_gpu_top;

                let mut avgs = vec![Avg::new(); cfg.values.len()];
                sample_intel_gpu_top_n(cfg, &mut avgs, cfg.values.len()).await;

                let mut point = DataPoint::builder("intel_gpu_top");
                for tag in cfg.tags.iter() {
                    point = point.tag(tag.name.clone(), tag.value.clone());
                }

                let pairs = avgs.into_iter().zip(cfg.values.iter().map(|map| &map.name));
                for (avg, name) in pairs {
                    point = point.field(name.clone(), avg.eval());
                }

                point.build().unwrap()
            }));
        }

        let results = futures::future::join_all(futures).await;
        if let Some(influx) = influx.as_ref() {
            influx.write_points(results).await.unwrap();
        }
    }
}
