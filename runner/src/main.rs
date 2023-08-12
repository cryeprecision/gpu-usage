mod config;

use anyhow::Context;

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

#[tokio::main(flavor = "current_thread")]
async fn main() {
    init_logger();

    let cfg = match tokio::fs::read_to_string("./config.json").await {
        Err(err) => {
            log::error!("couldn't read config file ({})", err);
            std::process::exit(1);
        }
        Ok(v) => v,
    };
    let cfg = match serde_json::from_str::<config::Conf>(&cfg) {
        Err(err) => {
            log::error!("couldn't parse config file ({})", err);
            std::process::exit(1);
        }
        Ok(v) => v,
    };
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

        if let Some(_) = cfg.cpu_temp.as_ref() {
            let temp = cpu_temp::cpu_temp()
                .await
                .context("couldn't fetch cpu temp")
                .unwrap();
            log::info!("cpu temp: {:?}", temp);
        }
        if let Some(cfg) = cfg.gpu_usage.as_ref() {
            let usage = gpu_usage::gpu_usage(&cfg.device)
                .await
                .context("couldn't fetch gpu usage")
                .unwrap();
            log::info!("gpu usage: {:?}", usage);
        }
    }
}
