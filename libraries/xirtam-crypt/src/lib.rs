#![doc = include_str!(concat!(env!("OUT_DIR"), "/README-rustdocified.md"))]

pub mod aead;
pub mod dh;
mod encoding;
pub mod hash;
pub mod signing;
pub use encoding::ParseKeyError;
