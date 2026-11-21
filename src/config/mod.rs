//! TOML configuration lifecycle: parse, validate, activate, and reload.

mod schema;
mod validate;

pub use schema::*;
pub use validate::{
    ip_in_allowlist, is_safe_bind_address, parse_cidr, validate_config, ValidationError,
};

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use thiserror::Error;

/// Name of the validated active configuration copy.
pub const ACTIVE_CONFIG_NAME: &str = ".seda.active.toml";

/// Current configuration schema. Schema 1 is accepted and normalized to this version.
pub const CURRENT_SCHEMA_VERSION: u32 = 2;

/// Configuration load or activation failure.
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("TOML parse error at line {line}: {message}")]
    Parse { line: usize, message: String },
    #[error("validation error at line {line}: {message}")]
    Validation { line: usize, message: String },
    #[error("unsupported schema_version {found}; supported: 1 and {CURRENT_SCHEMA_VERSION}")]
    SchemaVersion { found: u32 },
}

impl From<ValidationError> for ConfigError {
    fn from(error: ValidationError) -> Self {
        Self::Validation {
            line: error.line,
            message: error.message,
        }
    }
}

/// A validated configuration and its user/active paths.
#[derive(Debug, Clone)]
pub struct ActiveConfig {
    pub config: SedaConfig,
    pub active_path: PathBuf,
    pub user_path: PathBuf,
}

/// Parsed reload candidate whose source has not yet been activated.
#[derive(Debug)]
pub struct ConfigCandidate {
    pub config: SedaConfig,
    source: String,
    user_path: PathBuf,
}

/// Parse and validate TOML without activating it.
pub fn parse_toml(source: &str) -> Result<SedaConfig, ConfigError> {
    let mut document: toml::Value = toml::from_str(source).map_err(|error| {
        let line = error
            .span()
            .map(|span| offset_to_line(source, span.start))
            .unwrap_or(1);
        ConfigError::Parse {
            line,
            message: error.message().to_owned(),
        }
    })?;
    let version = document
        .get("schema_version")
        .and_then(toml::Value::as_integer)
        .and_then(|value| u32::try_from(value).ok())
        .unwrap_or(0);
    if version != 1 && version != CURRENT_SCHEMA_VERSION {
        return Err(ConfigError::SchemaVersion { found: version });
    }
    if version == 1 {
        migrate_v1(&mut document)?;
    }
    let config: SedaConfig = document.try_into().map_err(|error: toml::de::Error| {
        let line = error
            .span()
            .map(|span| offset_to_line(source, span.start))
            .unwrap_or(1);
        ConfigError::Parse {
            line,
            message: error.message().to_owned(),
        }
    })?;
    validate_config(&config, source)?;
    Ok(config)
}

fn migrate_v1(document: &mut toml::Value) -> Result<(), ConfigError> {
    let root = document.as_table_mut().ok_or_else(|| ConfigError::Parse {
        line: 1,
        message: "configuration root must be a table".into(),
    })?;
    let listen = root
        .entry("server")
        .or_insert_with(|| toml::Value::Table(toml::map::Map::new()))
        .as_table_mut()
        .ok_or_else(|| ConfigError::Parse {
            line: 1,
            message: "server must be a table".into(),
        })?
        .remove("listen")
        .unwrap_or_else(|| toml::Value::String(default_listen()));
    let roots = match root.remove("sites") {
        Some(toml::Value::Array(roots)) => roots,
        Some(_) => {
            return Err(ConfigError::Parse {
                line: 1,
                message: "schema_version 1 sites must be an array of tables".into(),
            });
        }
        None => Vec::new(),
    };
    let mut site1 = toml::map::Map::new();
    site1.insert("listen".into(), listen);
    site1.insert("roots".into(), toml::Value::Array(roots));
    let mut sites = toml::map::Map::new();
    sites.insert("site1".into(), toml::Value::Table(site1));
    root.insert("sites".into(), toml::Value::Table(sites));
    root.insert(
        "schema_version".into(),
        toml::Value::Integer(CURRENT_SCHEMA_VERSION.into()),
    );
    Ok(())
}

/// Validate a user config and copy it to `.seda.active.toml`.
pub fn load_and_activate(
    user_path: &Path,
    state_dir: Option<&Path>,
) -> Result<ActiveConfig, ConfigError> {
    let source = fs::read_to_string(user_path)?;
    let config = parse_toml(&source)?;
    let active_dir = state_dir
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from(&config.server.state_dir));
    fs::create_dir_all(&active_dir)?;
    let active_path = active_dir.join(ACTIVE_CONFIG_NAME);
    write_active(&active_path, &source)?;
    Ok(ActiveConfig {
        config,
        active_path,
        user_path: user_path.to_path_buf(),
    })
}

