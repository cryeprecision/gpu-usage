use gpu_usage;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let usage = gpu_usage::gpu_usage(5000, "pci:card=1").await.unwrap();
    println!("{}", serde_json::to_string(&usage).unwrap());
}
