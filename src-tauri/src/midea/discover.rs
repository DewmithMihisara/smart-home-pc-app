//! UDP broadcast discovery of Midea devices on the LAN.

use std::collections::HashSet;
use std::net::{Ipv4Addr, SocketAddr};
use std::time::Duration;

use serde::Serialize;
use tokio::net::UdpSocket;
use tokio::time::{timeout_at, Instant};

use super::crypto::{self, sha256};
use super::Result;

const DISCOVERY_MSG: [u8; 72] = [
    0x5a, 0x5a, 0x01, 0x11, 0x48, 0x00, 0x92, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x7f, 0x75, 0xbd, 0x6b, 0x3e, 0x4f, 0x8b, 0x76, //
    0x2e, 0x84, 0x9c, 0x6e, 0x57, 0x8d, 0x65, 0x90, 0x03, 0x6e, 0x9d, 0x43, 0x42, 0xa5, 0x0f, 0x1f, //
    0x56, 0x9e, 0xb8, 0xec, 0x91, 0x8e, 0x92, 0xe5,
];
const DISCOVERY_PORTS: [u16; 2] = [6445, 20086];

#[derive(Clone, Debug, Serialize)]
pub struct DeviceInfo {
    pub ip: String,
    pub port: u16,
    /// Serialized as a string: JS numbers can't safely hold every 64-bit id.
    #[serde(serialize_with = "as_string")]
    pub id: u64,
    pub name: String,
    pub sn: String,
    pub version: u8,
    pub device_type: u8,
}

fn as_string<S: serde::Serializer>(v: &u64, s: S) -> std::result::Result<S::Ok, S::Error> {
    s.serialize_str(&v.to_string())
}

/// Broadcast (or unicast to `target`) discovery packets and collect replies for `wait`.
pub async fn discover(target: Option<Ipv4Addr>, wait: Duration) -> Result<Vec<DeviceInfo>> {
    let sock = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).await?;
    let dest = target.unwrap_or(Ipv4Addr::BROADCAST);
    if target.is_none() {
        sock.set_broadcast(true)?;
    }
    for port in DISCOVERY_PORTS {
        for _ in 0..3 {
            sock.send_to(&DISCOVERY_MSG, (dest, port)).await?;
        }
    }

    let deadline = Instant::now() + wait;
    let mut seen = HashSet::new();
    let mut found = Vec::new();
    let mut buf = [0u8; 2048];
    while let Ok(res) = timeout_at(deadline, sock.recv_from(&mut buf)).await {
        let Ok((n, SocketAddr::V4(addr))) = res else { continue };
        if !seen.insert(*addr.ip()) {
            continue;
        }
        match parse_reply(&buf[..n], addr.ip().to_string()) {
            Some(dev) => found.push(dev),
            None => log::debug!("ignoring discovery reply from {addr}"),
        }
        if target.is_some() {
            break;
        }
    }
    Ok(found)
}

fn parse_reply(data: &[u8], ip: String) -> Option<DeviceInfo> {
    let (version, body) = match data.get(..2)? {
        [0x5a, 0x5a] => (2, data),
        [0x83, 0x70] if data.len() > 24 => (3, &data[8..data.len() - 16]),
        _ => return None, // V1 (XML) devices are not supported
    };
    if body.len() < 40 + 16 + 16 {
        return None;
    }
    let id = u64::from_le_bytes([body[20], body[21], body[22], body[23], body[24], body[25], 0, 0]);
    let dec = crypto::ecb_decrypt(&body[40..body.len() - 16]).ok()?;
    if dec.len() < 41 {
        return None;
    }
    let port = u16::from_le_bytes([dec[4], dec[5]]);
    let sn = String::from_utf8_lossy(&dec[8..40]).trim_end_matches('\0').to_string();
    let name_len = dec[40] as usize;
    let name = String::from_utf8_lossy(dec.get(41..41 + name_len)?).to_string();
    // SSID looks like "net_ac_XXXX": the second component is the device type in hex.
    let device_type = name.split('_').nth(1).and_then(|t| u8::from_str_radix(t, 16).ok()).unwrap_or(0);
    Some(DeviceInfo { ip, port, id, name, sn, version, device_type })
}

/// The "udpid" the cloud uses to look up a device's token/key.
pub fn udpid(id: u64, little_endian: bool) -> String {
    let le = id.to_le_bytes();
    let bytes: Vec<u8> = if little_endian { le[..6].to_vec() } else { le[..6].iter().rev().copied().collect() };
    let h = sha256(&bytes);
    let x: Vec<u8> = (0..16).map(|i| h[i] ^ h[i + 16]).collect();
    hex::encode(x)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn udpid_endianness() {
        // Matches msmart-ng's Security.udpid(id.to_bytes(6, endian)).
        let id = 0x0000_1234_5678_9ABC_u64;
        let le = udpid(id, true);
        let be = udpid(id, false);
        assert_eq!(le.len(), 32);
        assert_ne!(le, be);
        let manual = sha256(&[0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC]);
        let expect: Vec<u8> = (0..16).map(|i| manual[i] ^ manual[i + 16]).collect();
        assert_eq!(be, hex::encode(expect));
    }
}
