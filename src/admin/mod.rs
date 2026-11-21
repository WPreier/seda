//! Local administration and the shared fail-closed reload path.

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, Mutex, RwLock};

use tracing::{info, warn};

#[cfg(unix)]
use std::sync::atomic::Ordering;
#[cfg(unix)]
use std::thread;

use crate::config::{self, SedaConfig};
use crate::content::ContentStore;

/// Runtime state shared by HTTP workers, admin clients, and signal handlers.
#[derive(Clone)]
pub struct RuntimeState {
    snapshot: Arc<RwLock<Arc<RuntimeSnapshot>>>,
    pub active_connections: Arc<AtomicUsize>,
}

struct RuntimeSnapshot {
    config: Arc<SedaConfig>,
    content: BTreeMap<String, Arc<ContentStore>>,
}

impl RuntimeState {
    pub fn new(config: SedaConfig, content: BTreeMap<String, Arc<ContentStore>>) -> Self {
        Self {
            snapshot: Arc::new(RwLock::new(Arc::new(RuntimeSnapshot {
                config: Arc::new(config),
                content,
            }))),
            active_connections: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn load_config(&self) -> Arc<SedaConfig> {
        Arc::clone(&self.load_snapshot().config)
    }

    pub fn load_site(&self, name: &str) -> Option<(Arc<SedaConfig>, Arc<ContentStore>)> {
        let snapshot = self.load_snapshot();
        snapshot
            .content
            .get(name)
            .map(|content| (Arc::clone(&snapshot.config), Arc::clone(content)))
    }

    pub fn content_len(&self) -> usize {
        self.load_snapshot()
            .content
            .values()
            .map(|store| store.len())
            .sum()
    }

    fn load_snapshot(&self) -> Arc<RuntimeSnapshot> {
        Arc::clone(
            &self
                .snapshot
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        )
    }
}

/// Serializes reloads and applies config/content as one prepared transaction.
#[derive(Clone)]
pub struct ReloadController {
    user_config: PathBuf,
    state_dir: PathBuf,
    runtime: RuntimeState,
    lock: Arc<Mutex<()>>,
}

impl ReloadController {
    pub fn new(user_config: PathBuf, state_dir: PathBuf, runtime: RuntimeState) -> Self {
        Self {
            user_config,
            state_dir,
            runtime,
            lock: Arc::new(Mutex::new(())),
        }
    }

    pub fn reload(&self) -> Result<(), String> {
        let _guard = self.lock.lock().unwrap_or_else(|error| error.into_inner());
        let candidate = config::prepare_reload(&self.user_config).map_err(|e| e.to_string())?;
        let current = self.runtime.load_config();
        let current_listeners: BTreeMap<_, _> = current
            .sites
            .iter()
            .map(|(name, site)| (name, &site.listen))
            .collect();
        let next_listeners: BTreeMap<_, _> = candidate
            .config
            .sites
            .iter()
            .map(|(name, site)| (name, &site.listen))
            .collect();
        if current_listeners != next_listeners {
            return Err("listener names/addresses cannot change on reload; restart Seda".into());
        }
        let mut content = BTreeMap::new();
        for (name, site) in &candidate.config.sites {
            let store =
                ContentStore::load_for_site(&candidate.config, site).map_err(|e| e.to_string())?;
            content.insert(name.clone(), Arc::new(store));
        }
        let next_config = candidate.config.clone();
        config::activate_candidate(candidate, &self.state_dir).map_err(|e| e.to_string())?;
        *self
            .runtime
            .snapshot
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Arc::new(RuntimeSnapshot {
            config: Arc::new(next_config),
            content,
        });
        info!("configuration and in-memory sites reloaded");
        Ok(())
    }
}

/// Bound admin socket. Binding occurs before privilege drop.
pub struct AdminServer {
    #[cfg(unix)]
    listener: std::os::unix::net::UnixListener,
    #[cfg(unix)]
    _path: PathBuf,
}

impl AdminServer {
    pub fn bind(path: &Path) -> io::Result<Self> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            if path.exists() {
                std::fs::remove_file(path)?;
            }
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let listener = std::os::unix::net::UnixListener::bind(path)?;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
            info!(path = %path.display(), mode = "0600", "admin socket bound");
            Ok(Self {
                listener,
                _path: path.to_path_buf(),
            })
        }

        #[cfg(not(unix))]
        {
            let _ = path;
            warn!("admin Unix socket is unavailable on this platform");
            Ok(Self {})
        }
    }

