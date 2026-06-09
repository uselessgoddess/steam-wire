//! Steam Guard TOTP helpers.
//!
//! This crate generates the time-based one-time auth codes, mobile confirmation
//! keys and device identifiers used by the Steam mobile authenticator.
//!
//! ## Steam IDs without a hard dependency
//!
//! The upstream implementation took a `protocol::types::SteamId`. To keep this
//! crate dependency-free (and therefore usable both standalone and inside any
//! workspace), [`device_id`] and [`device_id_with_salt`] are generic over
//! `Into<u64>`: pass a plain `u64`, or any Steam ID type that converts into one
//! (for example `steamid_ng::SteamID`, which implements `Into<u64>`).
//!
//! ```
//! # use steam_totp::device_id;
//! let id = device_id(76561197960287930u64);
//! assert_eq!(id, "android:6d3f10d9-6369-a1ae-97a0-94df28b95192");
//! ```

use std::fmt;
use std::fmt::Write;
use std::time::{SystemTime, SystemTimeError, UNIX_EPOCH};

use base64::Engine;
use base64::engine::general_purpose::STANDARD as STANDARD_BASE64;
use hmac::{Hmac, KeyInit, Mac};
use sha1::{Digest, Sha1};

type HmacSha1 = Hmac<Sha1>;

const CHARS: &[char] = &[
    '2', '3', '4', '5', '6', '7', '8', '9', 'B', 'C', 'D', 'F', 'G', 'H', 'J', 'K', 'M', 'N', 'P',
    'Q', 'R', 'T', 'V', 'W', 'X', 'Y',
];

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("error decoding secret: {0}")]
    InvalidSecret(#[from] base64::DecodeError),
    #[error("secret is empty")]
    EmptySecret,
    #[error("invalid system clock difference from unix timestamp: {0:?}")]
    SystemTime(#[from] SystemTimeError),
    #[error("TODO: internal buffer error")]
    InvalidBuffer,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum Tag {
    Conf,
    Details,
    Allow,
    Cancel,
}

impl fmt::Display for Tag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Conf => write!(f, "conf"),
            Self::Details => write!(f, "details"),
            Self::Allow => write!(f, "allow"),
            Self::Cancel => write!(f, "cancel"),
        }
    }
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn decode_hex<T: AsRef<[u8]>>(secret: T) -> Option<Vec<u8>> {
    let secret = secret.as_ref();
    let len = secret.len();
    if len % 2 != 0 {
        return None;
    }
    let mut bytes = Vec::with_capacity(len / 2);
    for i in (0..len).step_by(2) {
        let hi = hex_value(secret[i])?;
        let lo = hex_value(secret[i + 1])?;
        bytes.push((hi << 4) | lo);
    }
    Some(bytes)
}

fn decode_secret<T: AsRef<[u8]>>(secret: T) -> Result<Vec<u8>, Error> {
    let decoded = if let Some(decoded) = decode_hex(secret.as_ref()) {
        decoded
    } else {
        STANDARD_BASE64.decode(secret.as_ref())?
    };
    Ok(decoded)
}

pub fn auth_code<T: AsRef<[u8]>>(
    shared_secret: T,
    time_offset: Option<i64>,
) -> Result<String, Error> {
    auth_code_for_time(shared_secret, timestampt_offset(time_offset)?)
}

pub fn confirmation_key<T: AsRef<[u8]>>(
    identity: T,
    tag: Tag,
    time_offset: Option<i64>,
) -> Result<(String, u64), Error> {
    let timestamp = timestampt_offset(time_offset)?;
    let confirmation_key = confirmation_key_for_time(identity, tag, timestamp)?;
    Ok((confirmation_key, timestamp))
}

/// Generate the Steam mobile device id for a Steam ID.
///
/// Accepts a plain `u64` or any Steam ID type that implements `Into<u64>`
/// (such as `steamid_ng::SteamID`).
pub fn device_id<S: Into<u64>>(steamid: S) -> String {
    generate_device_id(steamid.into(), None)
}

/// Like [`device_id`] but mixes an extra `salt` into the hash.
pub fn device_id_with_salt<S: Into<u64>>(steamid: S, salt: &str) -> String {
    generate_device_id(steamid.into(), Some(salt))
}

