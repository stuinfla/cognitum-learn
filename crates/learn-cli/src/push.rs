//! `learn push <topic> --seed <address> [--token <bearer>]` — push a topic's
//! vectors to a Cognitum One Seed device over LAN (or USB-gadget link).
//!
//! Matches the Seed's published JSON ingest contract:
//!
//! ```text
//! POST http://{address}/api/v1/store/ingest
//! Authorization: Bearer <token>      (optional on USB-gadget link)
//! Content-Type: application/json
//! Body: {"vectors":[[id_u64, [f32, f32, ...]], ...]}
//! ```
//!
//! Vectors are batched to stay under the Seed's 64 KB HTTP body limit
//! (`TARGET_BATCH_BYTES` = 50 000 bytes leaves headroom for JSON overhead).

#![deny(unsafe_code)]

use camino::Utf8PathBuf;
use learn_core::{LearnError, Topic};
use learn_index::LearnIndex;
use serde::Serialize;
use std::time::Duration;

/// Stay safely under the Seed's 64 KB body cap.
const TARGET_BATCH_BYTES: usize = 50_000;

#[derive(Serialize)]
struct IngestBody<'a> {
    vectors: Vec<(u64, &'a [f32])>,
}

/// Resolve the seed address: return the provided address directly, or discover
/// via mDNS if none is given.  When multiple Seeds are found and `seed_index`
/// is `Some(n)` (1-based), the n-th result is chosen without prompting.
///
/// Exposed for unit testing.
pub(crate) async fn resolve_seed_address(
    seed: Option<String>,
    seed_index: Option<usize>,
) -> learn_core::Result<String> {
    match seed {
        Some(addr) => Ok(addr),
        None => discover_via_mdns(seed_index).await,
    }
}

/// Construct the `.rvf` file path for a topic under `kb_root`.
///
/// Exposed for unit testing.
pub(crate) fn rvf_path_for_topic(topic: &str, kb_root: &Utf8PathBuf) -> Utf8PathBuf {
    kb_root.join(format!("{topic}.rvf"))
}

/// Browse for `_cognitum._tcp.local.` with a 5-second timeout.
async fn discover_via_mdns(seed_index: Option<usize>) -> learn_core::Result<String> {
    use mdns_sd::{ServiceDaemon, ServiceEvent};

    let daemon = ServiceDaemon::new()
        .map_err(|e| LearnError::Acquire(format!("failed to start mDNS daemon: {e}")))?;

    let receiver = daemon
        .browse("_cognitum._tcp.local.")
        .map_err(|e| LearnError::Acquire(format!("mDNS browse failed: {e}")))?;

    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    let mut found: Vec<String> = Vec::new();

    loop {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        if remaining.is_zero() {
            break;
        }
        match receiver.recv_timeout(remaining) {
            Ok(ServiceEvent::ServiceResolved(info)) => {
                let addr = info
                    .get_addresses_v4()
                    .into_iter()
                    .next()
                    .map(|a| a.to_string())
                    .unwrap_or_else(|| info.get_hostname().trim_end_matches('.').to_owned());
                found.push(addr);
            }
            Ok(_) => {}
            Err(_) => break,
        }
    }

    let _ = daemon.stop_browse("_cognitum._tcp.local.");

    match found.len() {
        0 => Err(LearnError::Acquire(
            "no Cognitum Seed found on the network — use `--seed <address>` to specify one manually.".into(),
        )),
        1 => Ok(found.remove(0)),
        _ => {
            if let Some(idx) = seed_index {
                if idx == 0 || idx > found.len() {
                    return Err(LearnError::Acquire(format!(
                        "--seed-index {idx} out of range (found {} Seeds) — use `--seed <address>` to specify one manually.",
                        found.len()
                    )));
                }
                return Ok(found.remove(idx - 1));
            }
            eprintln!("Multiple Cognitum Seeds found:");
            for (i, addr) in found.iter().enumerate() {
                eprintln!("  {}: {addr}", i + 1);
            }
            eprintln!("Tip: re-run with `--seed-index N` to skip this prompt.");
            eprint!("Enter number: ");
            let mut line = String::new();
            std::io::stdin()
                .read_line(&mut line)
                .map_err(LearnError::Io)?;
            let choice: usize = line.trim().parse().unwrap_or(0);
            if choice == 0 || choice > found.len() {
                Err(LearnError::Acquire(
                    "invalid selection — use `--seed <address>` to specify a device manually."
                        .into(),
                ))
            } else {
                Ok(found.remove(choice - 1))
            }
        }
    }
}

