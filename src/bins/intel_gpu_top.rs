use anyhow::{anyhow, Context, Result};
use async_channel::Sender;
use serde_json::Value;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::AsyncReadExt;

const BIN: &str = "intel_gpu_top";
const BUFFER_LEN: usize = 4096;

/// Spawn the `intel_gpu_top` command and collect the JSON output
pub async fn intel_gpu_top(tx: Sender<Value>, interval_ms: u64, device: String) -> Result<()> {
    let interval_str = interval_ms.to_string();
    let args: [&str; 5] = ["-s", &interval_str, "-J", "-d", &device];

    let mut child = tokio::process::Command::new(BIN)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .context("couldn't spawn child process")?;

    // Take stdout and stderr here so we don't have to do it in the loop
    let mut stdout = child.stdout.take().context("child stdout pipe missing")?;
    let mut stderr = child.stderr.take().context("child stderr pipe missing")?;

    // Buffer to read child stdout into
    let mut read_buf = vec![0u8; BUFFER_LEN];
    // Buffer to assemble child stdout until it's a complete json object
    let mut json_buf = Vec::<u8>::with_capacity(BUFFER_LEN);

    // Check for new output with double the frequency to not miss any
    let timeout = Duration::from_millis(interval_ms / 2);

    loop {
        // check if the child exited prematurely
        if let Some(status) = child.try_wait().context("couldn't check child status")? {
            // read out the stderr buffer
            let mut stderr_str = String::with_capacity(BUFFER_LEN);
            stderr
                .read_to_string(&mut stderr_str)
                .await
                .context("couldn't read stderr of exited child")?;

            // print it as error log and return
            log::error!("child stderr: {}", stderr_str);
            return Err(anyhow!("child exited prematurely with {}", status));
        }

        // Keep checking the child stdout until it writes something
        loop {
            match tokio::time::timeout(timeout, stdout.read(&mut read_buf)).await {
                // Timed out, child is finished writing to stdout for now
                Err(_) => break,
                // Reading from the child stdout failed, return the error
                Ok(Err(err)) => return Err(err).context("couldn't read from child stdout"),
                // Child process wrote something, append it to the buffer
                Ok(Ok(val)) => json_buf.extend_from_slice(&read_buf[0..val]),
            }
        }

        // buffer is empty, keep checking for more output
        if json_buf.is_empty() {
            continue;
        }

        // parse the collected output as json
        let json = serde_json::from_slice::<Value>(&json_buf)
            .context("couldn't parse child stdout as json")?;

        // clear the buffer for the next object
        json_buf.clear();

        // this is not allowed to block since we are in an async function
        tx.try_send(json).context("couldn't send collected value")?;
    }
}
