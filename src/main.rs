//! Seda binary entrypoint.

use std::net::SocketAddr;
use std::path::PathBuf;

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
        let address = listen
            .parse::<SocketAddr>()
            .with_context(|| format!("invalid --listen address `{listen}`"))?;
        if !is_safe_bind_address(address.ip()) {
            anyhow::bail!(
                "unsafe --listen address `{}`: only loopback, private, or link-local addresses are allowed",
                address.ip()
            );
        }
        active.config.server.listen = listen;
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
    let address: SocketAddr = config
        .server
        .listen
        .parse()
        .context("validated server.listen became invalid")?;
    let content = ContentStore::load(&config).context("failed to load static content")?;
    let listener = bind_listener(address)?;
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
        listen = %address,
        files = runtime.content.len(),
        "starting Seda"
    );
    proxy::serve(listener, runtime).context("listener failed")
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