fn generate_device_id(steamid: u64, salt: Option<&str>) -> String {
    let mut hasher = Sha1::new();
    if let Some(salt) = salt {
        hasher.update(format!("{steamid}{salt}"));
    } else {
        hasher.update(steamid.to_string());
    }
    let result = hasher.finalize();
    let hash = result.iter().fold(String::new(), |mut output, b| {
        let _ = write!(output, "{b:02x}");
        output
    });
    let (p1, rest) = hash.split_at(8);
    let (p2, rest) = rest.split_at(4);
    let (p3, rest) = rest.split_at(4);
    let (p4, rest) = rest.split_at(4);
    let (p5, _) = rest.split_at(12);
    format!("android:{p1}-{p2}-{p3}-{p4}-{p5}")
}

fn auth_code_for_time<T: AsRef<[u8]>>(shared: T, timestamp: u64) -> Result<String, Error> {
    let mut full_code = {
        let bytes = (timestamp / 30).to_be_bytes();
        let hmac = hmac_of(shared, &bytes)?;
        let result = hmac.finalize().into_bytes();
        let slice_start = result[19] & 0x0F;
        let slice_end = slice_start + 4;
        let slice: &[u8] = &result[slice_start as usize..slice_end as usize];
        let full_code_slice: [u8; 4] = slice.try_into().map_err(|_| Error::InvalidBuffer)?;
        let full_code_bytes = u32::from_be_bytes(full_code_slice);
        full_code_bytes & 0x7FFFFFFF
    };
    let chars_len = CHARS.len() as u32;
    let code = (0..5)
        .map(|_| {
            let char_code = CHARS[(full_code % chars_len) as usize];
            full_code /= chars_len;
            char_code
        })
        .collect::<String>();
    Ok(code)
}

fn confirmation_key_for_time<T: AsRef<[u8]>>(
    identity_secret: T,
    tag: Tag,
    timestamp: u64,
) -> Result<String, Error> {
    let timestamp_bytes = timestamp.to_be_bytes();
    let tag_string = tag.to_string();
    let tag_bytes = tag_string.as_bytes();
    let array = [&timestamp_bytes[..], tag_bytes].concat();
    let hmac = hmac_of(identity_secret, &array)?;
    let code_bytes = hmac.finalize().into_bytes();
    Ok(STANDARD_BASE64.encode(code_bytes))
}

fn hmac_of<T: AsRef<[u8]>>(secret: T, bytes: &[u8]) -> Result<HmacSha1, Error> {
    let decoded = decode_secret(secret)?;
    let mut mac = HmacSha1::new_from_slice(&decoded[..]).map_err(|_| Error::EmptySecret)?;
    mac.update(bytes);
    Ok(mac)
}

fn timestampt_offset(time_offset: Option<i64>) -> Result<u64, Error> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_secs()
        .saturating_add_signed(-time_offset.unwrap_or(0)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_confirmation_key_for_time() {
        let identity_secret: &'static str = "000000000000000000000000000=";
        let timestamp = 1634603498;
        let hash = confirmation_key_for_time(identity_secret, Tag::Allow, timestamp).unwrap();
        assert_eq!(hash, "9/OyNC3rk7VNsMFklzayOuznImU=");
    }

    #[test]
    fn generating_a_code_works() {
        let shared_secret = "000000000000000000000000000=";
        let timestamp = 1634603498;
        let code = auth_code_for_time(shared_secret, timestamp).unwrap();
        assert_eq!(code, "2C5H2");
    }

    #[test]
    fn generating_a_code_from_hex_works() {
        let shared_secret = "D34D34D34D34D34D34D34D34D34D34D34D34D34D";
        let timestamp = 1634603498;
        let code = auth_code_for_time(shared_secret, timestamp).unwrap();
        assert_eq!(code, "2C5H2");
    }

    #[test]
    fn gets_device_id() {
        // The upstream test used `protocol::types::SteamId::new(76561197960287930)`.
        // `device_id` is now generic over `Into<u64>`, so a plain `u64` works.
        let device_id = device_id(76561197960287930u64);
        assert_eq!(device_id, "android:6d3f10d9-6369-a1ae-97a0-94df28b95192");
    }

    #[test]
    fn decode_hex_works() {
        let hex = "48656c6c6f";
        let decoded = decode_hex(hex).unwrap();
        assert_eq!(decoded, b"Hello");
    }

    #[test]
    fn hex_value_works() {
        assert_eq!(hex_value(b'0'), Some(0));
        assert_eq!(hex_value(b'9'), Some(9));
        assert_eq!(hex_value(b'a'), Some(10));
        assert_eq!(hex_value(b'f'), Some(15));
        assert_eq!(hex_value(b'A'), Some(10));
        assert_eq!(hex_value(b'F'), Some(15));
        assert_eq!(hex_value(b'G'), None);
    }
}
