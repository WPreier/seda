use std::fmt;
use std::io;
use std::net::{SocketAddr, TcpListener};

#[derive(Debug)]
pub struct BindError {
    address: SocketAddr,
    source: io::Error,
}

impl fmt::Display for BindError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.source.kind() == io::ErrorKind::AddrInUse {
            write!(
                f,
                "cannot listen on {}: address/port is already in use",
                self.address
            )
        } else {
            write!(f, "cannot listen on {}: {}", self.address, self.source)
        }
    }
}

impl std::error::Error for BindError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

/// Bind the listener before any irreversible startup hardening.
pub fn bind_listener(address: SocketAddr) -> Result<TcpListener, BindError> {
    TcpListener::bind(address).map_err(|source| BindError { address, source })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_address_already_in_use() {
        let first = bind_listener("127.0.0.1:0".parse().unwrap()).unwrap();
        let address = first.local_addr().unwrap();
        let error = bind_listener(address).unwrap_err();
        assert!(error.to_string().contains("already in use"));
    }
}
