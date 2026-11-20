//! Bounded HTTP/1.1 static content server.

use std::io::{self, Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use tracing::{debug, info, warn};

use crate::admin::RuntimeState;
use crate::config::{ip_in_allowlist, SedaConfig};
use crate::content::{Content, ContentStore};

use super::{probe, ProxyMode};

/// Serve static content until the listener fails.
pub fn serve(listener: TcpListener, runtime: RuntimeState) -> io::Result<()> {
    info!(listen = %listener.local_addr()?, "Seda listening");

    for accepted in listener.incoming() {
        match accepted {
            Ok(mut stream) => {
                let config = runtime.load_config();
                let Some(permit) = ConnectionPermit::try_acquire(
                    &runtime.active_connections,
                    config.limits.max_connections,
                ) else {
                    warn!("rejected connection because max_connections was reached");
                    let _ = send_response(
                        &mut stream,
                        503,
                        "Service Unavailable",
                        "text/plain; charset=utf-8",
                        b"service unavailable",
                        false,
                        &[],
                    );
                    continue;
                };
                let content = Arc::clone(&runtime.content);
                thread::spawn(move || {
                    let _permit = permit;
                    if let Err(error) = handle(stream, &config, &content) {
                        debug!(%error, "connection closed with an error");
                    }
                });
            }
            Err(error) if is_transient_accept_error(&error) => {
                warn!(%error, "transient accept error");
            }
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

struct ConnectionPermit {
    active: Arc<AtomicUsize>,
}

impl ConnectionPermit {
    fn try_acquire(active: &Arc<AtomicUsize>, maximum: usize) -> Option<Self> {
        let mut current = active.load(Ordering::Acquire);
        loop {
            if current >= maximum {
                return None;
            }
            match active.compare_exchange_weak(
                current,
                current + 1,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => break,
                Err(observed) => current = observed,
            }
        }
        Some(Self {
            active: Arc::clone(active),
        })
    }
}

impl Drop for ConnectionPermit {
    fn drop(&mut self) {
        self.active.fetch_sub(1, Ordering::AcqRel);
    }
}

fn handle(
    mut stream: TcpStream,
    config: &SedaConfig,
    content_store: &ContentStore,
) -> io::Result<()> {
    let peer = stream.peer_addr()?;
    // Connections are deliberately closed after one request, so the smaller
    // request/idle bound governs the entire socket lifetime.
    let timeout = Duration::from_secs(
        config
            .limits
            .request_timeout_secs
            .min(config.limits.idle_timeout_secs),
    );
    stream.set_read_timeout(Some(timeout))?;
    stream.set_write_timeout(Some(timeout))?;

    if !ip_in_allowlist(peer.ip(), &config.trust.allowed_peers) {
        warn!(%peer, "rejected peer outside trust allowlist");
        send_plain_response(&mut stream, 403, "Forbidden", b"forbidden")?;
        return Ok(());
    }

    let mode = ProxyMode::from_config(&config.proxy.protocol);
    let parsed = match probe(&mut stream, peer, mode, config.proxy.require_proxy) {
        Ok(parsed) => parsed,
        Err(error) => {
            warn!(%peer, %error, "rejected connection preamble");
            send_plain_response(&mut stream, 403, "Forbidden", b"forbidden")?;
            return Ok(());
        }
    };

    let request = match read_http_head(&mut stream, parsed.prefix, config.limits.max_header_bytes) {
        Ok(request) => request,
        Err(HeaderReadError::TooLarge) => {
            send_plain_response(
                &mut stream,
                431,
                "Request Header Fields Too Large",
                b"request headers too large",
            )?;
            return Ok(());
        }
        Err(HeaderReadError::Incomplete) => {
            send_plain_response(&mut stream, 400, "Bad Request", b"bad request")?;
            return Ok(());
        }
        Err(HeaderReadError::Io(error))
            if matches!(
                error.kind(),
                io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
            ) =>
        {
            send_plain_response(&mut stream, 408, "Request Timeout", b"request timeout")?;
            return Ok(());
        }
        Err(HeaderReadError::Io(error)) => return Err(error),
    };
    let request = match parse_request(&request, config.limits.max_headers) {
        Ok(request) => request,
        Err(ParseRequestError::TooManyHeaders) => {
            send_plain_response(
                &mut stream,
                431,
                "Request Header Fields Too Large",
                b"too many headers",
            )?;
            return Ok(());
        }
        Err(ParseRequestError::UnsupportedVersion) => {
            send_plain_response(
                &mut stream,
                505,
                "HTTP Version Not Supported",
                b"HTTP/1.1 required",
            )?;
            return Ok(());
        }
        Err(ParseRequestError::Malformed) => {
            send_plain_response(&mut stream, 400, "Bad Request", b"bad request")?;
            return Ok(());
        }
    };

    info!(
        peer = %peer,
        client = %parsed.client,
        proxied = parsed.proxied,
        method = request.method,
        path = request.path,
        forwarded_headers = ?request.forwarded_headers,
        "accepted request"
    );
    if request.method != "GET" && request.method != "HEAD" {
        return send_response(
            &mut stream,
            405,
            "Method Not Allowed",
            "text/plain; charset=utf-8",
            b"method not allowed",
            request.method == "HEAD",
            &[("Allow", "GET, HEAD")],
        );
    }

    match content_store.lookup(request.path) {
        Some(content) => send_content(&mut stream, 200, "OK", &content, request.method == "HEAD"),
        None => {
            let not_found = content_store.not_found();
            send_content(
                &mut stream,
                404,
                "Not Found",
                &not_found,
                request.method == "HEAD",
            )
        }
    }
}

#[derive(Debug)]
enum HeaderReadError {
    TooLarge,
    Incomplete,
    Io(io::Error),
}

fn read_http_head(
    stream: &mut TcpStream,
    mut buffer: Vec<u8>,
    maximum: usize,
) -> Result<Vec<u8>, HeaderReadError> {
    while !buffer.windows(4).any(|window| window == b"\r\n\r\n") {
        if buffer.len() >= maximum {
            return Err(HeaderReadError::TooLarge);
        }
        let mut chunk = [0u8; 1024];
        let wanted = (maximum - buffer.len()).min(chunk.len());
        let read = stream
            .read(&mut chunk[..wanted])
            .map_err(HeaderReadError::Io)?;
        if read == 0 {
            return Err(HeaderReadError::Incomplete);
        }
        buffer.extend_from_slice(&chunk[..read]);
    }
    Ok(buffer)
}

#[derive(Debug, PartialEq, Eq)]
enum ParseRequestError {
    Malformed,
    TooManyHeaders,
    UnsupportedVersion,
}

#[derive(Debug)]
struct Request<'a> {
    method: &'a str,
    path: &'a str,
    forwarded_headers: Vec<(&'a str, &'a str)>,
}

fn parse_request(bytes: &[u8], maximum_headers: usize) -> Result<Request<'_>, ParseRequestError> {
    let end = bytes
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or(ParseRequestError::Malformed)?;
    let head = std::str::from_utf8(&bytes[..end]).map_err(|_| ParseRequestError::Malformed)?;
    let mut lines = head.split("\r\n");
    let mut request_parts = lines.next().ok_or(ParseRequestError::Malformed)?.split(' ');
    let method = request_parts.next().ok_or(ParseRequestError::Malformed)?;
    let path = request_parts.next().ok_or(ParseRequestError::Malformed)?;
    let version = request_parts.next().ok_or(ParseRequestError::Malformed)?;
    if request_parts.next().is_some()
        || method.is_empty()
        || !path.starts_with('/')
        || path.contains(['\r', '\n'])
    {
        return Err(ParseRequestError::Malformed);
    }
    if version != "HTTP/1.1" {
        return Err(ParseRequestError::UnsupportedVersion);
    }

    let mut header_count = 0;
    let mut forwarded_headers = Vec::new();
    for line in lines {
        header_count += 1;
        if header_count > maximum_headers {
            return Err(ParseRequestError::TooManyHeaders);
        }
        let Some((name, value)) = line.split_once(':') else {
            return Err(ParseRequestError::Malformed);
        };
        if name.is_empty()
            || name.starts_with([' ', '\t'])
            || !name.bytes().all(is_header_name_byte)
        {
            return Err(ParseRequestError::Malformed);
        }
        if is_forwarded_header(name) {
            forwarded_headers.push((name, value.trim()));
        }
    }
    Ok(Request {
        method,
        path,
        forwarded_headers,
    })
}

fn is_header_name_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte)
}

