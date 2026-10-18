//! TCP session with a device: connect, V3 handshake, request/response.

use std::collections::VecDeque;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::{sleep, timeout, Instant};

use super::ac::{self, AcState, Capabilities};
use super::packet::{self, derive_local_key, take_v3_packets, V3Codec, V3Message};
use super::{protocol, Error, Result};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const READ_TIMEOUT: Duration = Duration::from_secs(2);
const AUTH_LIFETIME: Duration = Duration::from_secs(12 * 3600);
const RETRIES: usize = 3;

#[derive(Clone, Debug)]
pub struct DeviceConfig {
    pub ip: String,
    pub port: u16,
    pub id: u64,
    /// 64-byte token and 32-byte key for V3 devices; `None` for V2.
    pub auth: Option<(Vec<u8>, [u8; 32])>,
}

struct Conn {
    stream: TcpStream,
    codec: V3Codec,
    buf: Vec<u8>,
    pending: VecDeque<Vec<u8>>,
    authed_at: Option<Instant>,
}

pub struct LanClient {
    cfg: DeviceConfig,
    conn: Option<Conn>,
}

impl LanClient {
    pub fn new(cfg: DeviceConfig) -> Self {
        Self { cfg, conn: None }
    }

    pub async fn query_state(&mut self) -> Result<AcState> {
        let frames = self.request(&ac::query_state()).await?;
        first_state(&frames)?.ok_or_else(|| Error::Protocol("device sent no state".into()))
    }

    /// Send a full state; returns the device's reported state if it sent one back.
    pub async fn set_state(&mut self, state: &AcState) -> Result<Option<AcState>> {
        let frames = self.request(&ac::set_state(state, true)).await?;
        first_state(&frames)
    }

    /// Ask the device what it supports (one or two 0xB5 round trips).
    pub async fn query_capabilities(&mut self) -> Result<Capabilities> {
        let mut caps = Capabilities { ieco_number: 1, ..Default::default() };
        let mut got_any = false;
        for additional in [false, true] {
            let frames = self.request(&ac::query_capabilities(additional)).await?;
            let mut more = false;
            for f in &frames {
                if let Ok(Some(m)) = ac::parse_capabilities(f, &mut caps) {
                    got_any = true;
                    more = m;
                }
            }
            if !more {
                break;
            }
        }
        Ok(if got_any { caps } else { Capabilities::basic() })
    }

    pub async fn query_properties(&mut self, ids: &[u16]) -> Result<Vec<(u16, Vec<u8>)>> {
        let frames = self.request(&ac::query_properties(ids)).await?;
        Ok(frames.iter().filter_map(|f| ac::parse_properties(f).ok().flatten()).flatten().collect())
    }

    pub async fn set_properties(&mut self, props: &[(u16, Vec<u8>)]) -> Result<Vec<(u16, Vec<u8>)>> {
        let frames = self.request(&ac::set_properties(props)).await?;
        Ok(frames.iter().filter_map(|f| ac::parse_properties(f).ok().flatten()).flatten().collect())
    }

    /// Send a frame and collect the response frames. Reconnects once on a stale connection.
    async fn request(&mut self, frame: &[u8]) -> Result<Vec<Vec<u8>>> {
        match self.try_request(frame).await {
            Err(Error::Auth(e)) => Err(Error::Auth(e)),
            Err(e) => {
                log::info!("request failed ({e}); reconnecting");
                self.conn = None;
                self.try_request(frame).await.inspect_err(|_| self.conn = None)
            }
            ok => ok,
        }
    }

