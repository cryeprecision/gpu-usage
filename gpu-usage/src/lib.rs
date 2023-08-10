use anyhow::{Context, Result};
use serde_json::Value;
use std::process::Stdio;
use tokio::io::AsyncReadExt;

const BIN: &str = "intel_gpu_top";
const BUFFER_LEN: usize = 4096;

pub async fn gpu_usage(delay_ms: u64, device: &str) -> Result<Value> {
    let delay = delay_ms.to_string();
    let args: [&str; 5] = ["-s", &delay, "-J", "-d", device];

    let mut child = tokio::process::Command::new(BIN)
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .context("couldn't spawn child process")?;

    let mut read_buf = vec![0u8; BUFFER_LEN];
    let mut json_buf = Vec::<u8>::with_capacity(BUFFER_LEN);

    let mut stdout = child.stdout.take().context("child stdout pipe missing")?;

    loop {
        if let Some(status) = child.try_wait().context("couldn't check child status")? {
            panic!("child exited prematurely with {}", status);
        }

        let bytes_read = match stdout.read(&mut read_buf).await {
            Err(err) => panic!("error reading child stdout: {:?}", err),
            Ok(count) => count,
        };

        // append to the buffer
        json_buf.extend_from_slice(&read_buf[0..bytes_read]);

        // try to parse it as json to see if the output is complete
        match serde_json::from_slice::<serde_json::Value>(&json_buf) {
            // not a complete json object yet
            Err(_) => continue,
            // full json object read, stop parsing
            Ok(val) => break Ok(val),
        }
    }
}

// fn main() {
//     let mut process = Command::new(BIN)
//         .args(BIN_ARGS)
//         .stdout(Stdio::piped())
//         .stderr(Stdio::null())
//         .spawn()
//         .expect("execute command");

//     let mut buf = vec![0u8; 4096];
//     let mut json = Vec::with_capacity(4096);

//     loop {
//         // check if child exited (non-blocking)
//         if let Some(status) = process.try_wait().expect("couldn't check child status") {
//             panic!("child exited prematurely with {}", status)
//         }

//         // get a mutable reference to stdout
//         let stdout = process.stdout.as_mut().expect("child stdout is missing");

//         let bytes_read = match stdout.read(&mut buf).map_err(|e| e.kind()) {
//             Err(kind) => panic!("error reading stdout: {:?}", kind),
//             Ok(count) => count,
//         };

//         // append to the buffer
//         json.extend_from_slice(&buf[0..bytes_read]);

//         // try to parse it as json to see if the output is complete
//         match serde_json::from_slice::<serde_json::Value>(&json) {
//             // not a complete json object yet
//             Err(_) => continue,
//             // full json object read, stop parsing
//             Ok(_) => break,
//         }
//     }

//     // json now contains the desired json object, write it out
//     std::io::stdout()
//         .write_all(&json)
//         .expect("couldn't write to stdout");
// }