fn is_forwarded_header(name: &str) -> bool {
    name.eq_ignore_ascii_case("forwarded")
        || name.eq_ignore_ascii_case("x-real-ip")
        || name
            .get(.."x-forwarded-".len())
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("x-forwarded-"))
}

fn send_content(
    stream: &mut TcpStream,
    status: u16,
    reason: &str,
    content: &Content,
    head_only: bool,
) -> io::Result<()> {
    send_response(
        stream,
        status,
        reason,
        content.mime(),
        content.bytes(),
        head_only,
        &[],
    )
}

fn send_plain_response(
    stream: &mut TcpStream,
    status: u16,
    reason: &str,
    body: &[u8],
) -> io::Result<()> {
    send_response(
        stream,
        status,
        reason,
        "text/plain; charset=utf-8",
        body,
        false,
        &[],
    )
}

fn send_response(
    stream: &mut TcpStream,
    status: u16,
    reason: &str,
    mime: &str,
    body: &[u8],
    head_only: bool,
    extra_headers: &[(&str, &str)],
) -> io::Result<()> {
    write!(
        stream,
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nConnection: close\r\n",
        body.len()
    )?;
    for (name, value) in extra_headers {
        write!(stream, "{name}: {value}\r\n")?;
    }
    stream.write_all(b"\r\n")?;
    if !head_only {
        stream.write_all(body)?;
    }
    stream.flush()?;
    let _ = stream.shutdown(Shutdown::Both);
    Ok(())
}

