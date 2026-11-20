//! Bounded, synchronous PROXY protocol v1/v2 parsing.

use std::fmt;
use std::io::{self, Read};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpStream};

pub const PROXY_V2_SIGNATURE: &[u8; 12] = b"\r\n\r\n\0\r\nQUIT\n";
const PROXY_V1_PREFIX: &[u8] = b"PROXY ";
const MAX_V1_HEADER: usize = 108;
const MAX_V2_PAYLOAD: usize = 4096;

/// Configured PROXY protocol acceptance mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProxyMode {
    Auto,
    V1,
    V2,
    Off,
}

impl ProxyMode {
    #[must_use]
    pub fn from_config(value: &str) -> Self {
        match value {
            "v1" => Self::V1,
            "v2" => Self::V2,
            "off" => Self::Off,
            _ => Self::Auto,
        }
    }
}

/// Parsed connection identity and bytes already read from the HTTP request.
#[derive(Debug, PartialEq, Eq)]
pub struct ProbeResult {
    pub client: SocketAddr,
    pub prefix: Vec<u8>,
    pub proxied: bool,
}

#[derive(Debug)]
pub enum ProxyProtocolError {
    Io(io::Error),
    Malformed(&'static str),
    VersionNotAllowed,
    ProxyRequired,
}

impl fmt::Display for ProxyProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "failed reading connection preamble: {error}"),
            Self::Malformed(reason) => write!(f, "malformed PROXY header: {reason}"),
            Self::VersionNotAllowed => write!(f, "PROXY protocol version is not allowed"),
            Self::ProxyRequired => write!(f, "a PROXY protocol header is required"),
        }
    }
}

impl std::error::Error for ProxyProtocolError {}

impl From<io::Error> for ProxyProtocolError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

enum Preamble {
    Plain,
    V1,
    V2,
}

/// Probe and consume an optional PROXY header, preserving any HTTP bytes read.
pub fn probe(
    stream: &mut TcpStream,
    peer: SocketAddr,
    mode: ProxyMode,
    require_proxy: bool,
) -> Result<ProbeResult, ProxyProtocolError> {
    let mut buffer = Vec::with_capacity(128);
    let preamble = classify(stream, &mut buffer)?;

    match preamble {
        Preamble::Plain => {
            if require_proxy {
                return Err(ProxyProtocolError::ProxyRequired);
            }
            Ok(ProbeResult {
                client: peer,
                prefix: buffer,
                proxied: false,
            })
        }
        Preamble::V1 => {
            if !matches!(mode, ProxyMode::Auto | ProxyMode::V1) {
                return Err(ProxyProtocolError::VersionNotAllowed);
            }
            read_until_crlf(stream, &mut buffer)?;
            let (client, consumed) = parse_v1(&buffer, peer)?;
            Ok(ProbeResult {
                client,
                prefix: buffer[consumed..].to_vec(),
                proxied: true,
            })
        }
        Preamble::V2 => {
            if !matches!(mode, ProxyMode::Auto | ProxyMode::V2) {
                return Err(ProxyProtocolError::VersionNotAllowed);
            }
            read_to(stream, &mut buffer, 16)?;
            let length = u16::from_be_bytes([buffer[14], buffer[15]]) as usize;
            if length > MAX_V2_PAYLOAD {
                return Err(ProxyProtocolError::Malformed("v2 payload is too large"));
            }
            read_to(stream, &mut buffer, 16 + length)?;
            let (client, consumed) = parse_v2(&buffer, peer)?;
            Ok(ProbeResult {
                client,
                prefix: buffer[consumed..].to_vec(),
                proxied: true,
            })
        }
    }
}

fn classify(stream: &mut TcpStream, buffer: &mut Vec<u8>) -> Result<Preamble, ProxyProtocolError> {
    loop {
        if buffer.starts_with(PROXY_V1_PREFIX) {
            return Ok(Preamble::V1);
        }
        if buffer.len() >= PROXY_V2_SIGNATURE.len()
            && &buffer[..PROXY_V2_SIGNATURE.len()] == PROXY_V2_SIGNATURE
        {
            return Ok(Preamble::V2);
        }

        let could_be_v1 = PROXY_V1_PREFIX.starts_with(buffer);
        let could_be_v2 = PROXY_V2_SIGNATURE.starts_with(buffer);
        if !could_be_v1 && !could_be_v2 {
            return Ok(Preamble::Plain);
        }

        let mut byte = [0u8; 1];
        if stream.read(&mut byte)? == 0 {
            return Ok(Preamble::Plain);
        }
        buffer.push(byte[0]);
    }
}

fn read_until_crlf(stream: &mut TcpStream, buffer: &mut Vec<u8>) -> Result<(), ProxyProtocolError> {
    while !buffer.windows(2).any(|window| window == b"\r\n") {
        if buffer.len() >= MAX_V1_HEADER {
            return Err(ProxyProtocolError::Malformed("v1 header is too long"));
        }
        let mut byte = [0u8; 1];
        if stream.read(&mut byte)? == 0 {
            return Err(ProxyProtocolError::Malformed("truncated v1 header"));
        }
        buffer.push(byte[0]);
    }
    Ok(())
}