    pub fn serve(self, runtime: RuntimeState, reload: ReloadController) {
        #[cfg(unix)]
        thread::spawn(move || {
            for accepted in self.listener.incoming() {
                match accepted {
                    Ok(stream) => {
                        let runtime = runtime.clone();
                        let reload = reload.clone();
                        thread::spawn(move || {
                            if let Err(error) = handle_client(stream, &runtime, &reload) {
                                warn!(%error, "admin client error");
                            }
                        });
                    }
                    Err(error) => warn!(%error, "admin accept error"),
                }
            }
        });

        #[cfg(not(unix))]
        {
            let _ = (self, runtime, reload);
        }
    }
}

#[cfg(unix)]
fn handle_client(
    stream: std::os::unix::net::UnixStream,
    runtime: &RuntimeState,
    reload: &ReloadController,
) -> io::Result<()> {
    use std::io::{BufRead, BufReader, Write};

    let mut writer = stream.try_clone()?;
    let mut reader = BufReader::new(stream);
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            return Ok(());
        }
        let command = line.trim().to_ascii_lowercase();
        let reply = match command.as_str() {
            "status" => {
                let config = runtime.load_config();
                format!(
                    "OK seda {} connections={} files={} listen={}\n",
                    crate::VERSION,
                    runtime.active_connections.load(Ordering::Acquire),
                    runtime.content_len(),
                    config
                        .sites
                        .values()
                        .map(|site| site.listen.as_str())
                        .collect::<Vec<_>>()
                        .join(",")
                )
            }
            "reload" => match reload.reload() {
                Ok(()) => "OK reload\n".to_owned(),
                Err(error) => format!("ERR reload: {error}\n"),
            },
            "help" => "commands: status | reload | help | quit\n".to_owned(),
            "quit" => {
                writer.write_all(b"OK bye\n")?;
                return Ok(());
            }
            other => format!("ERR unknown command: {other}\n"),
        };
        writer.write_all(reply.as_bytes())?;
        writer.flush()?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_site_reload_keeps_config_and_active_copy() {
        let directory = tempfile::tempdir().unwrap();
        let user_path = directory.path().join("seda.toml");
        let state_dir = directory.path().join("state");
        std::fs::write(&user_path, crate::DEFAULT_CONFIG_TOML).unwrap();
        let active = config::load_and_activate(&user_path, Some(&state_dir)).unwrap();
        let content = active
            .config
            .sites
            .iter()
            .map(|(name, site)| {
                (
                    name.clone(),
                    Arc::new(ContentStore::load_for_site(&active.config, site).unwrap()),
                )
            })
            .collect();
        let runtime = RuntimeState::new(active.config, content);
        let reload = ReloadController::new(user_path.clone(), state_dir.clone(), runtime.clone());

        let invalid_site = format!(
            "{}\n[[sites.site1.roots]]\npath = \"/bad\"\nroot = \"../escape\"\n",
            crate::DEFAULT_CONFIG_TOML
        );
        std::fs::write(user_path, invalid_site).unwrap();
        assert!(reload.reload().is_err());
        assert!(runtime.load_config().sites["site1"].roots.is_empty());
        assert!(config::load_active(&state_dir).unwrap().sites["site1"]
            .roots
            .is_empty());
    }
}
