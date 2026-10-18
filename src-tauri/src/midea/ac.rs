//! Air conditioner (device type 0xAC) commands and state.

use std::sync::atomic::{AtomicU8, Ordering};

use serde::{Deserialize, Serialize};

use super::packet::{build_frame, checksum, crc8, validate_frame, DEVICE_TYPE_AC, FRAME_CONTROL, FRAME_QUERY};
use super::{protocol, Result};

const CONTROL_SOURCE_APP: u8 = 0x02;
const RESPONSE_STATE: u8 = 0xC0;
const TEMP_INDOOR: u8 = 0x02;
const SWING_VERTICAL: u8 = 0x0C;
const SWING_HORIZONTAL: u8 = 0x03;

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
#[serde(rename_all = "camelCase")]
pub struct AcState {
    pub power: bool,
    pub mode: Mode,
    pub target: f64,
    pub fan: FanSpeed,
    /// Up/down swing.
    pub swing_v: bool,
    /// Left/right swing.
    pub swing_h: bool,
    /// Fixed louver position 1–5 (0 = not set), filled from properties.
    pub vane_v: u8,
    pub vane_h: u8,
    /// "AI ECO" (iECO).
    pub ai: bool,
    pub eco: bool,
    pub turbo: bool,
    pub indoor: Option<f64>,
    pub outdoor: Option<f64>,
    #[serde(skip)]
    extras: Extras,
}

/// A partial update coming from the UI.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcPatch {
    pub power: Option<bool>,
    pub mode: Option<Mode>,
    pub target: Option<f64>,
    pub fan: Option<FanSpeed>,
    pub swing_v: Option<bool>,
    pub swing_h: Option<bool>,
    pub vane_v: Option<u8>,
    pub vane_h: Option<u8>,
    pub ai: Option<bool>,
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
        if let Some(v) = p.swing_v {
            self.swing_v = v;
        }
        if let Some(v) = p.swing_h {
            self.swing_h = v;
        }
        if p.swing_v.is_some() || p.swing_h.is_some() {
            self.extras.swing_raw =
                if self.swing_v { SWING_VERTICAL } else { 0 } | if self.swing_h { SWING_HORIZONTAL } else { 0 };
        }
        if let Some(v) = p.vane_v {
            self.vane_v = v.min(5);
        }
        if let Some(v) = p.vane_h {
            self.vane_h = v.min(5);
        }
        if let Some(v) = p.ai {
            self.ai = v;
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
        swing_v: swing_raw & SWING_VERTICAL != 0,
        swing_h: swing_raw & SWING_HORIZONTAL != 0,
        vane_v: 0,
        vane_h: 0,
        ai: false,
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

// ---------------------------------------------------------------- capabilities & properties

const RESPONSE_CAPABILITIES: u8 = 0xB5;
const RESPONSE_PROPERTIES_ACK: u8 = 0xB0;
const RESPONSE_PROPERTIES: u8 = 0xB1;

pub const PROP_VANE_VERTICAL: u16 = 0x0009; // up/down louver angle
pub const PROP_VANE_HORIZONTAL: u16 = 0x000A; // left/right louver angle
pub const PROP_IECO: u16 = 0x00E3; // "AI ECO"

const CAP_VANE_VERTICAL: u16 = 0x0009;
const CAP_VANE_HORIZONTAL: u16 = 0x000A;
const CAP_IECO: u16 = 0x00E3;
const CAP_SWING_MODES: u16 = 0x0215;
const CAP_ECO: u16 = 0x0212;
const CAP_TURBO: u16 = 0x021A;

pub fn query_capabilities(additional: bool) -> Vec<u8> {
    if additional {
        command(FRAME_QUERY, &[0xB5, 0x01, 0x01, 0x01])
    } else {
        command(FRAME_QUERY, &[0xB5, 0x01, 0x00])
    }
}

pub fn query_properties(ids: &[u16]) -> Vec<u8> {
    let mut body = vec![0xB1, ids.len() as u8];
    for id in ids {
        body.extend_from_slice(&id.to_le_bytes());
    }
    command(FRAME_QUERY, &body)
}

pub fn set_properties(props: &[(u16, Vec<u8>)]) -> Vec<u8> {
    let mut body = vec![0xB0, props.len() as u8];
    for (id, value) in props {
        body.extend_from_slice(&id.to_le_bytes());
        body.push(value.len() as u8);
        body.extend_from_slice(value);
    }
    command(FRAME_CONTROL, &body)
}

/// What the model supports, from its 0xB5 capability report(s).
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    pub swing_vertical: bool,
    pub swing_horizontal: bool,
    pub vane_vertical: bool,
    pub vane_horizontal: bool,
    pub ieco: bool,
    pub eco: bool,
    pub turbo: bool,
    #[serde(skip)]
    pub ieco_number: u8,
    /// Raw `id → value` pairs, kept for diagnostics.
    #[serde(skip)]
    pub raw: Vec<(u16, Vec<u8>)>,
}

