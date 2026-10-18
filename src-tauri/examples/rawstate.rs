//! Read-only: dump the raw 0xC0 state frame of each saved device and how we decode it.
//!     cargo run --example rawstate
use ac_remote_lib::midea::{ac, lan::{DeviceConfig, LanClient}};

#[tokio::main]
async fn main() {
    let path = std::path::Path::new(&std::env::var("APPDATA").unwrap_or_default()).join("com.acremote.app/config.json");
    let cfg: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).expect("no saved config")).unwrap();
    for d in cfg["devices"].as_array().unwrap() {
        println!("== {} ({})", d["name"], d["ip"]);
        let token = hex::decode(d["token"].as_str().unwrap()).unwrap();
        let key: [u8; 32] = hex::decode(d["key"].as_str().unwrap()).unwrap().try_into().unwrap();
        let mut c = LanClient::new(DeviceConfig { ip: d["ip"].as_str().unwrap().into(), port: d["port"].as_u64().unwrap() as u16, id: d["id"].as_str().unwrap().parse().unwrap(), auth: Some((token, key)) });
        match c.raw(&ac::query_state()).await {
            Ok(frames) => for f in frames {
                println!("   frame {}", hex::encode(&f));
                if f.len() > 26 {
                    let p = &f[10..];
                    println!("   p[2]=0x{:02X} (nibble {} half {}) p[13]=0x{:02X} (alt {})", p[2], p[2] & 0xF, p[2] & 0x10 != 0, p[13], p[13] & 0x1F);
                }
                println!("   decoded {:?}", ac::parse_state(&f).ok().flatten().map(|s| (s.power, s.mode, s.target)));
            },
            Err(e) => println!("   failed: {e}"),
        }
    }
}