/// Load and validate the active copy.
pub fn load_active(state_dir: &Path) -> Result<SedaConfig, ConfigError> {
    let source = fs::read_to_string(state_dir.join(ACTIVE_CONFIG_NAME))?;
    parse_toml(&source)
}

/// Attempt a reload, leaving the active copy untouched on parse/validation failure.
pub fn try_reload(user_path: &Path, state_dir: &Path) -> Result<ActiveConfig, ConfigError> {
    activate_candidate(prepare_reload(user_path)?, state_dir)
}

/// Read and validate a reload candidate without changing the active copy.
pub fn prepare_reload(user_path: &Path) -> Result<ConfigCandidate, ConfigError> {
    let source = fs::read_to_string(user_path)?;
    let config = parse_toml(&source)?;
    Ok(ConfigCandidate {
        config,
        source,
        user_path: user_path.to_path_buf(),
    })
}

/// Persist an already validated candidate as the active configuration.
pub fn activate_candidate(
    candidate: ConfigCandidate,
    state_dir: &Path,
) -> Result<ActiveConfig, ConfigError> {
    fs::create_dir_all(state_dir)?;
    let active_path = state_dir.join(ACTIVE_CONFIG_NAME);
    write_active(&active_path, &candidate.source)?;
    Ok(ActiveConfig {
        config: candidate.config,
        active_path,
        user_path: candidate.user_path,
    })
}

/// Replace the user config with the embedded default and activate it.
pub fn reset_config(
    user_path: &Path,
    state_dir: Option<&Path>,
) -> Result<ActiveConfig, ConfigError> {
    if let Some(parent) = user_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(user_path, crate::DEFAULT_CONFIG_TOML)?;
    load_and_activate(user_path, state_dir)
}

fn write_active(path: &Path, source: &str) -> Result<(), ConfigError> {
    fs::write(path, source)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = fs::metadata(path)?.permissions();
        permissions.set_mode(0o600);
        fs::set_permissions(path, permissions)?;
    }
    Ok(())
}

fn offset_to_line(source: &str, offset: usize) -> usize {
    source[..offset.min(source.len())]
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count()
        + 1
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn embedded_default_is_valid() {
        let config = parse_toml(crate::DEFAULT_CONFIG_TOML).expect("default config must validate");
        assert_eq!(config.schema_version, CURRENT_SCHEMA_VERSION);
    }

    #[test]
    fn schema_v1_is_migrated_to_site1_in_memory() {
        let config = parse_toml(
            "schema_version = 1\n[server]\nlisten = \"127.0.0.1:9444\"\n\
             [[sites]]\npath = \"/legacy\"\nroot = \".\"\n",
        )
        .unwrap();
        assert_eq!(config.schema_version, 2);
        assert_eq!(config.sites["site1"].listen, "127.0.0.1:9444");
        assert_eq!(config.sites["site1"].roots[0].path, "/legacy");
    }

    #[test]
    fn parse_error_has_a_line() {
        let error = parse_toml("schema_version = 1\n[server\nlisten = \"bad\"\n")
            .expect_err("invalid TOML must fail");
        assert!(matches!(error, ConfigError::Parse { line, .. } if line >= 2));
    }

    #[test]
    fn failed_reload_preserves_active_copy() {
        let directory = tempdir().expect("temporary directory");
        let user_path = directory.path().join("seda.toml");
        let state_dir = directory.path().join("state");
        fs::write(&user_path, crate::DEFAULT_CONFIG_TOML).expect("write default");
        load_and_activate(&user_path, Some(&state_dir)).expect("activate default");

        fs::write(&user_path, "schema_version = 1\n[server\n").expect("write invalid config");
        assert!(try_reload(&user_path, &state_dir).is_err());

        let active = load_active(&state_dir).expect("old active config remains valid");
        assert_eq!(active.schema_version, CURRENT_SCHEMA_VERSION);
    }

    #[test]
    fn reset_writes_user_and_active_configs() {
        let directory = tempdir().expect("temporary directory");
        let user_path = directory.path().join("seda.toml");
        let state_dir = directory.path().join("state");
        let active = reset_config(&user_path, Some(&state_dir)).expect("reset config");
        assert!(user_path.exists());
        assert!(active.active_path.exists());
    }
}
