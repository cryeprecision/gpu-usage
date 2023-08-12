mod config;

use anyhow::Context;
use serde_json::Value;

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

        if let Some(_cfg) = cfg.cpu_temp.as_ref() {
            let temp = cpu_temp::cpu_temp()
                .await
                .context("couldn't fetch cpu temp")
                .unwrap();
            log::info!("cpu temp: {:#?}", temp);

            #[derive(Debug)]
            struct Temps {
                nvme_composite: f64,
                nvme_1: f64,
                nvme_2: f64,
                acpi: f64,
                cpu_package: f64,
                cpu_0: f64,
                cpu_1: f64,
                cpu_2: f64,
                cpu_3: f64,
            }

            fn get_temp(val: &Value, ptr: &str) -> f64 {
                val.pointer(ptr)
                    .with_context(|| format!("couldn't find path to value `{}`", ptr))
                    .unwrap()
                    .as_f64()
                    .context("couldn't parse value as f64")
                    .unwrap()
            }

            let temps = Temps {
                nvme_composite: get_temp(&temp, "nvme-pci-0100/Composite/temp1_input"),
                nvme_1: get_temp(&temp, "nvme-pci-0100/Sensor 1/temp2_input"),
                nvme_2: get_temp(&temp, "nvme-pci-0100/Sensor 2/temp3_input"),
                acpi: get_temp(&temp, "acpitz-acpi-0/temp1/temp1_input"),
                cpu_package: get_temp(&temp, "coretemp-isa-0000/Package id 0/temp1_input"),
                cpu_0: get_temp(&temp, "coretemp-isa-0000/Core 0/temp2_input"),
                cpu_1: get_temp(&temp, "coretemp-isa-0000/Core 1/temp3_input"),
                cpu_2: get_temp(&temp, "coretemp-isa-0000/Core 2/temp4_input"),
                cpu_3: get_temp(&temp, "coretemp-isa-0000/Core 3/temp5_input"),
            };

            log::info!("cpu temps: {:#?}", temps);
        }
        if let Some(cfg) = cfg.gpu_usage.as_ref() {
            let usage = gpu_usage::gpu_usage(&cfg.device)
                .await
                .context("couldn't fetch gpu usage")
                .unwrap();
            log::info!("gpu usage: {:#?}", usage);
        }
    }
}
