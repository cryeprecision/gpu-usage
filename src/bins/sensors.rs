use anyhow::{anyhow, Context, Result};
use async_channel::Sender;
use serde_json::Value;
use std::process::Stdio;
use std::time::Duration;
use tokio::time::MissedTickBehavior;

const BIN: &str = "sensors";

/// Run the `sensors` command and capture the JSON output
pub async fn sensors(tx: Sender<Value>, interval_ms: u64) -> Result<()> {
    let args: [&str; 1] = ["-j"];

    let mut ticker = tokio::time::interval(Duration::from_millis(interval_ms));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

    loop {
        // Wait for the next interval
        ticker.tick().await;

        let child = tokio::process::Command::new(BIN)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .context("couldn't spawn child process")?;

        // Wait for the child to run to completion and capture its output
        let output = child
            .wait_with_output()
            .await
            .context("couldn't wait for child process to finish")?;

        // Check if child exited with error
        if !output.status.success() {
            if let Ok(stderr) = std::str::from_utf8(&output.stderr) {
                log::error!("child stderr: {}", stderr);
            }
            return Err(anyhow!("child exited with error"));
        }

        let json = serde_json::from_slice(&output.stdout)
            .context("couldn't parse child stdout as json")?;

        tx.try_send(json).context("couldn't send collected value")?;
    }
}