impl Capabilities {
    /// Fallback for devices that don't answer the capability query.
    pub fn basic() -> Self {
        Self { swing_vertical: true, eco: true, turbo: true, ieco_number: 1, ..Default::default() }
    }

    fn add(&mut self, id: u16, v: &[u8]) {
        let first = v.first().copied().unwrap_or(0);
        match id {
            CAP_SWING_MODES => {
                self.swing_horizontal = matches!(first, 1 | 3);
                self.swing_vertical = matches!(first, 0 | 1);
            }
            CAP_ECO => self.eco = matches!(first, 1 | 2),
            CAP_TURBO => self.turbo = matches!(first, 0 | 1 | 3),
            CAP_VANE_VERTICAL => self.vane_vertical = first == 1,
            CAP_VANE_HORIZONTAL => self.vane_horizontal = first == 1,
            CAP_IECO => {
                let end = v.get(1).copied().unwrap_or(0);
                self.ieco = matches!(first, 1 | 3 | 4 | 8) || matches!(end, 1 | 2 | 3 | 8);
                self.ieco_number = match end {
                    8 => 8,
                    1..=3 => 3,
                    _ => 1,
                };
            }
            _ => {}
        }
        self.raw.push((id, v.to_vec()));
    }
}

/// Parse a capability report into `caps`. Returns `Some(more)` where `more` says a second
/// ("additional") query is needed, or `None` if the frame isn't a capability report.
pub fn parse_capabilities(frame: &[u8], caps: &mut Capabilities) -> Result<Option<bool>> {
    validate_frame(frame, DEVICE_TYPE_AC)?;
    if frame.len() < 13 || frame[10] != RESPONSE_CAPABILITIES || frame[9] != FRAME_QUERY {
        return Ok(None);
    }
    let p = &frame[10..frame.len() - 2];
    let count = p[1];
    let mut rest = &p[2..];
    for _ in 0..count {
        if rest.len() < 3 {
            break;
        }
        let id = u16::from_le_bytes([rest[0], rest[1]]);
        let size = rest[2] as usize;
        let value = rest.get(3..3 + size).unwrap_or(&[]);
        if size > 0 {
            caps.add(id, value);
        }
        rest = rest.get(3 + size..).unwrap_or(&[]);
    }
    Ok(Some(rest.len() > 1 && rest[rest.len() - 2] != 0))
}

/// Louver positions 1–5 map to these raw angles; 0 means "not set".
const VANE_ANGLES: [u8; 6] = [0, 1, 25, 50, 75, 100];

fn vane_from_raw(raw: u8) -> u8 {
    if raw == 0 {
        return 0;
    }
    (1..=5).min_by_key(|&i| VANE_ANGLES[i].abs_diff(raw)).unwrap_or(0) as u8
}

impl AcPatch {
    /// Fields carried by the full set-state (0x40) command.
    pub fn touches_state(&self) -> bool {
        self.power.is_some()
            || self.mode.is_some()
            || self.target.is_some()
            || self.fan.is_some()
            || self.swing_v.is_some()
            || self.swing_h.is_some()
            || self.eco.is_some()
            || self.turbo.is_some()
    }

    /// Fields sent as properties (0xB0), restricted to what the device supports.
    pub fn properties(&self, caps: &Capabilities) -> Vec<(u16, Vec<u8>)> {
        let mut out = Vec::new();
        if let Some(v) = self.vane_v.filter(|&v| v > 0 && caps.vane_vertical) {
            out.push((PROP_VANE_VERTICAL, vec![VANE_ANGLES[v.min(5) as usize]]));
        }
        if let Some(v) = self.vane_h.filter(|&v| v > 0 && caps.vane_horizontal) {
            out.push((PROP_VANE_HORIZONTAL, vec![VANE_ANGLES[v.min(5) as usize]]));
        }
        if let Some(on) = self.ai.filter(|_| caps.ieco) {
            let mut v = vec![0, caps.ieco_number.max(1), on as u8];
            v.extend_from_slice(&[0; 10]);
            out.push((PROP_IECO, v));
        }
        out
    }
}

impl AcState {
    /// Merge values from a properties response.
    pub fn update_properties(&mut self, props: &[(u16, Vec<u8>)]) {
        for (id, v) in props {
            match (*id, v.as_slice()) {
                (PROP_VANE_VERTICAL, [raw, ..]) => self.vane_v = vane_from_raw(*raw),
                (PROP_VANE_HORIZONTAL, [raw, ..]) => self.vane_h = vane_from_raw(*raw),
                (PROP_IECO, [_, on, ..]) => self.ai = *on != 0,
                _ => {}
            }
        }
    }
}

