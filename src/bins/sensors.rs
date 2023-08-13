use anyhow::{Context, Result};
use serde_json::Value;
use std::process::Stdio;

const BIN: &str = "sensors";

/// Run the `sensors` command and capture the JSON output
pub async fn sensors() -> Result<Value> {
    let args: [&str; 1] = ["-j"];

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

    Ok(json_val)
}
