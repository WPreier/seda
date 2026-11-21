//! Seda binary entrypoint.

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context;
use clap::Parser;
use tracing::{info, warn};

use seda::admin::{AdminServer, ReloadController, RuntimeState};
use seda::config::{self, is_safe_bind_address, SedaConfig};
use seda::content::ContentStore;
use seda::hardening::{self, bind_listener};
use seda::proxy;
use seda::VERSION;

#[derive(Debug, Parser)]
#[command(
    name = "seda",
    version = VERSION,
    about = "Static in-memory HTTP backend companion to SeDat"
)]
struct Cli {
    /// Path to the user-editable TOML config.
    #[arg(short = 'f', long = "config", default_value = "seda.toml")]
    config: PathBuf,

    /// Override the configured listen address.
    #[arg(long)]
    listen: Option<String>,

    /// Write and activate the embedded default config.
    #[arg(long)]
    resetconfig: bool,

    /// Validate and activate the config, then exit.
    #[arg(long)]
    validate: bool,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    if cli.resetconfig {
        let active = config::reset_config(&cli.config, None)?;
        println!(
            "reset config written to {} (active: {})",
            active.user_path.display(),
            active.active_path.display()
        );
        if !cli.validate {
            return Ok(());
        }
    } else if !cli.config.exists() {
        anyhow::bail!(
            "config not found: {} (use --resetconfig to create a default)",
            cli.config.display()
        );
    }

    let mut active = config::load_and_activate(&cli.config, None)?;
    if let Some(listen) = cli.listen {
        if active.config.sites.len() != 1 {
            anyhow::bail!("--listen can only override a configuration with exactly one site");
        }
        let address = listen
            .parse::<SocketAddr>()
            .with_context(|| format!("invalid --listen address `{listen}`"))?;
        if !is_safe_bind_address(address.ip()) {
            anyhow::bail!(
                "unsafe --listen address `{}`: only loopback, private, or link-local addresses are allowed",
                address.ip()
            );
        }
        active
            .config
            .sites
            .values_mut()
            .next()
            .expect("one site was checked")
            .listen = listen;
    }

    init_logging(&active.config.logging.level);

    if cli.validate {
        println!(
            "config OK (schema_version={})",
            active.config.schema_version
        );
        return Ok(());
    }

    run(active.config, active.user_path)
}

fn init_logging(level: &str) {
    let _ = tracing_subscriber::fmt().with_env_filter(level).try_init();
}

fn run(config: SedaConfig, user_config: PathBuf) -> anyhow::Result<()> {
    let mut content = BTreeMap::new();
    let mut listeners = Vec::new();
    for (name, site) in &config.sites {
        let address: SocketAddr = site
            .listen
            .parse()
            .with_context(|| format!("validated sites.{name}.listen became invalid"))?;
        let store = ContentStore::load_for_site(&config, site)
            .with_context(|| format!("failed to load static content for site `{name}`"))?;
        content.insert(name.clone(), Arc::new(store));
        listeners.push((name.clone(), address, bind_listener(address)?));
    }
    let admin = AdminServer::bind(std::path::Path::new(&config.server.admin_socket))
        .context("failed to bind admin socket")?;
    hardening::drop_privileges(&config.server).context("failed to drop privileges")?;

    let state_dir = PathBuf::from(&config.server.state_dir);
    let runtime = RuntimeState::new(config, content);
    let reload = ReloadController::new(user_config, state_dir, runtime.clone());
    admin.serve(runtime.clone(), reload.clone());
    install_sighup(reload)?;
    hardening::sd_notify_ready();
    hardening::spawn_watchdog();

    info!(
        version = VERSION,
        listeners = listeners.len(),
        files = runtime.content_len(),
        "starting Seda"
    );
    let (sender, receiver) = std::sync::mpsc::channel();
    for (name, address, listener) in listeners {
        let runtime = runtime.clone();
        let sender = sender.clone();
        std::thread::spawn(move || {
            let result = proxy::serve(listener, name.clone(), runtime)
                .with_context(|| format!("listener `{name}` ({address}) failed"));
            let _ = sender.send(result);
        });
    }
    drop(sender);
    receiver
        .recv()
        .context("all listener threads stopped unexpectedly")?
}

#[cfg(unix)]
fn install_sighup(reload: ReloadController) -> anyhow::Result<()> {
    use signal_hook::consts::signal::SIGHUP;
    use signal_hook::iterator::Signals;

    let mut signals = Signals::new([SIGHUP])?;
    std::thread::spawn(move || {
        for _ in signals.forever() {
            match reload.reload() {
                Ok(()) => info!("SIGHUP reload completed"),
                Err(error) => warn!(%error, "SIGHUP reload rejected; prior state retained"),
            }
        }
    });
    Ok(())
}

#[cfg(not(unix))]
fn install_sighup(_reload: ReloadController) -> anyhow::Result<()> {
    warn!("SIGHUP reload is unavailable on this platform");
    Ok(())
}
