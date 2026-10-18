//! Local-network control of Midea air conditioners.
//!
//! A Rust port of the protocol implemented by msmart-ng
//! (<https://github.com/mill1000/midea-msmart>, MIT).

pub mod ac;
pub mod discover;
pub mod lan;
pub mod crypto;
pub mod packet;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("protocol error: {0}")]
    Protocol(String),
    #[error("authentication failed: {0}")]
    Auth(String),
    #[error("no response from the air conditioner")]
    Timeout,
    #[error("network error: {0}")]
    Io(#[from] std::io::Error),
    #[error("cloud error: {0}")]
    Cloud(String),
}

impl serde::Serialize for Error {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;

pub(crate) fn protocol<T>(msg: impl Into<String>) -> Result<T> {
    Err(Error::Protocol(msg.into()))
}
