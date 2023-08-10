use anyhow::{bail, Context, Result};
use serde_json::Value;
use std::process::Stdio;
use tokio::io::AsyncReadExt;

const BIN: &str = "intel_gpu_top";
const BUFFER_LEN: usize = 4096;
const DELAY_MS: &str = "5000";

pub async fn gpu_usage(device: &str) -> Result<Value> {
    let args: [&str; 5] = ["-s", DELAY_MS, "-J", "-d", device];

    let start = std::time::Instant::now();

    let mut child = tokio::process::Command::new(BIN)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .context("couldn't spawn child process")?;

    // take stdout here so we don't have to do it in the loop
    let mut stdout = child.stdout.take().context("child stdout pipe missing")?;
    // take stderr here so we don't have to do it in the loop
    let mut stderr = child.stderr.take().context("child stderr pipe missing")?;

    // buffer to read child stdout into
    let mut read_buf = vec![0u8; BUFFER_LEN];
    // buffer to assemble child stdout until it's a complete json object
    let mut json_buf = Vec::<u8>::with_capacity(BUFFER_LEN);

    let json_val = loop {
        // check if the child exited prematurely and if it did, print its stderr output
        if let Some(status) = child.try_wait().context("couldn't check child status")? {
            let mut stderr_str = String::with_capacity(BUFFER_LEN);
            stderr
                .read_to_string(&mut stderr_str)
                .await
                .context("couldn't read stderr of exited child")?;

            log::error!("child-stderr: {}", stderr_str);
            bail!("child exited prematurely with {}", status)
        }

        let stdout_bytes_read = stdout
            .read(&mut read_buf)
            .await
            .context("couldn't read child stdout")?;
        json_buf.extend_from_slice(&read_buf[0..stdout_bytes_read]);

        // try to parse it as json to see if the output is complete
        match serde_json::from_slice::<serde_json::Value>(&json_buf) {
            // not a complete json object yet
            Err(_) => continue,
            // full json object read, stop parsing
            Ok(val) => break val,
        }
    };

    let elapsed_ms = start.elapsed().as_secs_f64() * 1e3;
    log::info!("elapsed: {:.1}ms", elapsed_ms);

    Ok(json_val)
}
