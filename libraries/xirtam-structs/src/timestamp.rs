use serde::{Deserialize, Serialize};

/// A seconds-granularity Unix timestamp, represented as an integer.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[serde(transparent)]
pub struct Timestamp(pub u64);
