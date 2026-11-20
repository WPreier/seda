//! Semantic validation for Seda configuration.

use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;

use regex::Regex;

use super::schema::SedaConfig;

/// Validation failure with a best-effort source line.
#[derive(Debug, Clone)]
pub struct ValidationError {
    pub line: usize,
    pub message: String,
}

fn find_line(source: &str, needle: &str) -> usize {
    source
        .lines()
        .position(|line| line.contains(needle))
        .map_or(1, |line| line + 1)
}

fn error(source: &str, needle: &str, message: impl Into<String>) -> ValidationError {
    ValidationError {
        line: find_line(source, needle),
        message: message.into(),
    }
}

/// Validate a parsed config without changing the active configuration.
pub fn validate_config(cfg: &SedaConfig, source: &str) -> Result<(), ValidationError> {
    let listen = cfg.server.listen.parse::<SocketAddr>().map_err(|_| {
        error(
            source,
            &cfg.server.listen,
            format!("invalid server.listen `{}`", cfg.server.listen),
        )
    })?;
    if !is_safe_bind_address(listen.ip()) {
        return Err(error(
            source,
            &cfg.server.listen,
            format!(
                "server.listen address `{}` is not loopback, private, or link-local",
                listen.ip()
            ),
        ));
    }
    if cfg.server.state_dir.trim().is_empty() {
        return Err(error(
            source,
            "state_dir",
            "server.state_dir must not be empty",
        ));
    }
    if cfg.server.admin_socket.trim().is_empty() {
        return Err(error(
            source,
            "admin_socket",
            "server.admin_socket must not be empty",
        ));
    }
    if cfg
        .server
        .user
        .as_ref()
        .is_some_and(|value| value.trim().is_empty())
        || cfg
            .server
            .group
            .as_ref()
            .is_some_and(|value| value.trim().is_empty())
    {
        return Err(error(
            source,
            "[server]",
            "server.user and server.group must not be empty when configured",
        ));
    }

    if cfg.trust.allowed_peers.is_empty() {
        return Err(error(
            source,
            "allowed_peers",
            "trust.allowed_peers must not be empty (fail-closed)",
        ));
    }
    for peer in &cfg.trust.allowed_peers {
        parse_cidr(peer).map_err(|reason| {
            error(
                source,
                peer,
                format!("invalid trusted peer `{peer}`: {reason}"),
            )
        })?;
    }

    match cfg.proxy.protocol.as_str() {
        "auto" | "v1" | "v2" | "off" => {}
        protocol => {
            return Err(error(
                source,
                protocol,
                format!("proxy.protocol must be auto|v1|v2|off, got `{protocol}`"),
            ));
        }
    }
    if cfg.proxy.require_proxy && cfg.proxy.protocol == "off" {
        return Err(error(
            source,
            "require_proxy",
            "proxy.require_proxy cannot be true when proxy.protocol is off",
        ));
    }

    validate_url_path(
        source,
        "embedded_docs.sedat_path",
        &cfg.embedded_docs.sedat_path,
    )?;
    validate_url_path(
        source,
        "embedded_docs.seda_path",
        &cfg.embedded_docs.seda_path,
    )?;
    if cfg.embedded_docs.sedat_path == cfg.embedded_docs.seda_path {
        return Err(error(
            source,
            &cfg.embedded_docs.seda_path,
            "embedded documentation paths must be distinct",
        ));
    }

    for site in &cfg.sites {
        validate_url_path(source, "sites.path", &site.path)?;
        if site.root.trim().is_empty() {
            return Err(error(source, "root", "sites.root must not be empty"));
        }
    }

    if cfg.limits.max_connections == 0
        || cfg.limits.max_header_bytes == 0
        || cfg.limits.max_headers == 0
        || cfg.limits.request_timeout_secs == 0
        || cfg.limits.idle_timeout_secs == 0
        || cfg.limits.max_content_bytes == 0
    {
        return Err(error(
            source,
            "[limits]",
            "all limits must be greater than zero",
        ));
    }

    match cfg.logging.level.as_str() {
        "error" | "warn" | "info" | "debug" | "trace" => {}
        level => {
            return Err(error(
                source,
                level,
                format!("logging.level must be error|warn|info|debug|trace, got `{level}`"),
            ));
        }
    }

    Ok(())
}

/// Whether an address is safe for the backend listener.
#[must_use]
pub fn is_safe_bind_address(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => ip.is_loopback() || ip.is_private() || ip.is_link_local(),
        IpAddr::V6(ip) => {
            let first = ip.segments()[0];
            ip.is_loopback() || first & 0xfe00 == 0xfc00 || first & 0xffc0 == 0xfe80
        }
    }
}

