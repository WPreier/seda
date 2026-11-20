use std::fs;
use std::io::Write;
use std::net::{TcpListener, TcpStream};
use std::thread;

use seda::admin::{ReloadController, RuntimeState};
use seda::config::{self, EmbeddedDocsConfig, SiteConfig};
use seda::content::ContentStore;
use seda::proxy::{probe, ProxyMode, ProxyProtocolError};
use tempfile::tempdir;

#[test]
fn default_contract_uses_sedat_upstream_port_and_demo_routes() {
    let config = config::parse_toml(seda::DEFAULT_CONFIG_TOML).unwrap();
    assert_eq!(config.server.listen, "127.0.0.1:9080");
    let store = ContentStore::load(&config).unwrap();
    assert!(store.lookup("/seda").is_some());
    assert!(store.lookup("/sedat").is_some());
    assert!(store.lookup("/missing").is_none());
    assert!(!store.not_found().bytes().is_empty());
}

#[test]
fn root_site_is_loaded_into_memory() {
    let directory = tempdir().unwrap();
    fs::write(directory.path().join("index.html"), "site root").unwrap();
    let store = ContentStore::new(
        &EmbeddedDocsConfig::default(),
        &[SiteConfig {
            path: "/".into(),
            root: directory.path().to_string_lossy().into_owned(),
        }],
        1024,
    )
    .unwrap();
    assert_eq!(store.lookup("/").unwrap().bytes(), b"site root");
}

#[test]
fn config_is_fail_closed_for_empty_trust_and_public_bind() {
    let no_peers = seda::DEFAULT_CONFIG_TOML.replace(
        "allowed_peers = [\"127.0.0.1/32\", \"::1/128\"]",
        "allowed_peers = []",
    );
    assert!(config::parse_toml(&no_peers)
        .unwrap_err()
        .to_string()
        .contains("fail-closed"));

    let public = seda::DEFAULT_CONFIG_TOML.replace("127.0.0.1:9080", "8.8.8.8:9080");
    assert!(config::parse_toml(&public)
        .unwrap_err()
        .to_string()
        .contains("not loopback, private, or link-local"));
}

#[test]
fn reload_error_preserves_runtime_and_active_config() {
    let directory = tempdir().unwrap();
    let user = directory.path().join("seda.toml");
    let state = directory.path().join("state");
    fs::write(&user, seda::DEFAULT_CONFIG_TOML).unwrap();
    let active = config::load_and_activate(&user, Some(&state)).unwrap();
    let runtime = RuntimeState::new(
        active.config.clone(),
        ContentStore::load(&active.config).unwrap(),
    );
    let reload = ReloadController::new(user.clone(), state.clone(), runtime.clone());

    fs::write(&user, "schema_version = 1\n[server\n").unwrap();
    assert!(reload.reload().is_err());
    assert_eq!(runtime.load_config().server.listen, "127.0.0.1:9080");
    assert_eq!(
        config::load_active(&state).unwrap().server.listen,
        "127.0.0.1:9080"
    );
}

#[test]
fn proxy_v1_and_v2_are_parsed_from_trusted_transport() {
    let v1 = probe_over_tcp(
        b"PROXY TCP4 203.0.113.9 127.0.0.1 4312 9080\r\nGET / HTTP/1.1\r\n",
        ProxyMode::Auto,
        true,
    )
    .unwrap();
    assert_eq!(v1.client.to_string(), "203.0.113.9:4312");
    assert!(v1.proxied);

    let mut v2 = b"\r\n\r\n\0\r\nQUIT\n".to_vec();
    v2.extend_from_slice(&[0x21, 0x11, 0, 12]);
    v2.extend_from_slice(&[203, 0, 113, 10, 127, 0, 0, 1]);
    v2.extend_from_slice(&4313u16.to_be_bytes());
    v2.extend_from_slice(&9080u16.to_be_bytes());
    let parsed = probe_over_tcp(&v2, ProxyMode::V2, true).unwrap();
    assert_eq!(parsed.client.to_string(), "203.0.113.10:4313");
}

#[test]
fn proxy_policy_rejects_plain_or_wrong_version() {
    assert!(matches!(
        probe_over_tcp(b"GET / HTTP/1.1\r\n", ProxyMode::Auto, true),
        Err(ProxyProtocolError::ProxyRequired)
    ));
    assert!(matches!(
        probe_over_tcp(
            b"PROXY TCP4 203.0.113.9 127.0.0.1 4312 9080\r\n",
            ProxyMode::V2,
            false
        ),
        Err(ProxyProtocolError::VersionNotAllowed)
    ));
}

fn probe_over_tcp(
    bytes: &[u8],
    mode: ProxyMode,
    require_proxy: bool,
) -> Result<seda::proxy::ProbeResult, ProxyProtocolError> {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let payload = bytes.to_vec();
    let sender = thread::spawn(move || {
        let mut stream = TcpStream::connect(address).unwrap();
        stream.write_all(&payload).unwrap();
    });
    let (mut stream, peer) = listener.accept().unwrap();
    let result = probe(&mut stream, peer, mode, require_proxy);
    sender.join().unwrap();
    result
}
