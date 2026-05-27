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

/// Return `true` for the IPv4 link-local range 169.254.0.0/16 (RFC 3927,
/// used by the Cognitum Seed's USB-gadget link).
fn is_link_local_v4(addr: &str) -> bool {
    addr.starts_with("169.254.")
}

/// Collapse multiple mDNS records that resolve to the same IP into one entry,
/// preserving the original discovery order. Bug fix in v0.5.6 — the prior
/// implementation prompted the user when the SAME Seed announced itself twice
/// (a single USB-link Seed often advertises on multiple interfaces).
///
/// When mixed addresses remain, prefer LAN over the 169.254/16 USB-gadget
/// link-local range so `learn push` (no flags) picks the routable address.
///
/// Exposed for unit testing.
pub(crate) fn dedup_and_rank(addrs: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut unique: Vec<String> = Vec::with_capacity(addrs.len());
    for a in addrs {
        if seen.insert(a.clone()) {
            unique.push(a);
        }
    }
    // Stable sort: LAN addresses (false) sort before link-local (true).
    unique.sort_by_key(|a| is_link_local_v4(a));
    unique
}

/// Parse a "dimension mismatch ... expected N, got M" snippet out of the Seed
/// response body. Returns `(expected, got)` when both numbers are recoverable.
///
/// Exposed for unit testing.
pub(crate) fn parse_dim_mismatch(body: &str) -> Option<(usize, usize)> {
    // Pattern: "expected N, got M" (Seed v0.x format).
    let lower = body.to_ascii_lowercase();
    let exp_idx = lower.find("expected ")?;
    let after_exp = &body[exp_idx + "expected ".len()..];
    let exp_end = after_exp
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(after_exp.len());
    let expected: usize = after_exp[..exp_end].parse().ok()?;

    let got_idx = lower.find("got ")?;
    let after_got = &body[got_idx + "got ".len()..];
    let got_end = after_got
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(after_got.len());
    let got: usize = after_got[..got_end].parse().ok()?;
    Some((expected, got))
}

