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

/// Resolution budget for the probe target, separate from the connect timeout.
/// mDNS `.local` names routinely take 1–2 s to resolve; sharing one 800 ms
/// budget between resolution and connect false-negatived on healthy Seeds.
const PROBE_RESOLVE_TIMEOUT_MS: u64 = 2500;

/// Derive the TCP probe target from a configured Seed address.
///
/// Mirrors `seed_query::build_query_url`: a bare host (IP or `.local` name)
/// gets the Seed's HTTPS API port 8443 — not port 80, which the Seed never
/// listens on. Explicit ports and scheme'd URLs are respected.
fn probe_target(addr: &str) -> String {
    let stripped = addr
        .trim()
        .trim_start_matches("https://")
        .trim_start_matches("http://");
    let host_port = stripped.split('/').next().unwrap_or(stripped);
    if host_port.contains(':') {
        host_port.to_string()
    } else {
        format!("{host_port}:{}", crate::seed_query::SEED_HTTPS_PORT)
    }
}

/// TCP reachability probe — returns true if connection succeeds within timeout_ms.
async fn tcp_probe(addr: &str, timeout_ms: u64) -> bool {
    let target = probe_target(addr);

    let resolved = tokio::time::timeout(
        Duration::from_millis(PROBE_RESOLVE_TIMEOUT_MS),
        tokio::net::lookup_host(target),
    )
    .await;
    let addrs: Vec<std::net::SocketAddr> = match resolved {
        Ok(Ok(iter)) => iter.collect(),
        _ => return false,
    };

    for sock_addr in addrs.into_iter().take(3) {
        let connected = tokio::time::timeout(
            Duration::from_millis(timeout_ms),
            tokio::net::TcpStream::connect(sock_addr),
        )
        .await
        .ok()
        .and_then(|r| r.ok())
        .is_some();
        if connected {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_target_bare_ip_uses_seed_https_port() {
        assert_eq!(probe_target("10.0.0.72"), "10.0.0.72:8443");
    }

    #[test]
    fn probe_target_bare_mdns_name_uses_seed_https_port() {
        assert_eq!(
            probe_target("cognitum-9842.local"),
            "cognitum-9842.local:8443"
        );
    }

    #[test]
    fn probe_target_explicit_port_is_respected() {
        assert_eq!(probe_target("10.0.0.72:9000"), "10.0.0.72:9000");
    }

    #[test]
    fn probe_target_strips_scheme_and_path() {
        assert_eq!(probe_target("https://10.0.0.72:8443/"), "10.0.0.72:8443");
        assert_eq!(probe_target("http://10.0.0.72"), "10.0.0.72:8443");
    }

    #[tokio::test]
    async fn tcp_probe_true_for_listening_socket() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        assert!(tcp_probe(&addr.to_string(), 800).await);
    }

    #[tokio::test]
    async fn tcp_probe_false_for_closed_port() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);
        assert!(!tcp_probe(&addr.to_string(), 800).await);
    }
}