fn validate_url_path(source: &str, field: &str, value: &str) -> Result<(), ValidationError> {
    let valid = Regex::new(r"^/[A-Za-z0-9._~!$&'()*+,;=:@%/-]*$")
        .expect("static path regex")
        .is_match(value);
    if !valid || value.contains("..") || (value.len() > 1 && value.ends_with('/')) {
        return Err(error(
            source,
            value,
            format!("{field} must be a normalized absolute URL path"),
        ));
    }
    Ok(())
}

/// Parse an IP address or CIDR prefix.
pub fn parse_cidr(value: &str) -> Result<(IpAddr, u8), String> {
    if let Some((address, prefix)) = value.split_once('/') {
        let address = IpAddr::from_str(address.trim()).map_err(|error| error.to_string())?;
        let prefix: u8 = prefix
            .trim()
            .parse()
            .map_err(|error: std::num::ParseIntError| error.to_string())?;
        let maximum = if address.is_ipv4() { 32 } else { 128 };
        if prefix > maximum {
            return Err(format!("prefix {prefix} exceeds {maximum}"));
        }
        Ok((address, prefix))
    } else {
        let address = IpAddr::from_str(value.trim()).map_err(|error| error.to_string())?;
        let prefix = if address.is_ipv4() { 32 } else { 128 };
        Ok((address, prefix))
    }
}

/// Check whether an IP address belongs to at least one configured CIDR.
#[must_use]
pub fn ip_in_allowlist(ip: IpAddr, list: &[String]) -> bool {
    list.iter().any(|cidr| {
        parse_cidr(cidr)
            .map(|(network, prefix)| ip_in_cidr(ip, network, prefix))
            .unwrap_or(false)
    })
}

fn ip_in_cidr(ip: IpAddr, network: IpAddr, prefix: u8) -> bool {
    match (ip, network) {
        (IpAddr::V4(ip), IpAddr::V4(network)) => {
            let mask = if prefix == 0 {
                0
            } else {
                u32::MAX << (32 - prefix)
            };
            (u32::from(ip) & mask) == (u32::from(network) & mask)
        }
        (IpAddr::V6(ip), IpAddr::V6(network)) => {
            let mask = if prefix == 0 {
                0
            } else {
                u128::MAX << (128 - prefix)
            };
            (u128::from(ip) & mask) == (u128::from(network) & mask)
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::parse_toml;

    #[test]
    fn safe_bind_addresses_are_non_global() {
        assert!(is_safe_bind_address("127.0.0.1".parse().unwrap()));
        assert!(is_safe_bind_address("10.2.3.4".parse().unwrap()));
        assert!(is_safe_bind_address("169.254.1.1".parse().unwrap()));
        assert!(is_safe_bind_address("::1".parse().unwrap()));
        assert!(is_safe_bind_address("fd00::1".parse().unwrap()));
        assert!(is_safe_bind_address("fe80::1".parse().unwrap()));
        assert!(!is_safe_bind_address("8.8.8.8".parse().unwrap()));
        assert!(!is_safe_bind_address(
            "2001:4860:4860::8888".parse().unwrap()
        ));
        assert!(!is_safe_bind_address("0.0.0.0".parse().unwrap()));
    }

    #[test]
    fn cidr_allowlist_matches_both_families() {
        assert!(ip_in_allowlist(
            "10.42.0.9".parse().unwrap(),
            &["10.0.0.0/8".into()]
        ));
        assert!(ip_in_allowlist(
            "fd12::9".parse().unwrap(),
            &["fd00::/8".into()]
        ));
        assert!(!ip_in_allowlist(
            "192.0.2.1".parse().unwrap(),
            &["10.0.0.0/8".into()]
        ));
    }

    #[test]
    fn config_rejects_global_listener() {
        let error = parse_toml(
            "schema_version = 1\n[server]\nlisten = \"8.8.8.8:8080\"\nstate_dir = \"state\"\n",
        )
        .unwrap_err();
        assert!(error
            .to_string()
            .contains("not loopback, private, or link-local"));
    }

    #[test]
    fn require_proxy_conflicts_with_off_mode() {
        let error =
            parse_toml("schema_version = 1\n[proxy]\nprotocol = \"off\"\nrequire_proxy = true\n")
                .unwrap_err();
        assert!(error.to_string().contains("cannot be true"));
    }
}
