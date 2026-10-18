//! Framing layers of the Midea protocol, from the inside out:
//!
//! 1. **Frame** (`0xAA ...`): appliance command/response with a checksum.
//! 2. **V2 packet** (`0x5A5A ...`): 40-byte header + AES-ECB encrypted frame + MD5 sign.
//! 3. **V3 packet** (`0x8370 ...`): wraps a V2 packet with AES-CBC using a session key
//!    negotiated from the device token/key.

use std::time::{SystemTime, UNIX_EPOCH};

use rand::RngCore;

use super::crypto::{self, cbc_decrypt, cbc_encrypt, sha256};
use super::{protocol, Error, Result};

pub const DEVICE_TYPE_AC: u8 = 0xAC;
pub const FRAME_CONTROL: u8 = 0x02;
pub const FRAME_QUERY: u8 = 0x03;

const FRAME_HEADER_LEN: usize = 10;

// ---------------------------------------------------------------- frame

const CRC8_854_TABLE: [u8; 256] = [
    0x00, 0x5E, 0xBC, 0xE2, 0x61, 0x3F, 0xDD, 0x83, 0xC2, 0x9C, 0x7E, 0x20, 0xA3, 0xFD, 0x1F, 0x41,
    0x9D, 0xC3, 0x21, 0x7F, 0xFC, 0xA2, 0x40, 0x1E, 0x5F, 0x01, 0xE3, 0xBD, 0x3E, 0x60, 0x82, 0xDC,
    0x23, 0x7D, 0x9F, 0xC1, 0x42, 0x1C, 0xFE, 0xA0, 0xE1, 0xBF, 0x5D, 0x03, 0x80, 0xDE, 0x3C, 0x62,
    0xBE, 0xE0, 0x02, 0x5C, 0xDF, 0x81, 0x63, 0x3D, 0x7C, 0x22, 0xC0, 0x9E, 0x1D, 0x43, 0xA1, 0xFF,
    0x46, 0x18, 0xFA, 0xA4, 0x27, 0x79, 0x9B, 0xC5, 0x84, 0xDA, 0x38, 0x66, 0xE5, 0xBB, 0x59, 0x07,
    0xDB, 0x85, 0x67, 0x39, 0xBA, 0xE4, 0x06, 0x58, 0x19, 0x47, 0xA5, 0xFB, 0x78, 0x26, 0xC4, 0x9A,
    0x65, 0x3B, 0xD9, 0x87, 0x04, 0x5A, 0xB8, 0xE6, 0xA7, 0xF9, 0x1B, 0x45, 0xC6, 0x98, 0x7A, 0x24,
    0xF8, 0xA6, 0x44, 0x1A, 0x99, 0xC7, 0x25, 0x7B, 0x3A, 0x64, 0x86, 0xD8, 0x5B, 0x05, 0xE7, 0xB9,
    0x8C, 0xD2, 0x30, 0x6E, 0xED, 0xB3, 0x51, 0x0F, 0x4E, 0x10, 0xF2, 0xAC, 0x2F, 0x71, 0x93, 0xCD,
    0x11, 0x4F, 0xAD, 0xF3, 0x70, 0x2E, 0xCC, 0x92, 0xD3, 0x8D, 0x6F, 0x31, 0xB2, 0xEC, 0x0E, 0x50,
    0xAF, 0xF1, 0x13, 0x4D, 0xCE, 0x90, 0x72, 0x2C, 0x6D, 0x33, 0xD1, 0x8F, 0x0C, 0x52, 0xB0, 0xEE,
    0x32, 0x6C, 0x8E, 0xD0, 0x53, 0x0D, 0xEF, 0xB1, 0xF0, 0xAE, 0x4C, 0x12, 0x91, 0xCF, 0x2D, 0x73,
    0xCA, 0x94, 0x76, 0x28, 0xAB, 0xF5, 0x17, 0x49, 0x08, 0x56, 0xB4, 0xEA, 0x69, 0x37, 0xD5, 0x8B,
    0x57, 0x09, 0xEB, 0xB5, 0x36, 0x68, 0x8A, 0xD4, 0x95, 0xCB, 0x29, 0x77, 0xF4, 0xAA, 0x48, 0x16,
    0xE9, 0xB7, 0x55, 0x0B, 0x88, 0xD6, 0x34, 0x6A, 0x2B, 0x75, 0x97, 0xC9, 0x4A, 0x14, 0xF6, 0xA8,
    0x74, 0x2A, 0xC8, 0x96, 0x15, 0x4B, 0xA9, 0xF7, 0xB6, 0xE8, 0x0A, 0x54, 0xD7, 0x89, 0x6B, 0x35,
];

