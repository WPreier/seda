//! Serde schema for Seda TOML configuration.

use serde::{Deserialize, Serialize};

/// Root configuration document.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SedaConfig {
    pub schema_version: u32,
    #[serde(default)]
    pub server: ServerConfig,
    #[serde(default)]
    pub trust: TrustConfig,
    #[serde(default)]
    pub proxy: ProxyConfig,
    #[serde(default)]
    pub embedded_docs: EmbeddedDocsConfig,
    #[serde(default)]
    pub sites: Vec<SiteConfig>,
    #[serde(default)]
    pub limits: LimitsConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
}

/// Listener and state paths.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServerConfig {
    #[serde(default = "default_listen")]
    pub listen: String,
    #[serde(default = "default_state_dir")]
    pub state_dir: String,
    /// Unix-domain control socket.
    #[serde(default = "default_admin_socket")]
    pub admin_socket: String,
    /// Unprivileged user selected after listeners are bound.
    #[serde(default)]
    pub user: Option<String>,
    /// Unprivileged group selected before the user.
    #[serde(default)]
    pub group: Option<String>,
}

fn default_listen() -> String {
    "127.0.0.1:9080".into()
}

fn default_state_dir() -> String {
    "./state".into()
}

fn default_admin_socket() -> String {
    "./state/admin.sock".into()
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            listen: default_listen(),
            state_dir: default_state_dir(),
            admin_socket: default_admin_socket(),
            user: None,
            group: None,
        }
    }
}

/// Peers allowed to connect, expressed as IP addresses or CIDRs.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrustConfig {
    #[serde(default = "default_allowed_peers")]
    pub allowed_peers: Vec<String>,
}

fn default_allowed_peers() -> Vec<String> {
    vec!["127.0.0.1/32".into(), "::1/128".into()]
}

impl Default for TrustConfig {
    fn default() -> Self {
        Self {
            allowed_peers: default_allowed_peers(),
        }
    }
}

/// Accepted connection preamble policy.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProxyConfig {
    #[serde(default = "default_proxy_protocol")]
    pub protocol: String,
    /// Reject plain HTTP connections unless they carry an accepted PROXY header.
    #[serde(default)]
    pub require_proxy: bool,
}

fn default_proxy_protocol() -> String {
    "auto".into()
}

impl Default for ProxyConfig {
    fn default() -> Self {
        Self {
            protocol: default_proxy_protocol(),
            require_proxy: false,
        }
    }
}

/// Embedded documentation routes.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmbeddedDocsConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_sedat_path")]
    pub sedat_path: String,
    #[serde(default = "default_seda_path")]
    pub seda_path: String,
}

fn default_true() -> bool {
    true
}

fn default_sedat_path() -> String {
    "/sedat".into()
}

fn default_seda_path() -> String {
    "/seda".into()
}

impl Default for EmbeddedDocsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            sedat_path: default_sedat_path(),
            seda_path: default_seda_path(),
        }
    }
}

/// Optional filesystem site loaded into memory during startup.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SiteConfig {
    pub path: String,
    pub root: String,
}

/// Resource and request bounds.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LimitsConfig {
    #[serde(default = "default_max_connections")]
    pub max_connections: usize,
    #[serde(default = "default_max_header_bytes")]
    pub max_header_bytes: usize,
    #[serde(default = "default_max_headers")]
    pub max_headers: usize,
    #[serde(default = "default_request_timeout")]
    pub request_timeout_secs: u64,
    #[serde(default = "default_idle_timeout")]
    pub idle_timeout_secs: u64,
    #[serde(default = "default_max_content_bytes")]
    pub max_content_bytes: u64,
}

fn default_max_connections() -> usize {
    1024
}
fn default_max_header_bytes() -> usize {
    16_384
}
fn default_max_headers() -> usize {
    100
}
fn default_request_timeout() -> u64 {
    30
}
fn default_idle_timeout() -> u64 {
    60
}
fn default_max_content_bytes() -> u64 {
    64 * 1024 * 1024
}

impl Default for LimitsConfig {
    fn default() -> Self {
        Self {
            max_connections: default_max_connections(),
            max_header_bytes: default_max_header_bytes(),
            max_headers: default_max_headers(),
            request_timeout_secs: default_request_timeout(),
            idle_timeout_secs: default_idle_timeout(),
            max_content_bytes: default_max_content_bytes(),
        }
    }
}

/// Tracing filter level.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoggingConfig {
    #[serde(default = "default_log_level")]
    pub level: String,
}

fn default_log_level() -> String {
    "info".into()
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
        }
    }
}