fn read_to(
    stream: &mut TcpStream,
    buffer: &mut Vec<u8>,
    length: usize,
) -> Result<(), ProxyProtocolError> {
    while buffer.len() < length {
        let mut chunk = [0u8; 512];
        let wanted = (length - buffer.len()).min(chunk.len());
        let read = stream.read(&mut chunk[..wanted])?;
        if read == 0 {
            return Err(ProxyProtocolError::Malformed("truncated v2 header"));
        }
        buffer.extend_from_slice(&chunk[..read]);
    }
    Ok(())
}

fn parse_v1(buffer: &[u8], peer: SocketAddr) -> Result<(SocketAddr, usize), ProxyProtocolError> {
    let end = buffer
        .windows(2)
        .position(|window| window == b"\r\n")
        .ok_or(ProxyProtocolError::Malformed("missing v1 terminator"))?;
    let line = std::str::from_utf8(&buffer[..end])
        .map_err(|_| ProxyProtocolError::Malformed("v1 header is not ASCII"))?;
    let fields: Vec<_> = line.split_ascii_whitespace().collect();

    if fields == ["PROXY", "UNKNOWN"] {
        return Ok((peer, end + 2));
    }
    if fields.len() != 6 || fields[0] != "PROXY" {
        return Err(ProxyProtocolError::Malformed("invalid v1 field count"));
    }

    let client_ip: IpAddr = fields[2]
        .parse()
        .map_err(|_| ProxyProtocolError::Malformed("invalid v1 client address"))?;
    let destination_ip: IpAddr = fields[3]
        .parse()
        .map_err(|_| ProxyProtocolError::Malformed("invalid v1 destination address"))?;
    let client_port = fields[4]
        .parse()
        .map_err(|_| ProxyProtocolError::Malformed("invalid v1 client port"))?;
    let _: u16 = fields[5]
        .parse()
        .map_err(|_| ProxyProtocolError::Malformed("invalid v1 destination port"))?;
    let family_matches = matches!(
        (fields[1], client_ip, destination_ip),
        ("TCP4", IpAddr::V4(_), IpAddr::V4(_)) | ("TCP6", IpAddr::V6(_), IpAddr::V6(_))
    );
    if !family_matches {
        return Err(ProxyProtocolError::Malformed("v1 address family mismatch"));
    }

    Ok((SocketAddr::new(client_ip, client_port), end + 2))
}

fn parse_v2(buffer: &[u8], peer: SocketAddr) -> Result<(SocketAddr, usize), ProxyProtocolError> {
    if buffer.len() < 16 || &buffer[..12] != PROXY_V2_SIGNATURE {
        return Err(ProxyProtocolError::Malformed("invalid v2 signature"));
    }
    let version_command = buffer[12];
    if version_command >> 4 != 2 {
        return Err(ProxyProtocolError::Malformed("invalid v2 version"));
    }
    let command = version_command & 0x0f;
    let length = u16::from_be_bytes([buffer[14], buffer[15]]) as usize;
    let consumed = 16 + length;
    if buffer.len() < consumed {
        return Err(ProxyProtocolError::Malformed("truncated v2 payload"));
    }
    if command == 0 {
        return Ok((peer, consumed));
    }
    if command != 1 {
        return Err(ProxyProtocolError::Malformed("invalid v2 command"));
    }

    let address = &buffer[16..consumed];
    let client = match buffer[13] {
        0x00 => peer,
        0x11 if address.len() >= 12 => SocketAddr::new(
            IpAddr::V4(Ipv4Addr::new(
                address[0], address[1], address[2], address[3],
            )),
            u16::from_be_bytes([address[8], address[9]]),
        ),
        0x21 if address.len() >= 36 => {
            let mut octets = [0u8; 16];
            octets.copy_from_slice(&address[..16]);
            SocketAddr::new(
                IpAddr::V6(Ipv6Addr::from(octets)),
                u16::from_be_bytes([address[32], address[33]]),
            )
        }
        0x11 | 0x21 => {
            return Err(ProxyProtocolError::Malformed("short v2 address block"));
        }
        _ => {
            return Err(ProxyProtocolError::Malformed(
                "unsupported v2 family or transport",
            ));
        }
    };
    Ok((client, consumed))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_v1_and_preserves_http() {
        let raw = b"PROXY TCP4 203.0.113.7 10.0.0.2 4567 8080\r\nGET / HTTP/1.1\r\n";
        let peer = "127.0.0.1:9000".parse().unwrap();
        let (client, consumed) = parse_v1(raw, peer).unwrap();
        assert_eq!(client, "203.0.113.7:4567".parse().unwrap());
        assert!(raw[consumed..].starts_with(b"GET /"));
    }

    #[test]
    fn parses_v2_ipv4() {
        let mut raw = PROXY_V2_SIGNATURE.to_vec();
        raw.extend_from_slice(&[0x21, 0x11, 0, 12]);
        raw.extend_from_slice(&[203, 0, 113, 8, 10, 0, 0, 2]);
        raw.extend_from_slice(&4567u16.to_be_bytes());
        raw.extend_from_slice(&8080u16.to_be_bytes());
        let peer = "127.0.0.1:9000".parse().unwrap();
        let (client, consumed) = parse_v2(&raw, peer).unwrap();
        assert_eq!(client, "203.0.113.8:4567".parse().unwrap());
        assert_eq!(consumed, raw.len());
    }
}
