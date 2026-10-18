//! Air conditioner (device type 0xAC) commands and state.

use std::sync::atomic::{AtomicU8, Ordering};

use serde::{Deserialize, Serialize};

use super::packet::{build_frame, checksum, crc8, validate_frame, DEVICE_TYPE_AC, FRAME_CONTROL, FRAME_QUERY};
use super::{protocol, Result};

const CONTROL_SOURCE_APP: u8 = 0x02;
const RESPONSE_STATE: u8 = 0xC0;
const TEMP_INDOOR: u8 = 0x02;
const SWING_VERTICAL: u8 = 0x0C;

pub const MIN_TEMP: f64 = 16.0;
pub const MAX_TEMP: f64 = 30.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Auto,
    Cool,
    Dry,
    Heat,
    Fan,
}

impl Mode {
    fn to_raw(self) -> u8 {
        match self {
            Mode::Auto => 1,
            Mode::Cool => 2,
            Mode::Dry => 3,
            Mode::Heat => 4,
            Mode::Fan => 5,
        }
    }

    fn from_raw(v: u8) -> Mode {
        match v {
            1 => Mode::Auto,
            2 => Mode::Cool,
            3 | 6 => Mode::Dry, // 6 = "smart dry"
            4 => Mode::Heat,
            _ => Mode::Fan,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FanSpeed {
    Auto,
    Low,
    Medium,
    High,
}

impl FanSpeed {
    fn to_raw(self) -> u8 {
        match self {
            FanSpeed::Auto => 102,
            FanSpeed::Low => 40,
            FanSpeed::Medium => 60,
            FanSpeed::High => 80,
        }
    }

    /// The device reports 0–100 (or 101/102 for auto); bucket it.
    fn from_raw(v: u8) -> FanSpeed {
        match v {
            101.. => FanSpeed::Auto,
            0..=50 => FanSpeed::Low,
            51..=70 => FanSpeed::Medium,
            _ => FanSpeed::High,
        }
    }
}

/// Raw values we don't expose in the UI but must echo back on every set,
/// because a set-state command always carries the full state.
#[derive(Clone, Debug, Default, PartialEq)]
struct Extras {
    fan_raw: u8,
    swing_raw: u8,
    sleep: bool,
    fahrenheit: bool,
    follow_me: bool,
    purifier: bool,
    freeze_protection: bool,
    aux_heat: bool,
    independent_aux_heat: bool,
    humidity: u8,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AcState {
    pub power: bool,
    pub mode: Mode,
    pub target: f64,
    pub fan: FanSpeed,
    pub swing: bool,
    pub eco: bool,
    pub turbo: bool,
    pub indoor: Option<f64>,
    pub outdoor: Option<f64>,
    #[serde(skip)]
    extras: Extras,
}

/// A partial update coming from the UI.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct AcPatch {
    pub power: Option<bool>,
    pub mode: Option<Mode>,
    pub target: Option<f64>,
    pub fan: Option<FanSpeed>,
    pub swing: Option<bool>,
    pub eco: Option<bool>,
    pub turbo: Option<bool>,
}

impl AcState {
    pub fn apply(&mut self, p: &AcPatch) {
        if let Some(v) = p.power {
            self.power = v;
        }
        if let Some(v) = p.mode {
            self.mode = v;
        }
        if let Some(v) = p.target {
            self.target = ((v * 2.0).round() / 2.0).clamp(MIN_TEMP, MAX_TEMP);
        }
        if let Some(v) = p.fan {
            self.fan = v;
            self.extras.fan_raw = v.to_raw();
        }
        if let Some(v) = p.swing {
            self.swing = v;
            self.extras.swing_raw = if v { SWING_VERTICAL } else { 0 };
        }
        if let Some(v) = p.eco {
            self.eco = v;
        }
        if let Some(v) = p.turbo {
            self.turbo = v;
        }
    }
}

static MESSAGE_ID: AtomicU8 = AtomicU8::new(0);

/// Wrap a command body in a frame: body + message id + CRC8.
fn command(frame_type: u8, body: &[u8]) -> Vec<u8> {
    let mut payload = body.to_vec();
    payload.push(MESSAGE_ID.fetch_add(1, Ordering::Relaxed).wrapping_add(1));
    payload.push(crc8(&payload));
    build_frame(DEVICE_TYPE_AC, frame_type, &payload)
}

pub fn query_state() -> Vec<u8> {
    let mut body = vec![0x41, 0x81, 0x00, 0xFF, 0x03, 0xFF, 0x00, TEMP_INDOOR];
    body.extend_from_slice(&[0; 12]);
    body.push(0x03);
    command(FRAME_QUERY, &body)
}

pub fn set_state(s: &AcState, beep: bool) -> Vec<u8> {
    let x = &s.extras;
    let whole = s.target.trunc() as u8;
    let half = s.target.fract() > 0.0;

    let (mut temp, temp_alt) = if (17..=30).contains(&whole) { ((whole - 16) & 0xF, 0) } else { (0, whole.wrapping_sub(12) & 0x1F) };
    if half {
        temp |= 0x10;
    }

    let flag = |b: bool, bit: u8| if b { bit } else { 0 };
    let body = [
        0x40,
        CONTROL_SOURCE_APP | flag(beep, 0x40) | flag(s.power, 0x01),
        temp | (s.mode.to_raw() & 0x7) << 5,
        x.fan_raw,
        0x7F,
        0x7F,
        0x00, // timers
        0x30 | (x.swing_raw & 0x3F),
        flag(x.follow_me, 0x80) | flag(s.turbo, 0x20),
        flag(s.eco, 0x80) | flag(x.purifier, 0x20) | flag(x.aux_heat, 0x08),
        flag(x.sleep, 0x01) | flag(s.turbo, 0x02) | flag(x.fahrenheit, 0x04),
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        temp_alt,
        x.humidity & 0x7F,
        0x00,
        flag(x.freeze_protection, 0x80),
        flag(x.independent_aux_heat, 0x08),
        0x00,
    ];
    command(FRAME_CONTROL, &body)
}

/// Parse a frame received from the device. Returns `None` for frames that aren't state responses.
pub fn parse_state(frame: &[u8]) -> Result<Option<AcState>> {
    validate_frame(frame, DEVICE_TYPE_AC)?;
    if frame.len() < 13 || frame[10] != RESPONSE_STATE {
        return Ok(None);
    }

    // Payload (from the response id up to, excluding, the frame checksum) ends with a CRC8,
    // although some devices use a checksum instead.
    let with_crc = &frame[10..frame.len() - 1];
    let (body, crc) = with_crc.split_at(with_crc.len() - 1);
    if crc8(body) != crc[0] && checksum(body) != crc[0] {
        return protocol("state response failed CRC");
    }

    let p = body;
    if p.len() < 16 {
        return protocol("state response too short");
    }

    let fahrenheit = p[10] & 0x04 != 0;
    let mut target = (p[2] & 0xF) as f64 + 16.0 + if p[2] & 0x10 != 0 { 0.5 } else { 0.0 };
    let alt = p[13] & 0x1F;
    if alt != 0 {
        target = alt as f64 + 12.0 + if p[2] & 0x10 != 0 { 0.5 } else { 0.0 };
    }

    let fan_raw = p[3] & 0x7F;
    let swing_raw = p[7] & 0xF;

    Ok(Some(AcState {
        power: p[1] & 0x01 != 0,
        mode: Mode::from_raw((p[2] >> 5) & 0x7),
        target,
        fan: FanSpeed::from_raw(fan_raw),
        swing: swing_raw != 0,
        eco: p[9] & 0x10 != 0,
        turbo: p[8] & 0x20 != 0 || p[10] & 0x02 != 0,
        indoor: parse_temperature(p[11], (p[15] & 0xF) as f64 / 10.0, fahrenheit),
        outdoor: parse_temperature(p[12], (p[15] >> 4) as f64 / 10.0, fahrenheit),
        extras: Extras {
            fan_raw,
            swing_raw,
            sleep: p[10] & 0x01 != 0,
            fahrenheit,
            follow_me: p[8] & 0x80 != 0,
            purifier: p[9] & 0x20 != 0,
            aux_heat: p[9] & 0x08 != 0,
            independent_aux_heat: p[8] & 0x40 != 0,
            humidity: p.get(19).map_or(40, |h| h & 0x7F),
            freeze_protection: p.get(21).is_some_and(|f| f & 0x80 != 0),
        },
    }))
}

fn parse_temperature(data: u8, decimals: f64, fahrenheit: bool) -> Option<f64> {
    if data == 0xFF {
        return None;
    }
    let t = (data as f64 - 50.0) / 2.0;
    let sign = if t >= 0.0 { 1.0 } else { -1.0 };
    if !fahrenheit && decimals != 0.0 {
        return Some(t.trunc() + sign * decimals);
    }
    if decimals >= 0.5 {
        return Some(t.trunc() + sign * 0.5);
    }
    Some(t)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h(s: &str) -> Vec<u8> {
        hex::decode(s).unwrap()
    }

    #[test]
    fn query_state_frame() {
        let f = query_state();
        // Same as msmart-ng's GetStateCommand apart from the message id + CRC + checksum.
        assert_eq!(&f[..31], &h("aa21ac00000000000003418100ff03ff000200000000000000000000000003")[..]);
        validate_frame(&f, DEVICE_TYPE_AC).unwrap();
        assert_eq!(f.len(), 34);
        assert_eq!(f[32], crc8(&f[10..32]));
    }

    #[test]
    fn parse_v3_state() {
        let s = parse_state(&h("aa23ac00000000000303c00145660000003c0010045c6b20000000000000000000020d79")).unwrap().unwrap();
        assert!(s.power);
        assert_eq!(s.mode, Mode::Cool);
        assert_eq!(s.target, 21.0);
        assert_eq!(s.indoor, Some(21.0));
        assert_eq!(s.outdoor, Some(28.5));
        assert_eq!(s.fan, FanSpeed::Auto);
    }

    #[test]
    fn parse_state_checksum_as_crc() {
        let s = parse_state(&h("aa1eac00000000000003c0004b1e7f7f000000000069630000000000000d33")).unwrap().unwrap();
        assert_eq!(s.target, 27.0);
        assert_eq!(s.indoor, Some(27.5));
        assert_eq!(s.outdoor, Some(24.5));
    }

    #[test]
    fn set_state_roundtrips_through_parser_fields() {
        let mut s = parse_state(&h("aa23ac00000000000303c00145660000003c0010045c6b20000000000000000000020d79")).unwrap().unwrap();
        s.apply(&AcPatch { target: Some(24.5), mode: Some(Mode::Heat), fan: Some(FanSpeed::Low), swing: Some(true), eco: Some(true), ..Default::default() });
        let f = set_state(&s, true);
        validate_frame(&f, DEVICE_TYPE_AC).unwrap();
        let b = &f[10..];
        assert_eq!(b[0], 0x40);
        assert_eq!(b[1], 0x43); // app source | beep | power
        assert_eq!(b[2], 0x10 | 8 | (4 << 5)); // 24.5 °C, heat
        assert_eq!(b[3], 40);
        assert_eq!(b[7], 0x30 | 0x0C);
        assert_eq!(b[9] & 0x80, 0x80);
    }

    #[test]
    fn non_state_frames_are_ignored() {
        let f = command(FRAME_QUERY, &[0xB5, 0x01, 0x00]);
        assert!(parse_state(&f).unwrap().is_none());
    }
}
