//! `learn ui` — start the local web dashboard.

use camino::Utf8PathBuf;
use learn_core::Result;
use std::time::Duration;

const DEFAULT_PORT: u16 = 7878;

pub async fn run_ui(kb_root: Utf8PathBuf, port: Option<u16>) -> Result<()> {
    let port = port.unwrap_or(DEFAULT_PORT);
    let url = format!("http://127.0.0.1:{port}");

    // ── Seed discovery before starting the server ─────────────────────────
    check_and_configure_seed().await;

    println!("\ncognitum-learn dashboard → {url}");
    println!("Press Ctrl-C to stop.\n");

    // Open browser (best-effort)
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(&url).spawn();
    #[cfg(target_os = "linux")]
    let _ = std::process::Command::new("xdg-open").arg(&url).spawn();

    learn_serve::run_ui_server(kb_root, port)
        .await
        .map_err(|e| learn_core::LearnError::Acquire(format!("UI server: {e}")))?;

    Ok(())
}

/// Check Seed configuration and attempt mDNS discovery when not yet set.
/// Prints status and offers to auto-configure. Non-blocking if Seed is already
/// configured or if discovery finds nothing.
async fn check_and_configure_seed() {
    use crate::config::LearnConfig;

    let cfg = LearnConfig::load();

    if let Some(ref addr) = cfg.seed.address {
        // Already configured — probe reachability
        let reachable = tcp_probe(addr, 800).await;
        if reachable {
            println!("✓ Cognitum Seed  {addr}  reachable");
        } else {
            println!("⚠  Cognitum Seed  {addr}  not reachable — check it is powered on and on the same network");
        }
        return;
    }

    // Not configured — try mDNS
    println!("Cognitum Seed not configured. Scanning local network…");
    match discover_seed_mdns(3).await {
        Ok(addrs) if !addrs.is_empty() => {
            println!();
            for (i, addr) in addrs.iter().enumerate() {
                println!("  {}. Cognitum Seed found: {addr}", i + 1);
            }
            let addr = &addrs[0];
            println!();
            println!("  To connect: learn config set seed.address {addr}");
            println!("  For auto-push after every ingest: learn config set seed.auto_push true");
        }
        _ => {
            println!("  No Cognitum Seed found on local network.");
            println!("  To configure manually: learn config set seed.address <ip>");
            println!("  To discover automatically when pushing: learn push <topic>");
        }
    }
    println!();
}

/// Browse for `_cognitum._tcp.local.` with a timeout.
async fn discover_seed_mdns(timeout_secs: u64) -> learn_core::Result<Vec<String>> {
    use mdns_sd::{ServiceDaemon, ServiceEvent};

    let daemon =
        ServiceDaemon::new().map_err(|e| learn_core::LearnError::Acquire(format!("mDNS: {e}")))?;
    let receiver = daemon
        .browse("_cognitum._tcp.local.")
        .map_err(|e| learn_core::LearnError::Acquire(format!("mDNS browse: {e}")))?;

    let deadline = std::time::Instant::now() + Duration::from_secs(timeout_secs);
    let mut found = Vec::new();

    loop {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        if remaining.is_zero() {
            break;
        }
        if let Ok(ServiceEvent::ServiceResolved(info)) = receiver.recv_timeout(remaining) {
            let addr = info
                .get_addresses_v4()
                .into_iter()
                .next()
                .map(|a| a.to_string())
                .unwrap_or_else(|| info.get_hostname().trim_end_matches('.').to_owned());
            if !found.contains(&addr) {
                found.push(addr);
            }
        }
    }
    Ok(found)
}

/// TCP reachability probe — returns true if connection succeeds within timeout_ms.
async fn tcp_probe(addr: &str, timeout_ms: u64) -> bool {
    let addr_with_port = if addr.contains(':') {
        addr.to_string()
    } else {
        format!("{addr}:80")
    };
    tokio::time::timeout(
        Duration::from_millis(timeout_ms),
        tokio::net::TcpStream::connect(&addr_with_port),
    )
    .await
    .ok()
    .and_then(|r| r.ok())
    .is_some()
}