/// Push a topic's vectors to a Cognitum One Seed over LAN.
///
/// `token` is the bearer returned by the Seed's pairing flow
/// (`POST /api/v1/pair`).  USB-gadget-link clients on `169.254.x.x` may
/// be auto-trusted without a token; LAN clients require one.
pub async fn run_push(
    topic: String,
    seed: Option<String>,
    seed_index: Option<usize>,
    token: Option<String>,
    kb_root: Utf8PathBuf,
) -> learn_core::Result<()> {
    let address = resolve_seed_address(seed, seed_index).await?;

    let rvf_path = rvf_path_for_topic(&topic, &kb_root);
    if !rvf_path.exists() {
        return Err(LearnError::Acquire(format!(
            "topic '{topic}' not found at {rvf_path}\n  \
             Run `learn ingest <source> --topic {topic}` to build it first."
        )));
    }

    let topic_obj = Topic::new(&topic)
        .map_err(|e| LearnError::Acquire(format!("invalid topic slug '{topic}': {e}")))?;
    let index = LearnIndex::open_read(kb_root.as_path(), topic_obj)
        .map_err(|e| LearnError::Acquire(format!("failed to open KB '{topic}': {e}")))?;

    let vectors: Vec<(u64, &[f32])> = index.all_embeddings().collect();
    let total = vectors.len();
    if total == 0 {
        return Err(LearnError::Acquire(format!(
            "topic '{topic}' has no embeddings — nothing to push."
        )));
    }
    let dim = vectors[0].1.len();

    println!("pushing {total} vectors ({dim}-dim) to {address}…");

    let client = reqwest::Client::new();
    let ingest_url = format!("http://{address}/api/v1/store/ingest");

    // ~14 chars per f32 + tuple overhead; pick a batch size that stays under the cap.
    let bytes_per_vec = dim * 14 + 32;
    let batch_size = TARGET_BATCH_BYTES.saturating_div(bytes_per_vec).max(1);

    let mut sent = 0usize;
    let mut batch_no = 0usize;
    for chunk in vectors.chunks(batch_size) {
        batch_no += 1;
        let body = IngestBody {
            vectors: chunk.to_vec(),
        };
        let body_json = serde_json::to_string(&body)
            .map_err(|e| LearnError::Acquire(format!("JSON encode failed: {e}")))?;

        let mut req = client
            .post(&ingest_url)
            .header("Content-Type", "application/json")
            .body(body_json);
        if let Some(t) = token.as_deref() {
            req = req.header("Authorization", format!("Bearer {t}"));
        }

        let response = req.send().await.map_err(|e| {
            let hint = if e.is_connect() {
                " — Seed not reachable (check it's powered on and on the same network or USB link)"
            } else if e.is_timeout() {
                " — connection timed out"
            } else {
                ""
            };
            LearnError::Acquire(format!("HTTP POST to {ingest_url} failed: {e}{hint}"))
        })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            let hint = if status.as_u16() == 401 {
                "\n  hint: pair this client first via `POST /api/v1/pair/window` then `POST /api/v1/pair`, \
                 and pass the returned token with `--token <TOKEN>` (or set `LEARN_SEED_TOKEN`)."
            } else if status.as_u16() == 409 {
                "\n  hint: Seed store dimension does not match the KB's embedding dimension. \
                 Reset the Seed store or restart the agent with `--dimension <N>`."
            } else {
                ""
            };
            return Err(LearnError::Acquire(format!(
                "Seed rejected batch {batch_no} (HTTP {status}): {body}{hint}"
            )));
        }
        sent += chunk.len();
        println!(
            "  batch {batch_no}: ingested {} vectors ({sent}/{total})",
            chunk.len()
        );
    }

    println!("✓ pushed {sent} vectors ({dim}-dim) — verifying store status…");

    let status_url = format!("http://{address}/api/v1/status");
    let status_response = client
        .get(&status_url)
        .send()
        .await
        .map_err(|e| LearnError::Acquire(format!("HTTP GET {status_url} failed: {e}")))?;
    let status_body = status_response
        .text()
        .await
        .map_err(|e| LearnError::Acquire(format!("failed to read status response: {e}")))?;
    println!("{status_body}");
    Ok(())
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn resolve_seed_address_uses_provided_address() {
        let result = resolve_seed_address(Some("192.168.1.42".to_string()), None).await;
        assert_eq!(result.unwrap(), "192.168.1.42");
    }

    #[tokio::test]
    async fn resolve_seed_address_uses_mdns_hostname() {
        let result = resolve_seed_address(Some("cognitum.local".to_string()), None).await;
        assert_eq!(result.unwrap(), "cognitum.local");
    }

    #[test]
    #[cfg(unix)]
    fn rvf_path_for_topic_constructs_correctly() {
        let kb_root = Utf8PathBuf::from("/home/user/Docs/KB");
        let path = rvf_path_for_topic("french-cooking", &kb_root);
        assert_eq!(
            path,
            Utf8PathBuf::from("/home/user/Docs/KB/french-cooking.rvf")
        );
    }

    #[test]
    #[cfg(unix)]
    fn rvf_path_for_topic_nested_root() {
        let kb_root = Utf8PathBuf::from("/tmp/test-kb");
        let path = rvf_path_for_topic("rust-programming", &kb_root);
        assert_eq!(path.as_str(), "/tmp/test-kb/rust-programming.rvf");
    }

    #[test]
    fn rvf_path_for_topic_joins_correctly() {
        let dir = tempfile::tempdir().unwrap();
        let kb_root = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        let path = rvf_path_for_topic("my-topic", &kb_root);
        assert!(path.as_str().ends_with("my-topic.rvf"));
        assert!(path.starts_with(&kb_root));
    }

    #[tokio::test]
    async fn run_push_errors_clearly_when_rvf_missing() {
        let dir = tempfile::tempdir().unwrap();
        let kb_root = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        let result = run_push(
            "nonexistent-topic".to_string(),
            Some("127.0.0.1".to_string()),
            None,
            None,
            kb_root,
        )
        .await;
        assert!(
            matches!(result, Err(learn_core::LearnError::Acquire(_))),
            "expected Err(LearnError::Acquire) for missing .rvf, got: {result:?}"
        );
        if let Err(learn_core::LearnError::Acquire(msg)) = result {
            assert!(
                msg.contains("nonexistent-topic"),
                "error should name the topic; got: {msg}"
            );
            assert!(
                msg.contains("learn ingest"),
                "error should suggest learn ingest; got: {msg}"
            );
        }
    }

    #[test]
    fn ingest_body_serializes_to_seed_contract() {
        // Verify the wire format matches the Cognitum Seed's published shape:
        //   {"vectors":[[id_u64,[f32,f32,...]],...]}
        let v: Vec<f32> = vec![0.1, 0.2, 0.3];
        let body = IngestBody {
            vectors: vec![(9999u64, v.as_slice())],
        };
        let json = serde_json::to_string(&body).unwrap();
        assert!(
            json.starts_with("{\"vectors\":[[9999,[0.1,"),
            "ingest body does not match seed contract: {json}"
        );
    }
}