pub fn crc8(data: &[u8]) -> u8 {
    data.iter().fold(0u8, |crc, &b| CRC8_854_TABLE[(crc ^ b) as usize])
}

/// Two's complement of the byte sum.
pub fn checksum(data: &[u8]) -> u8 {
    data.iter().fold(0u8, |s, &b| s.wrapping_add(b)).wrapping_neg()
}

pub fn build_frame(device_type: u8, frame_type: u8, data: &[u8]) -> Vec<u8> {
    let mut frame = vec![0u8; FRAME_HEADER_LEN];
    frame[0] = 0xAA;
    frame[1] = (data.len() + FRAME_HEADER_LEN) as u8;
    frame[2] = device_type;
    frame[9] = frame_type;
    frame.extend_from_slice(data);
    frame.push(checksum(&frame[1..]));
    frame
}

pub fn validate_frame(frame: &[u8], device_type: u8) -> Result<()> {
    if frame.len() < FRAME_HEADER_LEN + 1 {
        return protocol(format!("frame too short: {}", hex::encode(frame)));
    }
    let (body, sum) = frame.split_at(frame.len() - 1);
    if checksum(&body[1..]) != sum[0] {
        return protocol(format!("frame checksum mismatch: {}", hex::encode(frame)));
    }
    if frame[2] != device_type {
        return protocol(format!("unexpected device type 0x{:X}", frame[2]));
    }
    Ok(())
}

// ---------------------------------------------------------------- V2 packet

pub fn encode_packet(device_id: u64, frame: &[u8]) -> Vec<u8> {
    let encrypted = crypto::ecb_encrypt(frame);
    let length = (40 + encrypted.len() + 16) as u16;

    let mut packet = Vec::with_capacity(length as usize);
    packet.extend_from_slice(&[0x5A, 0x5A, 0x01, 0x11]);
    packet.extend_from_slice(&length.to_le_bytes());
    packet.extend_from_slice(&[0x20, 0x00]);
    packet.extend_from_slice(&[0; 4]); // message id
    packet.extend_from_slice(&timestamp());
    packet.extend_from_slice(&device_id.to_le_bytes());
    packet.extend_from_slice(&[0; 12]);
    packet.extend_from_slice(&encrypted);
    let sign = crypto::sign(&packet);
    packet.extend_from_slice(&sign);
    packet
}

pub fn decode_packet(data: &[u8]) -> Result<Vec<u8>> {
    if data.len() < 6 || data[..2] != [0x5A, 0x5A] {
        return protocol(format!("unsupported packet: {}", hex::encode(data)));
    }
    let length = u16::from_le_bytes([data[4], data[5]]) as usize;
    if data.len() < length || length < 56 {
        return protocol("packet truncated");
    }
    let packet = &data[..length];
    let (body, rx_sign) = packet.split_at(length - 16);
    if crypto::sign(body) != rx_sign {
        return protocol("packet signature mismatch");
    }
    crypto::ecb_decrypt(&body[40..])
}

/// UTC timestamp, one byte per component, least significant first:
/// centiseconds, sec, min, hour, day, month, year % 100, year / 100.
fn timestamp() -> [u8; 8] {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    let secs = now.as_secs();
    let (y, m, d) = civil_from_days((secs / 86_400) as i64);
    let tod = secs % 86_400;
    [
        (now.subsec_millis() / 10) as u8,
        (tod % 60) as u8,
        (tod / 60 % 60) as u8,
        (tod / 3600) as u8,
        d as u8,
        m as u8,
        (y % 100) as u8,
        (y / 100) as u8,
    ]
}

/// Days since 1970-01-01 to (year, month, day). Howard Hinnant's algorithm.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    (y, m, d)
}

// ---------------------------------------------------------------- V3 packet

const V3_HANDSHAKE_REQUEST: u8 = 0x0;
const V3_HANDSHAKE_RESPONSE: u8 = 0x1;
const V3_ENCRYPTED_RESPONSE: u8 = 0x3;
const V3_ENCRYPTED_REQUEST: u8 = 0x6;
const V3_ERROR: u8 = 0xF;

pub enum V3Message {
    Handshake(Vec<u8>),
    Data(Vec<u8>),
}

