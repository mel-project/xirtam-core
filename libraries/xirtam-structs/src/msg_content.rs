use bytes::Bytes;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

use crate::{handle::Handle, timestamp::NanoTimestamp};

/// A decoded message payload.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MessageContent {
    pub recipient: Handle,
    pub sent_at: NanoTimestamp,
    pub mime: SmolStr,
    pub body: Bytes,
}