    async fn try_request(&mut self, frame: &[u8]) -> Result<Vec<Vec<u8>>> {
        self.ensure_connected().await?;
        let id = self.cfg.id;
        let v3 = self.cfg.auth.is_some();
        let conn = self.conn.as_mut().expect("connected");

        let payload = packet::encode_packet(id, frame);
        let wire = if v3 { conn.codec.encode_data(&payload)? } else { payload };

        // Discard anything unsolicited that arrived since the last request.
        conn.pending.clear();
        for attempt in 1..=RETRIES {
            conn.stream.write_all(&wire).await?;
            match conn.read_payload(v3, READ_TIMEOUT).await {
                Ok(first) => {
                    let mut frames = vec![packet::decode_packet(&first)?];
                    // Pick up any additional responses that are already queued.
                    while let Ok(p) = conn.read_payload(v3, Duration::from_millis(150)).await {
                        if let Ok(f) = packet::decode_packet(&p) {
                            frames.push(f);
                        }
                    }
                    return Ok(frames);
                }
                Err(Error::Timeout) if attempt < RETRIES => continue,
                Err(e) => return Err(e),
            }
        }
        Err(Error::Timeout)
    }

    async fn ensure_connected(&mut self) -> Result<()> {
        let expired = self
            .conn
            .as_ref()
            .and_then(|c| c.authed_at)
            .is_some_and(|t| t.elapsed() > AUTH_LIFETIME);
        if self.conn.is_some() && !expired {
            return Ok(());
        }

        let addr = format!("{}:{}", self.cfg.ip, self.cfg.port);
        let stream = timeout(CONNECT_TIMEOUT, TcpStream::connect(&addr))
            .await
            .map_err(|_| Error::Timeout)??;
        stream.set_nodelay(true)?;
        let mut conn = Conn { stream, codec: V3Codec::default(), buf: Vec::new(), pending: VecDeque::new(), authed_at: None };

        if let Some((token, key)) = &self.cfg.auth {
            conn.handshake(token, key).await?;
            // The device needs a moment after authenticating before it accepts commands.
            sleep(Duration::from_millis(1000)).await;
        }
        log::info!("connected to {addr}");
        self.conn = Some(conn);
        Ok(())
    }
}

impl Conn {
    async fn handshake(&mut self, token: &[u8], key: &[u8; 32]) -> Result<()> {
        for attempt in 1..=RETRIES {
            let req = self.codec.encode_handshake(token);
            self.stream.write_all(&req).await?;
            match self.read_v3(READ_TIMEOUT).await {
                Ok(V3Message::Handshake(resp)) => {
                    let local = derive_local_key(key, &resp)?;
                    self.codec.set_local_key(local);
                    self.authed_at = Some(Instant::now());
                    return Ok(());
                }
                Ok(V3Message::Data(_)) => return Err(Error::Auth("unexpected data during handshake".into())),
                Err(Error::Timeout) if attempt < RETRIES => continue,
                Err(Error::Protocol(e)) => return Err(Error::Auth(e)),
                Err(e) => return Err(e),
            }
        }
        Err(Error::Timeout)
    }

    /// Read one inner (5A5A) payload.
    async fn read_payload(&mut self, v3: bool, wait: Duration) -> Result<Vec<u8>> {
        if v3 {
            match self.read_v3(wait).await? {
                V3Message::Data(d) => Ok(d),
                V3Message::Handshake(_) => protocol("unexpected handshake response"),
            }
        } else {
            self.read_v2(wait).await
        }
    }

    async fn read_v3(&mut self, wait: Duration) -> Result<V3Message> {
        let deadline = Instant::now() + wait;
        loop {
            if let Some(p) = self.pending.pop_front() {
                return self.codec.decode(&p);
            }
            self.fill(deadline).await?;
            self.pending.extend(take_v3_packets(&mut self.buf));
        }
    }

    async fn read_v2(&mut self, wait: Duration) -> Result<Vec<u8>> {
        let deadline = Instant::now() + wait;
        loop {
            if let Some(start) = self.buf.windows(2).position(|w| w == [0x5A, 0x5A]) {
                self.buf.drain(..start);
                if self.buf.len() >= 6 {
                    let len = u16::from_le_bytes([self.buf[4], self.buf[5]]) as usize;
                    if len >= 6 && self.buf.len() >= len {
                        return Ok(self.buf.drain(..len).collect());
                    }
                }
            }
            self.fill(deadline).await?;
        }
    }

