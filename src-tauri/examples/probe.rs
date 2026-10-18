//! Discover devices, fetch each one's token/key from the cloud and read its state (read-only).
//! Set MIDEA_EMAIL / MIDEA_PASSWORD to use your own SmartHome account.
//!     cargo run --example probe [ip]
use std::time::Duration;

use ac_remote_lib::midea::{cloud::resolve_token, discover::discover};

#[tokio::main]
async fn main() {
    env_logger::init();
    let target = std::env::args().nth(1).map(|ip| ip.parse().expect("invalid IPv4 address"));
    let account = std::env::var("MIDEA_EMAIL").ok().zip(std::env::var("MIDEA_PASSWORD").ok());
    let devs = discover(target, Duration::from_secs(4)).await.expect("discovery failed");
    for d in devs.iter().filter(|d| d.version == 3) {
        println!("== {} ({}) id {}", d.name, d.ip, d.id);
        match resolve_token(&d.ip, d.port, d.id, account.clone()).await {
            Ok((token, key, state)) => {
                println!("   token {}..  key {}..", &token[..8], &key[..8]);
                println!("   {}", serde_json::to_string(&state).unwrap());
            }
            Err(e) => println!("   failed: {e}"),
        }
    }
}
