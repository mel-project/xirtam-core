use std::fmt;

use base64::{Engine as _, engine::general_purpose::STANDARD};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParseKeyError {
    InvalidBase64,
    InvalidLength,
    InvalidPublicKey,
}

pub fn encode_32_base64(bytes: [u8; 32]) -> String {
    STANDARD.encode(bytes)
}

pub fn decode_32_base64(input: &str) -> Result<[u8; 32], ParseKeyError> {
    let bytes = STANDARD
        .decode(input.as_bytes())
        .map_err(|_| ParseKeyError::InvalidBase64)?;
    if bytes.len() != 32 {
        return Err(ParseKeyError::InvalidLength);
    }
    let mut buf = [0u8; 32];
    buf.copy_from_slice(&bytes);
    Ok(buf)
}

impl fmt::Display for ParseKeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseKeyError::InvalidBase64 => write!(f, "invalid base64"),
            ParseKeyError::InvalidLength => write!(f, "invalid key length"),
            ParseKeyError::InvalidPublicKey => write!(f, "invalid public key"),
        }
    }
}

impl std::error::Error for ParseKeyError {}
