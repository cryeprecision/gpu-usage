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

    let mut ticker = tokio::time::interval(tokio::time::Duration::from_millis(1000));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    loop {
        ticker.tick().await;

        let usage = gpu_usage::gpu_usage("pci:card=1").await.unwrap();
        log::info!("gpu usage: {}", serde_json::to_string(&usage).unwrap());
    }
}
