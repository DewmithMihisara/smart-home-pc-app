//! Cryptographic primitives used by the Midea LAN protocol.

use cipher::block_padding::{NoPadding, Pkcs7};
use cipher::{BlockDecryptMut, BlockEncryptMut, KeyInit, KeyIvInit};
use md5::{Digest, Md5};
use sha2::Sha256;

use super::{Error, Result};

pub const SIGN_KEY: &[u8] = b"xhdiwjnchekd4d512chdjx5d8e4c394D2D7S";

/// AES-128 key used for the inner (V2) packet payload: MD5 of the sign key.
fn enc_key() -> [u8; 16] {
    Md5::digest(SIGN_KEY).into()
}

pub fn md5(data: &[u8]) -> [u8; 16] {
    Md5::digest(data).into()
}

pub fn sha256(data: &[u8]) -> [u8; 32] {
    Sha256::digest(data).into()
}

/// MD5 signature appended to V2 packets.
pub fn sign(data: &[u8]) -> [u8; 16] {
    let mut h = Md5::new();
    h.update(data);
    h.update(SIGN_KEY);
    h.finalize().into()
}

/// AES-128-ECB with PKCS7 padding, keyed with the fixed sign key.
pub fn ecb_encrypt(data: &[u8]) -> Vec<u8> {
    ecb::Encryptor::<aes::Aes128>::new(&enc_key().into()).encrypt_padded_vec_mut::<Pkcs7>(data)
}

pub fn ecb_decrypt(data: &[u8]) -> Result<Vec<u8>> {
    ecb::Decryptor::<aes::Aes128>::new(&enc_key().into())
        .decrypt_padded_vec_mut::<Pkcs7>(data)
        .map_err(|_| Error::Protocol("failed to decrypt payload".into()))
}

/// AES-256-CBC with a zero IV and no padding (data must be 16-byte aligned).
pub fn cbc_encrypt(key: &[u8; 32], data: &[u8]) -> Vec<u8> {
    cbc::Encryptor::<aes::Aes256>::new(key.into(), &[0u8; 16].into()).encrypt_padded_vec_mut::<NoPadding>(data)
}

pub fn cbc_decrypt(key: &[u8; 32], data: &[u8]) -> Result<Vec<u8>> {
    cbc::Decryptor::<aes::Aes256>::new(key.into(), &[0u8; 16].into())
        .decrypt_padded_vec_mut::<NoPadding>(data)
        .map_err(|_| Error::Protocol("payload is not block aligned".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ecb_roundtrip() {
        let data = b"hello midea";
        let enc = ecb_encrypt(data);
        assert_eq!(enc.len(), 16);
        assert_eq!(ecb_decrypt(&enc).unwrap(), data);
    }

    #[test]
    fn cbc_roundtrip() {
        let key = [7u8; 32];
        let data = [42u8; 32];
        assert_eq!(cbc_decrypt(&key, &cbc_encrypt(&key, &data)).unwrap(), data);
    }
}
