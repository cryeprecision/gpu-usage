use anyhow::{bail, Context, Result};
use serde_json::Value;
use std::process::Stdio;

const BIN: &str = "sensors";

pub async fn cpu_temp_version() -> Result<String> {
    let args: [&str; 1] = ["-v"];

    let child = tokio::process::Command::new(BIN)
        .args(args)
        .stderr(Stdio::null())
        .stdout(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .context("couldn't spawn child process")?;

    let output = child
        .wait_with_output()
        .await
        .context("couldn't wait for child process to finish")?;

    match std::str::from_utf8(&output.stdout) {
        Ok(stdout) => Ok(stdout.to_string()),
        Err(_) => bail!("child stdout is invalid utf8"),
    }
}

pub async fn cpu_temp() -> Result<Value> {
    let args: [&str; 1] = ["-j"];

    let start = std::time::Instant::now();

    let child = tokio::process::Command::new(BIN)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .context("couldn't spawn child process")?;

    let output = child
        .wait_with_output()
        .await
        .context("couldn't wait for child process to finish")?;

    let json_val = serde_json::from_slice(&output.stdout)
        .context("couldn't parse child process stdout as json")?;

    let elapsed_ms = start.elapsed().as_secs_f64() * 1e3;
    log::info!("elapsed: {:.1}ms", elapsed_ms);

    Ok(json_val)
}
