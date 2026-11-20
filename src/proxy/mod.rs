//! Inbound connection preamble handling.

mod proxy_protocol;
mod server;

pub use proxy_protocol::{probe, ProbeResult, ProxyMode, ProxyProtocolError};
pub use server::serve;
