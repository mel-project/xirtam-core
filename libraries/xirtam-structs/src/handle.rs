use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize};
use smol_str::SmolStr;

/// A validated user handle matching ^@[A-Za-z0-9_]{5,15}$.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct Handle(SmolStr);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HandleError;

impl Handle {
    pub fn parse(handle: impl AsRef<str>) -> Result<Self, HandleError> {
        let handle = handle.as_ref();
        if !is_valid_handle(handle) {
            return Err(HandleError);
        }
        Ok(Self(SmolStr::new(handle)))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for Handle {
    type Err = HandleError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl TryFrom<SmolStr> for Handle {
    type Error = HandleError;

    fn try_from(value: SmolStr) -> Result<Self, Self::Error> {
        if !is_valid_handle(value.as_str()) {
            return Err(HandleError);
        }
        Ok(Self(value))
    }
}

impl<'de> Deserialize<'de> for Handle {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = SmolStr::deserialize(deserializer)?;
        Handle::try_from(value).map_err(serde::de::Error::custom)
    }
}

impl fmt::Display for HandleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid handle")
    }
}

impl std::error::Error for HandleError {}

fn is_valid_handle(handle: &str) -> bool {
    let bytes = handle.as_bytes();
    if bytes.len() < 6 || bytes.len() > 16 || bytes[0] != b'@' {
        return false;
    }
    bytes[1..]
        .iter()
        .all(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
}