fn is_transient_accept_error(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::Interrupted
            | io::ErrorKind::ConnectionAborted
            | io::ErrorKind::ConnectionReset
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::EmbeddedDocsConfig;

    #[test]
    fn parses_origin_form_http_11_request() {
        let request = parse_request(
            b"GET /seda?source=test HTTP/1.1\r\nHost: localhost\r\nX-Forwarded-For: ignored\r\n\r\n",
            2,
        )
        .unwrap();
        assert_eq!(request.method, "GET");
        assert_eq!(request.path, "/seda?source=test");
        assert_eq!(
            request.forwarded_headers,
            vec![("X-Forwarded-For", "ignored")]
        );
    }

    #[test]
    fn rejects_excess_headers_and_old_http() {
        assert_eq!(
            parse_request(b"GET / HTTP/1.1\r\nA: 1\r\nB: 2\r\n\r\n", 1).unwrap_err(),
            ParseRequestError::TooManyHeaders
        );
        assert_eq!(
            parse_request(b"GET / HTTP/1.0\r\n\r\n", 1).unwrap_err(),
            ParseRequestError::UnsupportedVersion
        );
    }

    #[test]
    fn serves_embedded_content_and_embedded_not_found_page() {
        let store = ContentStore::new(&EmbeddedDocsConfig::default(), &[], 1024).unwrap();
        let config: SedaConfig = toml::from_str(crate::DEFAULT_CONFIG_TOML).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();

        thread::scope(|scope| {
            scope.spawn(|| {
                for _ in 0..3 {
                    let (stream, _) = listener.accept().unwrap();
                    handle(stream, &config, &store).unwrap();
                }
            });

            let found = request(address, b"GET /seda HTTP/1.1\r\nHost: localhost\r\n\r\n");
            assert!(found.starts_with(b"HTTP/1.1 200 OK\r\n"));
            assert!(find_bytes(
                &found,
                b"Content-Type: text/html; charset=utf-8\r\n"
            ));

            let sedat = request(address, b"GET /sedat HTTP/1.1\r\nHost: localhost\r\n\r\n");
            assert!(sedat.starts_with(b"HTTP/1.1 200 OK\r\n"));

            let missing = request(
                address,
                b"GET /definitely-missing HTTP/1.1\r\nHost: localhost\r\n\r\n",
            );
            assert!(missing.starts_with(b"HTTP/1.1 404 Not Found\r\n"));
            assert!(find_bytes(
                &missing,
                b"Content-Type: text/html; charset=utf-8\r\n"
            ));
        });
    }

    fn request(address: std::net::SocketAddr, request: &[u8]) -> Vec<u8> {
        let mut stream = TcpStream::connect(address).unwrap();
        stream.write_all(request).unwrap();
        let mut response = Vec::new();
        stream.read_to_end(&mut response).unwrap();
        response
    }

    fn find_bytes(haystack: &[u8], needle: &[u8]) -> bool {
        haystack
            .windows(needle.len())
            .any(|window| window == needle)
    }
}