/// Encoder/decoder for the V3 ("8370") transport.
#[derive(Default)]
pub struct V3Codec {
    packet_id: u16,
    local_key: Option<[u8; 32]>,
}

impl V3Codec {
    pub fn set_local_key(&mut self, key: [u8; 32]) {
        self.local_key = Some(key);
    }

    fn next_id(&mut self) -> u16 {
        let id = self.packet_id;
        self.packet_id = (self.packet_id + 1) & 0xFFF;
        id
    }

    fn header(length: usize, extra: u8) -> [u8; 6] {
        let len = (length as u16).to_be_bytes();
        [0x83, 0x70, len[0], len[1], 0x20, extra]
    }

    pub fn encode_handshake(&mut self, token: &[u8]) -> Vec<u8> {
        let mut out = Self::header(token.len(), V3_HANDSHAKE_REQUEST).to_vec();
        out.extend_from_slice(&self.next_id().to_be_bytes());
        out.extend_from_slice(token);
        out
    }

    pub fn encode_data(&mut self, data: &[u8]) -> Result<Vec<u8>> {
        let key = self.local_key.ok_or_else(|| Error::Auth("not authenticated".into()))?;
        let pad = (16 - (data.len() + 2) % 16) % 16;
        let header = Self::header(data.len() + pad + 32, (pad as u8) << 4 | V3_ENCRYPTED_REQUEST);

        let mut payload = self.next_id().to_be_bytes().to_vec();
        payload.extend_from_slice(data);
        let mut padding = vec![0u8; pad];
        rand::thread_rng().fill_bytes(&mut padding);
        payload.extend_from_slice(&padding);

        let hash = sha256(&[&header[..], &payload].concat());
        let mut out = header.to_vec();
        out.extend_from_slice(&cbc_encrypt(&key, &payload));
        out.extend_from_slice(&hash);
        Ok(out)
    }

    pub fn decode(&self, packet: &[u8]) -> Result<V3Message> {
        if packet.len() < 8 || packet[..2] != [0x83, 0x70] {
            return protocol("invalid V3 start of packet");
        }
        if packet[4] != 0x20 {
            return protocol(format!("invalid V3 magic byte 0x{:X}", packet[4]));
        }
        match packet[5] & 0xF {
            V3_HANDSHAKE_RESPONSE => Ok(V3Message::Handshake(packet[8..].to_vec())),
            V3_ENCRYPTED_RESPONSE => {
                let key = self.local_key.ok_or_else(|| Error::Auth("not authenticated".into()))?;
                if packet.len() < 6 + 32 + 16 {
                    return protocol("V3 packet too short");
                }
                let (header, rest) = packet.split_at(6);
                let (enc, rx_hash) = rest.split_at(rest.len() - 32);
                let dec = cbc_decrypt(&key, enc)?;
                if sha256(&[header, &dec].concat()) != rx_hash {
                    return protocol("V3 hash mismatch");
                }
                let pad = (header[5] >> 4) as usize;
                if dec.len() < 2 + pad {
                    return protocol("V3 payload too short");
                }
                Ok(V3Message::Data(dec[2..dec.len() - pad].to_vec()))
            }
            V3_ERROR => protocol("device returned an error packet"),
            t => protocol(format!("unexpected V3 packet type {t}")),
        }
    }
}

/// Derive the session key from the device's handshake response.
pub fn derive_local_key(key: &[u8; 32], response: &[u8]) -> Result<[u8; 32]> {
    if response.len() != 64 {
        return Err(Error::Auth("invalid handshake response length".into()));
    }
    let dec = cbc_decrypt(key, &response[..32])?;
    if sha256(&dec)[..] != response[32..] {
        return Err(Error::Auth("handshake hash mismatch (wrong token/key?)".into()));
    }
    let mut local = [0u8; 32];
    for i in 0..32 {
        local[i] = dec[i] ^ key[i];
    }
    Ok(local)
}

