mod config;
mod influx;
mod json_ptr;

use anyhow::Context;
use config::{CpuTempConf, GpuUsageConf};
use influx::Influx;
use influxdb2::models::DataPoint;
use structopt::StructOpt;

use crate::config::Conf;

#[derive(Debug, StructOpt)]
#[structopt(name = "Logger")]
#[structopt(about = "Log temperature and usage stats to InfluxDB2.")]
struct Opt {
    /// Path to the config file (JSON)
    #[structopt(long)]
    #[structopt(default_value = "./config.json")]
    config: String,
}

pub fn init_logger() {
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

pub async fn run_cpu(cfg: &CpuTempConf) -> DataPoint {
    let temp = cpu_temp::cpu_temp()
        .await
        .context("couldn't fetch cpu temp")
        .unwrap();

    cfg.values.iter().for_each(|mapping| {
        log::info!(
            "[temp] {}: {}",
            mapping.name,
            mapping.path.get_f64(&temp).unwrap(),
        );
    });

    let mut builder = DataPoint::builder("temperature").tag("server", "proxmox");
    for map in &cfg.values {
        builder = builder.field(map.name.clone(), map.path.get_f64(&temp).unwrap())
    }
    builder.build().unwrap()
}

pub async fn run_gpu(cfg: &GpuUsageConf) -> DataPoint {
    let usage = gpu_usage::gpu_usage(&cfg.device)
        .await
        .context("couldn't fetch gpu usage")
        .unwrap();

    cfg.values.iter().for_each(|mapping| {
        log::info!(
            "[gpu] {}: {}",
            mapping.name,
            mapping.path.get_f64(&usage).unwrap()
        )
    });

    let mut builder = DataPoint::builder("gpu_usage").tag("server", "jellyfin");
    for map in &cfg.values {
        builder = builder.field(map.name.clone(), map.path.get_f64(&usage).unwrap())
    }
    builder.build().unwrap()
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    init_logger();

    let opts = Opt::from_args();

    let cfg = match Conf::load(&opts.config).await {
        Err(err) => {
            log::error!("couldn't load config file: {}", err);
            std::process::exit(1);
        }
        Ok(v) => v,
    };

    if !(cfg.cpu_temp.as_ref().map_or(false, |c| c.enabled)
        || cfg.gpu_usage.as_ref().map_or(false, |c| c.enabled))
    {
        log::error!("all collectors disabled in config");
        std::process::exit(1);
    }

    let influx = if let Some(cfg) = cfg.influx.as_ref() {
        Some(Influx::new(&cfg.host, &cfg.org, &cfg.token, &cfg.bucket))
    } else {
        log::warn!("not connecting to db");
        None
    };

    let mut ticker = tokio::time::interval(tokio::time::Duration::from_millis(cfg.interval_ms));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    loop {
        ticker.tick().await;

        let mut point_buffer = Vec::with_capacity(2);
        if cfg.cpu_temp.as_ref().map_or(false, |c| c.enabled) {
            point_buffer.push(run_cpu(cfg.cpu_temp.as_ref().unwrap()).await);
        }
        if cfg.gpu_usage.as_ref().map_or(false, |c| c.enabled) {
            point_buffer.push(run_gpu(cfg.gpu_usage.as_ref().unwrap()).await);
        }

        if let Some(influx) = influx.as_ref() {
            influx.write_points(point_buffer).await.unwrap();
        }
    }
}
