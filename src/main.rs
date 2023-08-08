use std::io::{Read, Write};
use std::process::{Command, Stdio};

const BIN: &str = "intel_gpu_top";
const BIN_ARGS: &[&str] = &["-s", "5000", "-J", "-d", "pci:card=1"];

fn main() {
    let mut process = Command::new(BIN)
        .args(BIN_ARGS)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("execute command");

    let mut buf = vec![0u8; 4096];
    let mut json = Vec::with_capacity(4096);

    loop {
        // check if child exited (non-blocking)
        if let Some(status) = process.try_wait().expect("couldn't check child status") {
            panic!("child exited prematurely with {}", status)
        }

        // get a mutable reference to stdout
        let stdout = process.stdout.as_mut().expect("child stdout is missing");

        let bytes_read = match stdout.read(&mut buf).map_err(|e| e.kind()) {
            Err(kind) => panic!("error reading stdout: {:?}", kind),
            Ok(count) => count,
        };

        // append to the buffer
        json.extend_from_slice(&buf[0..bytes_read]);

        // try to parse it as json to see if the output is complete
        match serde_json::from_slice::<serde_json::Value>(&json) {
            // not a complete json object yet
            Err(_) => continue,
            // full json object read, stop parsing
            Ok(_) => break,
        }
    }

    // json now contains the desired json object, write it out
    std::io::stdout()
        .write_all(&json)
        .expect("couldn't write to stdout");
}
