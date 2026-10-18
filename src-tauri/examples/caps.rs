//! Read-only: print a saved device's capabilities, state and louver/iECO properties.
//!     cargo run --example caps [device-name-or-ip]
use ac_remote_lib::midea::{ac, lan::{DeviceConfig, LanClient}};

#[tokio::main]
async fn main() {
    let path = std::path::Path::new(&std::env::var("APPDATA").unwrap_or_default()).join("com.acremote.app/config.json");
    let cfg: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).expect("no saved config")).unwrap();
    let filter = std::env::args().nth(1);
    for d in cfg["devices"].as_array().unwrap() {
        let (name, ip) = (d["name"].as_str().unwrap(), d["ip"].as_str().unwrap());
        if filter.as_deref().is_some_and(|f| f != name && f != ip) {
            continue;
        }
        println!("== {name} ({ip})");
        let token = hex::decode(d["token"].as_str().unwrap()).unwrap();
        let key: [u8; 32] = hex::decode(d["key"].as_str().unwrap()).unwrap().try_into().unwrap();
        let mut c = LanClient::new(DeviceConfig { ip: ip.into(), port: d["port"].as_u64().unwrap() as u16, id: d["id"].as_str().unwrap().parse().unwrap(), auth: Some((token, key)) });
        match c.query_capabilities().await {
            Ok(caps) => {
                println!("   caps: {}", serde_json::to_string(&caps).unwrap());
                for (id, v) in &caps.raw { println!("     0x{id:04X} = {}", hex::encode(v)); }
            }
            Err(e) => println!("   caps failed: {e}"),
        }
        match c.query_state().await { Ok(s) => println!("   state: {}", serde_json::to_string(&s).unwrap()), Err(e) => println!("   state failed: {e}") }
        match c.query_properties(&[ac::PROP_VANE_VERTICAL, ac::PROP_VANE_HORIZONTAL, ac::PROP_IECO]).await {
            Ok(p) => for (id, v) in p { println!("   prop 0x{id:04X} = {}", hex::encode(v)); },
            Err(e) => println!("   props failed: {e}"),
        }
    }
}
