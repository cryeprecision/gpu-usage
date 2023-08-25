mod bins;
mod config;
mod influx;
mod json_ptr;
mod logger;

use std::pin::Pin;

use async_channel::Receiver;
use config::{Tag, ValueMapping};
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

struct BackgroundJob<T> {
    /// Handle to the future
    pub _handle: JoinHandle<T>,

    /// Receiver to collect the results
    pub rx: Receiver<Value>,
}

impl<T> BackgroundJob<T> {
    // pub fn spawn<T>(future: T) -> JoinHandle<T::Output>
    // where
    //     T: Future + Send + 'static,
    //     T::Output: Send + 'static,

    pub fn spawn<U>(future: U, rx: Receiver<Value>) -> BackgroundJob<T>
    where
        T: Send + 'static,
        U: Future<Output = T> + Send + 'static,
    {
        BackgroundJob {
            _handle: tokio::task::spawn(future),
            rx,
        }
    }
}

/// Push `n` samples from `rx` into `avgs`
async fn _sample_n(rx: Receiver<Value>, values: &[ValueMapping], avgs: &mut [Avg], n: usize) {
    for _ in 0..n {
        let val = rx.recv().await.unwrap();
        let pairs = values.iter().zip(avgs.iter_mut());
        pairs.for_each(|(map, avg)| {
            avg.add(map.path.get_f64(&val).unwrap());
        });
    }
}

/// Build a data point from the `avgs` with `tags`, `values` and `measurement`.
fn _build_point(measurement: &str, tags: &[Tag], values: &[ValueMapping], avgs: &[Avg]) {
    // Prepare a data point with measurement and tags
    let mut point = DataPoint::builder(measurement);
    for tag in tags.iter() {
        point = point.tag(tag.name.clone(), tag.value.clone());
    }

    // Insert all values into the data point
    let pairs = avgs.iter().zip(values.iter().map(|map| &map.name));
    for (avg, name) in pairs {
        point = point.field(name.clone(), avg.eval());
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    if let Err(err) = logger::init_logger() {
        log::error!("couldn't init logger: {}", err);
        std::process::exit(1);
    }

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
        let device = cfg.intel_gpu_top.device.clone();
        let future = bins::intel_gpu_top(tx, cfg.sample_interval_ms, device);
        BackgroundJob::spawn(future, rx)
    });
    let sensors = cfg.sensors.enabled.then(|| {
        let (tx, rx) = async_channel::unbounded();
        BackgroundJob::spawn(bins::sensors(tx, cfg.sample_interval_ms), rx)
    });

    // Spawn a future for each 'job', the jobs each sample with their own interval.
    // After all jobs have finished, the resulting points will be pushed to the db.
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
            match influx.write_points(results).await {
                Ok(_) => log::info!("wrote points to database"),
                Err(err) => log::warn!("couldn't write point to db: {}", err),
            }
        }
    }
}