/// Parse a properties response/ack into `id → value` pairs.
pub fn parse_properties(frame: &[u8]) -> Result<Option<Vec<(u16, Vec<u8>)>>> {
    validate_frame(frame, DEVICE_TYPE_AC)?;
    if frame.len() < 13 || !matches!(frame[10], RESPONSE_PROPERTIES | RESPONSE_PROPERTIES_ACK) {
        return Ok(None);
    }
    let p = &frame[10..frame.len() - 2];
    let count = p[1];
    let mut rest = &p[2..];
    let mut out = Vec::new();
    for _ in 0..count {
        if rest.len() < 4 {
            break;
        }
        let id = u16::from_le_bytes([rest[0], rest[1]]);
        let failed = rest[2] & 0x10 != 0;
        let size = rest[3] as usize;
        if size > 0 && !failed {
            if let Some(v) = rest.get(4..4 + size) {
                out.push((id, v.to_vec()));
            }
        } else if failed {
            log::warn!("device rejected property 0x{id:04X}");
        }
        rest = rest.get(4 + size..).unwrap_or(&[]);
    }
    Ok(Some(out))
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
        s.apply(&AcPatch { target: Some(24.5), mode: Some(Mode::Heat), fan: Some(FanSpeed::Low), swing_v: Some(true), eco: Some(true), ..Default::default() });
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

#[cfg(test)]
mod property_tests {
    use super::*;

    fn caps() -> Capabilities {
        let mut c = Capabilities::default();
        // Values reported by the user's Midea Xtreme Plus.
        c.add(CAP_SWING_MODES, &[0x01]);
        c.add(CAP_VANE_VERTICAL, &[0x01]);
        c.add(CAP_VANE_HORIZONTAL, &[0x01]);
        c.add(CAP_IECO, &[0x08, 0x08]);
        c.add(CAP_ECO, &[0x00]);
        c.add(CAP_TURBO, &[0x01]);
        c
    }

    #[test]
    fn capabilities_decode() {
        let c = caps();
        assert!(c.swing_vertical && c.swing_horizontal && c.vane_vertical && c.vane_horizontal && c.ieco);
        assert_eq!(c.ieco_number, 8);
        assert!(!c.eco && c.turbo);
    }

    #[test]
    fn property_encoding() {
        let p = AcPatch { vane_v: Some(3), ai: Some(true), ..Default::default() };
        let props = p.properties(&caps());
        assert_eq!(props[0], (PROP_VANE_VERTICAL, vec![50]));
        assert_eq!(&props[1].1[..3], &[0, 8, 1]);
        assert_eq!(props[1].1.len(), 13);
        assert!(!p.touches_state());
        let frame = set_properties(&props);
        validate_frame(&frame, DEVICE_TYPE_AC).unwrap();
        assert_eq!(&frame[10..15], &[0xB0, 2, 0x09, 0x00, 1]);
    }

    #[test]
    fn properties_roundtrip_into_state() {
        // B1 response: count 3, (id, result, size, data)...
        let body = [0xB1, 3, 0x09, 0x00, 0x00, 1, 75, 0x0A, 0x00, 0x00, 1, 0, 0xE3, 0x00, 0x00, 2, 8, 1];
        let mut payload = body.to_vec();
        payload.push(0x01); // message id
        payload.push(crc8(&payload));
        let frame = build_frame(DEVICE_TYPE_AC, FRAME_QUERY, &payload);
        let props = parse_properties(&frame).unwrap().unwrap();
        let mut s = parse_state(&hex::decode("aa23ac00000000000303c00145660000003c0010045c6b20000000000000000000020d79").unwrap()).unwrap().unwrap();
        s.update_properties(&props);
        assert_eq!((s.vane_v, s.vane_h, s.ai), (4, 0, true));
    }

    #[test]
    fn swing_axes() {
        let mut s = parse_state(&hex::decode("aa23ac00000000000303c00145660000003c0010045c6b20000000000000000000020d79").unwrap()).unwrap().unwrap();
        assert!(s.swing_v && !s.swing_h); // fixture has up/down swing on
        s.apply(&AcPatch { swing_v: Some(false), swing_h: Some(true), ..Default::default() });
        assert_eq!(set_state(&s, false)[10 + 7], 0x30 | 0x03);
        s.apply(&AcPatch { swing_v: Some(true), ..Default::default() });
        assert_eq!(set_state(&s, false)[10 + 7], 0x30 | 0x0F);
    }
}
