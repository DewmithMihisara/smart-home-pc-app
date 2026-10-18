//! Scan the LAN for Midea devices:  cargo run --example discover [ip]
use std::time::Duration;

use ac_remote_lib::midea::discover::discover;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let target = std::env::args().nth(1).map(|ip| ip.parse().expect("invalid IPv4 address"));
    println!("Scanning for 5 seconds...");
    match discover(target, Duration::from_secs(5)).await {
        Ok(devs) if devs.is_empty() => println!("No devices found."),
        Ok(devs) => {
            for d in devs {
                println!("{} | id {} | v{} | type 0x{:X} | {} | sn {}", d.ip, d.id, d.version, d.device_type, d.name, d.sn);
            }
        }
        Err(e) => eprintln!("Discovery failed: {e}"),
    }
}
