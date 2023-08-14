use serde_json::Value;
use std::{process::Stdio, time::Duration};
use tokio::{io::AsyncReadExt, process::Command};

const BIN: &str = "intel_gpu_top";
const BUFFER_LEN: usize = 64;
const DELAY_MS: &str = "1000";

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

#[tokio::main(flavor = "current_thread")]
async fn main() {
    init_logger();

    let args: [&str; 3] = ["-s", DELAY_MS, "-J"];

    let mut child = Command::new(BIN)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap();

    log::warn!(
        "cmd: `{}`",
        args.iter().fold(BIN.to_string(), |mut acc, &next| {
            acc.push(' ');
            acc.push_str(next);
            acc
        })
    );

    let mut stdout = child.stdout.take().unwrap();

    let mut read_buf = vec![0u8; BUFFER_LEN];
    let mut json_buf = Vec::<u8>::with_capacity(BUFFER_LEN);

    loop {
        let mut bytes_read = 0usize;
        let mut iterations = 0usize;

        loop {
            let stdout_bytes_read = stdout.read(&mut read_buf).await.unwrap();
            bytes_read += stdout_bytes_read;
            iterations += 1;

            log::warn!("read {} bytes in it {}", bytes_read, iterations);

            if stdout_bytes_read == 0 {
                break;
            } else {
                json_buf.extend_from_slice(&read_buf[0..stdout_bytes_read]);
            }
        }

        log::warn!("read {} bytes in {} its", bytes_read, iterations);
        tokio::time::sleep(Duration::from_millis(100)).await;

        // try to parse it as json to see if the output is complete
        match serde_json::from_slice::<Value>(&json_buf) {
            // not a complete json object yet
            Err(_) => {
                log::warn!("not a complete object yet");
                continue;
            }
            // full json object read, stop parsing
            Ok(val) => {
                log::warn!("got a complete object: {:?}", val);
                json_buf.clear();
            }
        }
    }
}
