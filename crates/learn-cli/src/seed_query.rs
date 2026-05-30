//! `seed_query` — route `learn ask` retrieval through a Cognitum One Seed.
//!
//! When the `--on-seed` flag is set (or auto-selected because a Seed is
//! configured and reachable), the question is embedded locally with the
//! same BGE-small-en-v1.5 (384-dim) embedder used for ingestion, then the
//! vector is POSTed to the Seed's `POST /api/v1/store/query` endpoint.
//!
//! The Seed returns top-k `(id, distance)` pairs with empty metadata —
//! it stores ONLY ids and vectors. We translate those u64 ids back into
//! full `Chunk` records using the *local* sidecar (`<topic>.meta.json`)
//! that was written during ingestion, then return `Vec<Hit>` shaped
//! exactly like local retrieval so the synthesizer is unchanged.
//!
//! Wire format (verified live 2026-05-26 against agent-cognitum v0.x):
//!
//! ```text
//! POST https://{address}:8443/api/v1/store/query
//! Authorization: Bearer <token>
//! Content-Type: application/json
//! Body: {"vector": [f32; 384], "top_k": <usize>}
//! ```
//!
//! Response (verified live):
//!
//! ```json
//! {
//!   "filtered": false,
//!   "k": 10,
//!   "metric": "cosine",
//!   "results": [
//!     {"id": 17371099242078173990, "distance": 0.9908, "metadata": []},
//!     ...
//!   ],
//!   "total_searched": 108
//! }
//! ```
//!
//! TLS uses `danger_accept_invalid_certs = true` because the Seed presents
//! a self-signed cert on the LAN. This is intentional and matches the
//! pairing-bootstrap trust model.

#![deny(unsafe_code)]

use learn_core::{Hit, LearnError, Result};
use learn_index::LearnIndex;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// HTTPS port that the Seed's API listens on.
const SEED_HTTPS_PORT: u16 = 8443;

/// How long to wait for a single Seed query before giving up.
const QUERY_TIMEOUT_SECS: u64 = 10;

#[derive(Serialize)]
struct QueryBody<'a> {
    vector: &'a [f32],
    top_k: usize,
}

#[derive(Deserialize, Debug)]
struct QueryResponse {
    #[serde(default)]
    results: Vec<SeedResult>,
}

#[derive(Deserialize, Debug)]
struct SeedResult {
    id: u64,
    distance: f32,
}

/// Build the query URL, accepting either a bare host (`10.0.0.72`) or a host
/// with explicit scheme/port (`https://10.0.0.72:8443`).
pub(crate) fn build_query_url(address: &str) -> String {
    let trimmed = address.trim().trim_end_matches('/');
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        format!("{trimmed}/api/v1/store/query")
    } else {
        format!("https://{trimmed}:{SEED_HTTPS_PORT}/api/v1/store/query")
    }
}

/// POST `query_vec` to the Seed and translate returned u64 ids into local
/// `Hit`s using `index` as the chunk-text lookup.
///
/// Hits whose ids are not present in the local sidecar are skipped — this
/// matters when the Seed holds vectors from multiple Mac KBs but we only
/// have one topic's sidecar loaded. Hits are returned in the Seed's
/// distance order (already sorted ascending = best first), with rank
/// re-numbered from 0.
pub async fn query_seed(
    address: &str,
    token: Option<&str>,
    query_vec: &[f32],
    top_k: usize,
    index: &LearnIndex,
) -> Result<Vec<Hit>> {
    let url = build_query_url(address);

    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .timeout(Duration::from_secs(QUERY_TIMEOUT_SECS))
        .build()
        .map_err(|e| LearnError::Retrieve(format!("HTTP client build failed: {e}")))?;

    let body = QueryBody {
        vector: query_vec,
        top_k,
    };

    let mut req = client.post(&url).json(&body);
    if let Some(t) = token {
        req = req.header("Authorization", format!("Bearer {t}"));
    }

    let response = req.send().await.map_err(|e| {
        let hint = if e.is_connect() {
            " — Seed not reachable (check it's powered on and on the same network)"
        } else if e.is_timeout() {
            " — Seed query timed out"
        } else {
            ""
        };
        LearnError::Retrieve(format!("HTTP POST to {url} failed: {e}{hint}"))
    })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        let hint = match status.as_u16() {
            401 => "\n  hint: Seed rejected the bearer token. Pair this client and \
                    store the token with `learn config set seed.token <BEARER>`."
                .to_owned(),
            400 if body.contains("dim mismatch") => {
                "\n  Your Seed is currently in sensor mode (a different vector dimension), \
                 so it can't answer KB queries yet.\n  \
                 Your knowledge bases stay on your Mac and answer locally — \
                 everything works.\n  \
                 To host KBs on the Seed, see docs/seed-setup.md."
                    .to_owned()
            }
            _ => String::new(),
        };
        return Err(LearnError::Retrieve(format!(
            "Seed rejected query (HTTP {status}): {body}{hint}"
        )));
    }

    let parsed: QueryResponse = response
        .json()
        .await
        .map_err(|e| LearnError::Retrieve(format!("failed to parse Seed query response: {e}")))?;

    Ok(hits_from_results(parsed.results, index))
}

/// Translate the Seed's `(id, distance)` pairs into local `Hit`s.
///
/// Pulled out for unit testing. Skips ids that don't resolve to a chunk
/// in the local sidecar — they are either from a different topic's KB
/// also pushed to the same Seed store, or were ingested before this
/// machine had the sidecar.
fn hits_from_results(results: Vec<SeedResult>, index: &LearnIndex) -> Vec<Hit> {
    let mut hits: Vec<Hit> = Vec::with_capacity(results.len());
    let mut rank = 0usize;
    for sr in results {
        if let Some(chunk) = index.chunk_by_u64(sr.id) {
            hits.push(Hit {
                chunk,
                score: (1.0 - sr.distance).max(0.0),
                rank,
            });
            rank += 1;
        }
    }
    hits
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_query_url_bare_host_uses_https_8443() {
        assert_eq!(
            build_query_url("10.0.0.72"),
            "https://10.0.0.72:8443/api/v1/store/query"
        );
    }

    #[test]
    fn build_query_url_preserves_explicit_scheme() {
        assert_eq!(
            build_query_url("http://seed.local:9000"),
            "http://seed.local:9000/api/v1/store/query"
        );
    }

    #[test]
    fn build_query_url_strips_trailing_slash() {
        assert_eq!(
            build_query_url("https://10.0.0.72:8443/"),
            "https://10.0.0.72:8443/api/v1/store/query"
        );
    }

    #[test]
    fn build_query_url_handles_whitespace() {
        assert_eq!(
            build_query_url("  10.0.0.72  "),
            "https://10.0.0.72:8443/api/v1/store/query"
        );
    }
}
