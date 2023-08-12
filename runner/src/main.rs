mod config;
mod json_ptr;

use anyhow::Context;
use config::{CpuTempConf, GpuUsageConf};

use crate::config::Conf;

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

pub async fn run_cpu(cfg: &CpuTempConf) {
    let temp = cpu_temp::cpu_temp()
        .await
        .context("couldn't fetch cpu temp")
        .unwrap();
    log::info!("cpu temp: {:#?}", temp);

    cfg.values.iter().for_each(|mapping| {
        log::info!(
            "[temp] {}: {}",
            mapping.name,
            mapping.path.get_f64(&temp).unwrap(),
        );
    });
}
pub async fn run_gpu(cfg: &GpuUsageConf) {
    let usage = gpu_usage::gpu_usage(&cfg.device)
        .await
        .context("couldn't fetch gpu usage")
        .unwrap();
    log::info!("gpu usage: {:#?}", usage);

    cfg.values.iter().for_each(|mapping| {
        log::info!(
            "[gpu] {}: {}",
            mapping.name,
            mapping.path.get_f64(&usage).unwrap()
        )
    })
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    init_logger();

    let cfg = Conf::load("./config.json").await.unwrap();
    log::info!("cfg: {:#?}", cfg);

    if !(cfg.cpu_temp.is_some() || cfg.gpu_usage.is_some()) {
        log::error!("nothing to collect");
        std::process::exit(1);
    }
    if cfg.influx.is_none() {
        log::warn!("not connecting to db");
    }

    let mut ticker = tokio::time::interval(tokio::time::Duration::from_millis(2000));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    log::info!(
        "cpu temp version: `{}`",
        cpu_temp::cpu_temp_version()
            .await
            .context("couldn't get cpu_temp version")
            .unwrap()
    );

    loop {
        ticker.tick().await;

        if cfg.cpu_temp.as_ref().map_or(false, |c| c.enabled) {
            run_cpu(cfg.cpu_temp.as_ref().unwrap()).await;
        }
        if cfg.gpu_usage.as_ref().map_or(false, |c| c.enabled) {
            run_gpu(cfg.gpu_usage.as_ref().unwrap()).await;
        }
    }
}