    async fn fill(&mut self, deadline: Instant) -> Result<()> {
        let mut chunk = [0u8; 1024];
        let n = tokio::time::timeout_at(deadline, self.stream.read(&mut chunk))
            .await
            .map_err(|_| Error::Timeout)??;
        if n == 0 {
            return Err(Error::Io(std::io::ErrorKind::ConnectionReset.into()));
        }
        self.buf.extend_from_slice(&chunk[..n]);
        Ok(())
    }
}

fn first_state(frames: &[Vec<u8>]) -> Result<Option<AcState>> {
    for f in frames {
        match ac::parse_state(f) {
            Ok(Some(s)) => return Ok(Some(s)),
            Ok(None) => {}
            Err(e) => log::warn!("bad frame from device: {e}"),
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::midea::crypto::{cbc_decrypt, cbc_encrypt, sha256};
    use tokio::net::TcpListener;

    const KEY: [u8; 32] = [0x11; 32];
    const STATE_FRAME: &str = "aa23ac00000000000303c00145660000003c0010045c6b20000000000000000000020d79";

    /// Encode a V3 encrypted *response* (type 3) the way a device would.
    fn device_packet(local: &[u8; 32], data: &[u8]) -> Vec<u8> {
        let mut codec = V3Codec::default();
        codec.set_local_key(*local);
        let mut p = codec.encode_data(data).unwrap();
        p[5] = (p[5] & 0xF0) | 0x3;
        let n = p.len();
        let dec = cbc_decrypt(local, &p[6..n - 32]).unwrap();
        let hash = sha256(&[&p[..6], &dec].concat());
        p[n - 32..].copy_from_slice(&hash);
        p
    }

    /// A fake V3 air conditioner that answers one handshake and one request.
    async fn fake_device(listener: TcpListener) {
        let (mut s, _) = listener.accept().await.unwrap();
        let mut buf = vec![0u8; 4096];

        // Handshake: 6 byte header + 2 byte id + 64 byte token.
        let n = s.read(&mut buf).await.unwrap();
        assert_eq!(n, 72);
        let session = [0x22u8; 32];
        let mut resp = vec![0x83, 0x70, 0x00, 0x40, 0x20, 0x01, 0x00, 0x00];
        resp.extend_from_slice(&cbc_encrypt(&KEY, &session));
        resp.extend_from_slice(&sha256(&session));
        s.write_all(&resp).await.unwrap();
        let local: [u8; 32] = std::array::from_fn(|i| session[i] ^ KEY[i]);

        // Request: decrypt and check it is a state query for our device.
        let n = s.read(&mut buf).await.unwrap();
        let req = &buf[..n];
        assert_eq!(req[5] & 0xF, 0x6);
        let dec = cbc_decrypt(&local, &req[6..n - 32]).unwrap();
        let frame = packet::decode_packet(&dec[2..]).unwrap();
        assert_eq!(frame[10], 0x41);

        let reply = packet::encode_packet(42, &hex::decode(STATE_FRAME).unwrap());
        s.write_all(&device_packet(&local, &reply)).await.unwrap();
    }

    #[tokio::test]
    async fn v3_session_end_to_end() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = tokio::spawn(fake_device(listener));

        let mut client = LanClient::new(DeviceConfig { ip: "127.0.0.1".into(), port, id: 42, auth: Some((vec![0xAB; 64], KEY)) });
        let state = client.query_state().await.unwrap();
        assert!(state.power);
        assert_eq!(state.target, 21.0);
        assert_eq!(state.outdoor, Some(28.5));
        server.await.unwrap();
    }

    #[tokio::test]
    async fn wrong_key_is_an_auth_error() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(fake_device(listener));

        let mut client = LanClient::new(DeviceConfig { ip: "127.0.0.1".into(), port, id: 42, auth: Some((vec![0xAB; 64], [0x99; 32])) });
        assert!(matches!(client.query_state().await, Err(Error::Auth(_))));
    }
}
