pub mod auth;
pub mod config;
pub mod pairing;
pub mod power;
pub mod protocol;
pub mod server;
#[cfg(windows)]
pub mod service;
pub mod storage;

pub const DEFAULT_PORT: u16 = 47_991;
pub const PROTOCOL_VERSION: u32 = 1;
pub const HOST_VERSION: &str = env!("CARGO_PKG_VERSION");
