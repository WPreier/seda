//! Startup hardening helpers.

mod bind;

pub use bind::{bind_listener, BindError};

use std::thread;
use std::time::Duration;

#[cfg(unix)]
use tracing::info;
use tracing::warn;

use crate::config::ServerConfig;

/// Drop to `server.user` / `server.group` after all listeners are bound.
pub fn drop_privileges(server: &ServerConfig) -> std::io::Result<()> {
    let Some(user_name) = server.user.as_deref() else {
        return Ok(());
    };

    #[cfg(unix)]
    {
        use nix::unistd::{setgid, setuid, Group, User};

        let user = User::from_name(user_name)
            .map_err(std::io::Error::other)?
            .ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("user `{user_name}` not found"),
                )
            })?;
        let gid = match server.group.as_deref() {
            Some(group_name) => {
                Group::from_name(group_name)
                    .map_err(std::io::Error::other)?
                    .ok_or_else(|| {
                        std::io::Error::new(
                            std::io::ErrorKind::NotFound,
                            format!("group `{group_name}` not found"),
                        )
                    })?
                    .gid
            }
            None => user.gid,
        };
        setgid(gid).map_err(std::io::Error::other)?;
        setuid(user.uid).map_err(std::io::Error::other)?;
        info!(user = user_name, "privileges dropped");
    }

    #[cfg(not(unix))]
    warn!(
        user = user_name,
        "privilege drop is unavailable on this platform"
    );

    Ok(())
}

/// Send systemd `READY=1` when a notify socket is present.
pub fn sd_notify_ready() {
    if let Err(error) = sd_notify(b"READY=1") {
        warn!(%error, "systemd readiness notification failed");
    }
}

/// Start best-effort watchdog pings at half the configured interval.
pub fn spawn_watchdog() {
    let Some(microseconds) = std::env::var("WATCHDOG_USEC")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
    else {
        return;
    };
    thread::spawn(move || {
        let interval = Duration::from_micros((microseconds / 2).max(1));
        loop {
            thread::sleep(interval);
            if let Err(error) = sd_notify(b"WATCHDOG=1") {
                warn!(%error, "systemd watchdog notification failed");
            }
        }
    });
}

#[cfg(unix)]
fn sd_notify(payload: &[u8]) -> std::io::Result<()> {
    use std::os::unix::net::UnixDatagram;

    let Some(socket_name) = std::env::var_os("NOTIFY_SOCKET") else {
        return Ok(());
    };
    let socket_name = socket_name.to_string_lossy();
    let socket = UnixDatagram::unbound()?;

    #[cfg(target_os = "linux")]
    if let Some(abstract_name) = socket_name.strip_prefix('@') {
        use std::os::linux::net::SocketAddrExt;
        let address = std::os::unix::net::SocketAddr::from_abstract_name(abstract_name.as_bytes())?;
        socket.send_to_addr(payload, &address)?;
        return Ok(());
    }

    socket.send_to(payload, socket_name.as_ref())?;
    Ok(())
}

#[cfg(not(unix))]
fn sd_notify(_payload: &[u8]) -> std::io::Result<()> {
    Ok(())
}