/// Build a user-facing hint to append to a non-2xx Seed response.
///
/// Bug 3 fix in v0.5.6: when the Seed body contains "dimension mismatch"
/// (the real symptom seen with a fresh Seed locked at 8-dim sensor data),
/// point the user at the wipe-and-recreate migration guide instead of the
/// vague "Reset the Seed store" hint.
///
/// Exposed for unit testing.
pub(crate) fn build_error_hint(status: u16, body: &str, kb_dim: usize) -> String {
    if body.to_ascii_lowercase().contains("dimension mismatch") {
        if let Some((expected, got)) = parse_dim_mismatch(body) {
            return format!(
                "\n  hint: Seed store is locked at dim {expected} (likely sensor data) — \
                 your KB is {got}-dim.\n  \
                 To migrate, wipe the Seed store and let it re-initialise at the new dim. \
                 See:\n    \
                 https://github.com/stuinfla/cognitum-learn/wiki/seed-dimension-migration"
            );
        }
        return format!(
            "\n  hint: Seed store dimension does not match the KB's embedding dimension \
             ({kb_dim}-dim).\n  \
             To migrate, wipe the Seed store and let it re-initialise at the new dim. \
             See:\n    \
             https://github.com/stuinfla/cognitum-learn/wiki/seed-dimension-migration"
        );
    }
    match status {
        401 => "\n  hint: pair this client first via `POST /api/v1/pair/window` then \
                `POST /api/v1/pair`, and pass the returned token with `--token <TOKEN>` \
                (or set `LEARN_SEED_TOKEN`, or store it with \
                `learn config set seed.token <TOKEN>`)."
            .to_owned(),
        409 => format!(
            "\n  hint: Seed store dimension does not match the KB's embedding dimension \
             ({kb_dim}-dim).\n  \
             To migrate, wipe the Seed store and let it re-initialise at the new dim. \
             See:\n    \
             https://github.com/stuinfla/cognitum-learn/wiki/seed-dimension-migration"
        ),
        _ => String::new(),
    }
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
    let mut raw: Vec<String> = Vec::new();

    loop {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        if remaining.is_zero() {
            break;
        }
        match receiver.recv_timeout(remaining) {
            Ok(ServiceEvent::ServiceResolved(info)) => {
                // Push ALL v4 addresses the Seed advertised (it may have
                // multiple interfaces), not just the first one. The dedup step
                // below collapses duplicates by IP, and a LAN address — if
                // present — will out-rank the 169.254/16 USB link-local one.
                let mut had_v4 = false;
                for ip in info.get_addresses_v4() {
                    raw.push(ip.to_string());
                    had_v4 = true;
                }
                if !had_v4 {
                    raw.push(info.get_hostname().trim_end_matches('.').to_owned());
                }
            }
            Ok(_) => {}
            Err(_) => break,
        }
    }

    let _ = daemon.stop_browse("_cognitum._tcp.local.");

    let mut found = dedup_and_rank(raw);

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
            let hint = build_error_hint(status.as_u16(), &body, dim);
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
    fn dedup_and_rank_collapses_duplicate_ips() {
        // Bug 2 reproducer: a single Seed announcing twice on the same IP.
        let raw = vec!["169.254.42.1".to_owned(), "169.254.42.1".to_owned()];
        assert_eq!(dedup_and_rank(raw), vec!["169.254.42.1".to_owned()]);
    }

    #[test]
    fn dedup_and_rank_prefers_lan_over_link_local() {
        let raw = vec!["169.254.42.1".to_owned(), "10.0.0.72".to_owned()];
        let out = dedup_and_rank(raw);
        assert_eq!(out, vec!["10.0.0.72".to_owned(), "169.254.42.1".to_owned()]);
    }

    #[test]
    fn dedup_and_rank_keeps_distinct_lan_addresses() {
        let raw = vec!["10.0.0.72".to_owned(), "192.168.1.5".to_owned()];
        let out = dedup_and_rank(raw);
        assert_eq!(out.len(), 2);
        assert!(out.contains(&"10.0.0.72".to_owned()));
        assert!(out.contains(&"192.168.1.5".to_owned()));
    }

    #[test]
    fn dedup_and_rank_empty_input_returns_empty() {
        assert!(dedup_and_rank(vec![]).is_empty());
    }

    #[test]
    fn parse_dim_mismatch_extracts_expected_and_got() {
        let body = "proof verification failed: dimension mismatch for vector 0: \
                    expected 8, got 384";
        assert_eq!(parse_dim_mismatch(body), Some((8, 384)));
    }

    #[test]
    fn parse_dim_mismatch_returns_none_on_unrelated_body() {
        assert_eq!(parse_dim_mismatch("internal server error"), None);
    }

    #[test]
    fn build_error_hint_dim_mismatch_points_at_wiki() {
        let body = "dimension mismatch for vector 0: expected 8, got 384";
        let hint = build_error_hint(500, body, 384);
        assert!(
            hint.contains("seed-dimension-migration"),
            "hint should point at the wiki page; got: {hint}"
        );
        assert!(
            hint.contains("dim 8"),
            "hint should name the expected dim; got: {hint}"
        );
        assert!(
            hint.contains("384"),
            "hint should name the KB dim; got: {hint}"
        );
    }

    #[test]
    fn build_error_hint_dim_mismatch_works_without_parseable_numbers() {
        let hint = build_error_hint(500, "got a dimension mismatch somewhere", 384);
        assert!(hint.contains("seed-dimension-migration"));
        assert!(hint.contains("384-dim"));
    }

    #[test]
    fn build_error_hint_401_mentions_config_token() {
        let hint = build_error_hint(401, "Bearer token required", 384);
        assert!(hint.contains("learn config set seed.token"));
    }

    #[test]
    fn build_error_hint_unrelated_status_returns_empty() {
        assert_eq!(build_error_hint(503, "service unavailable", 384), "");
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