/// Split complete V3 packets off the front of a receive buffer.
pub fn take_v3_packets(buf: &mut Vec<u8>) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    loop {
        let Some(start) = buf.windows(2).position(|w| w == [0x83, 0x70]) else {
            buf.clear();
            break;
        };
        buf.drain(..start);
        if buf.len() < 6 {
            break;
        }
        let total = u16::from_be_bytes([buf[2], buf[3]]) as usize + 8;
        if buf.len() < total {
            break;
        }
        out.push(buf.drain(..total).collect());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // Vectors from msmart-ng's test suite.
    const V3_LOCAL_KEY: &str = "55a0a178746a424bf1fc6bb74b9fb9e4515965048d24ce8dc72aca91597d05ab";
    const STATE_FRAME: &str = "aa23ac00000000000303c00145660000003c0010045c6800000000000000000000018426";

    fn h(s: &str) -> Vec<u8> {
        hex::decode(s).unwrap()
    }

    fn local_key() -> [u8; 32] {
        h(V3_LOCAL_KEY).try_into().unwrap()
    }

    #[test]
    fn frame_checksum_validates() {
        validate_frame(&h(STATE_FRAME), DEVICE_TYPE_AC).unwrap();
        let mut bad = h(STATE_FRAME);
        bad[12] ^= 1;
        assert!(validate_frame(&bad, DEVICE_TYPE_AC).is_err());
    }

    #[test]
    fn decode_v2_packet() {
        let packet = h("5a5a01116800208000000000000000000000000060ca0000000e0000000000000000000001000000c6a90377a364cb55af337259514c6f96bf084e8c7a899b50b68920cdea36cecf11c882a88861d1f46cd87912f201218c66151f0c9fbe5941c5384e707c36ff76");
        let frame = decode_packet(&packet).unwrap();
        assert_eq!(frame, h("aa22ac00000000000303c0014566000000300010045cff2070000000000000008bed19"));
    }

    #[test]
    fn v2_packet_roundtrip() {
        let frame = h("aa21ac8d000000000003418100ff03ff000200000000000000000000000003016971");
        assert_eq!(decode_packet(&encode_packet(123456, &frame)).unwrap(), frame);
    }

    #[test]
    fn decode_v3_packet() {
        let packet = h("8370008e2063ec2b8aeb17d4e3aff77094dde7fa65cf22671adf807f490a97b927347943626e9b4f58362cf34b97a0d641f8bf0c8fcbf69ad8cca131d2d7baa70ef048c5e3f3dc78da8af4598ff47aee762a0345c18815d91b50a24dedcacde0663c4ec5e73a963dc8bbbea9a593859996eb79dcfcc6a29b96262fcaa8ea6346366efea214e4a2e48caf83489475246b6fef90192b00");
        let mut codec = V3Codec::default();
        codec.set_local_key(local_key());
        let V3Message::Data(payload) = codec.decode(&packet).unwrap() else { panic!("expected data") };
        assert_eq!(payload, h("5a5a01116800208000000000eaa908020c0817143daa0000008600000000000000000180000000003e99f93bb0cf9ffa100cb24dbae7838641d6e63ccbcd366130cd74a372932526d98479ff1725dce7df687d32e1776bf68a3fa6fd6259d7eb25f32769fcffef78"));
        assert_eq!(decode_packet(&payload).unwrap(), h(STATE_FRAME));
    }

    #[test]
    fn v3_packet_roundtrip() {
        let mut codec = V3Codec::default();
        codec.set_local_key(local_key());
        let payload = encode_packet(123456, &h(STATE_FRAME));
        let mut packet = codec.encode_data(&payload).unwrap();
        // Flip the type nibble from request (6) to response (3) so we can decode our own packet.
        packet[5] = (packet[5] & 0xF0) | V3_ENCRYPTED_RESPONSE;
        let rehash = {
            let n = packet.len();
            let key = local_key();
            let dec = cbc_decrypt(&key, &packet[6..n - 32]).unwrap();
            sha256(&[&packet[..6], &dec].concat())
        };
        let n = packet.len();
        packet[n - 32..].copy_from_slice(&rehash);
        let V3Message::Data(out) = codec.decode(&packet).unwrap() else { panic!() };
        assert_eq!(decode_packet(&out).unwrap(), h(STATE_FRAME));
    }

    #[test]
    fn splits_stream_into_packets() {
        let mut codec = V3Codec::default();
        let a = codec.encode_handshake(&[1; 64]);
        let b = codec.encode_handshake(&[2; 64]);
        let mut buf = [&[0xFF, 0x00][..], &a, &b[..10]].concat();
        let got = take_v3_packets(&mut buf);
        assert_eq!(got, vec![a]);
        buf.extend_from_slice(&b[10..]);
        assert_eq!(take_v3_packets(&mut buf), vec![b]);
        assert!(buf.is_empty());
    }

    #[test]
    fn civil_dates() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(20_740), (2026, 10, 14));
    }
}
