# gpu-usage

The [`intel_gpu_top`](https://manpages.debian.org/bookworm/intel-gpu-tools/intel_gpu_top.1.en.html) command from the [intel-gpu-tools](https://manpages.debian.org/bookworm/intel-gpu-tools/index.html) package doesn't terminate.
To fix that, this program spawns the `intel_gpu_top` command as a child, parses its output until a complete JSON object is found, prints it to stdout, and terminates.

Also uses the [`sensors`](https://packages.debian.org/de/sid/lm-sensors) package.

## Binaries

- [`intel-gpu-tools`](https://packages.debian.org/source/stable/intel-gpu-tools)
- [`sensors`](https://packages.debian.org/source/stable/lm-sensors)
- [`inxi`](https://packages.debian.org/source/stable/inxi)

## Proxmox

If you want to display the GPU usage in Proxmox, do the following

- Build the binary (you may need to [install rust](https://rustup.rs/))
  - `cargo build --release`
- Copy the binary to an accessible path
  - `sudo cp ./target/release/gpu-usage /usr/local/bin/`
- Make sure non-root users do not have permission to write to the file
  - `sudo chmod 755 /usr/local/bin/gpu-usage`
- Allow non-root users to execute it with root permissions
  - `sudo chmod u+s /usr/local/bin/gpu-usage`
- Use this as a guide to change the Proxmox node summary
  - [https://www.reddit.com/r/homelab/comments/rhq56e/](https://www.reddit.com/r/homelab/comments/rhq56e/)
  - [https://www.reddit.com/r/homelab/comments/rhq56e/comment/ja1tr7a/](https://www.reddit.com/r/homelab/comments/rhq56e/comment/ja1tr7a/)
