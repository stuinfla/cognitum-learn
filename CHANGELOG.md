# Changelog

All notable user-facing changes to `cognitum-learn` are recorded here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and
the project adheres to [Semantic Versioning](https://semver.org/).

## [0.5.8] — 2026-05-26

### Added

- **Per-call synthesis overrides via env vars.** `AnthropicSynthesizer` now
  reads two new optional env vars in addition to the existing
  `LEARN_ANTHROPIC_MODEL`:
  - `LEARN_ANTHROPIC_MAX_TOKENS` — caps the Anthropic `max_tokens` request
    field (default `4096`, unchanged when unset).
  - `LEARN_ANTHROPIC_SYSTEM_PROMPT` — replaces the built-in system prompt
    for this call only (default behaviour preserved when unset).

  Together these let voice surfaces with hard timeouts (e.g. Alexa's ~8 s
  cap) trade depth for latency by spawning `learn ask` with
  `LEARN_ANTHROPIC_MODEL=claude-haiku-4-5-…`,
  `LEARN_ANTHROPIC_MAX_TOKENS=180`, and a spoken-natural system prompt —
  without affecting the Mac CLI or Apple Siri paths, which leave the env
  unset and continue to use Opus/Sonnet with the full prompt.
  (`crates/learn-synth/src/lib.rs`)

## [0.5.7] — 2026-05-26

### Added

- **`learn ask` can now route retrieval through a Cognitum One Seed.**
  When `seed.address` is configured the new default behaviour POSTs the
  query vector to the Seed's `POST /api/v1/store/query` endpoint (HTTPS
  on port 8443, self-signed cert accepted) and translates the returned
  ids back into local chunks. Two new flags:
  - `--on-seed` — force-route through the Seed; fail fast if it is
    unconfigured or unreachable. Use this when you want certainty that
    the answer came from the shared device, not the Mac.
  - `--no-seed` — force the local HNSW+BM25 path even when a Seed is
    configured. Preserves the v0.5.6 behaviour exactly.

  Default mode picks the Seed when `seed.address` is set and falls back
  to local retrieval automatically if the Seed errors out. Output
  format, exit codes, and the synthesizer surface are unchanged, so
  existing MCP and CLI consumers see no contract change.

  Bearer-token resolution order: `LEARN_SEED_TOKEN` env var →
  `seed.token` in `config.json` → `~/.cognitum-seed.token` file.
  Measured Seed-side query latency on a Pi Zero Seed at 384-dim with
  ~200 vectors: ~60 ms. (`crates/learn-cli/src/seed_query.rs`,
  `crates/learn-cli/src/main.rs`, `crates/learn-index/src/lib.rs`)

- **New `LearnIndex::chunk_by_u64(id)`** for callers that already have
  the FNV-1a u64 id in hand (e.g. ids returned by an external vector
  store). Avoids round-tripping back through the chunk_id string.
  (`crates/learn-index/src/lib.rs`)

- **`scripts/smoke-test-seed-ask.sh`** — end-to-end probe that exits 0
  when a Seed-backed `learn ask` returns a cited answer. Verifies
  the binary is installed, the API key is set, the Seed is reachable,
  and the store has vectors before paying for the embedder + Anthropic
  call.

## [0.5.6] — 2026-05-26

### Fixed

- **`learn push` now reads `seed.token` from `config.json`.**
  Previously the CLI ignored the stored bearer and returned HTTP 401 unless
  `--token <X>` or `LEARN_SEED_TOKEN` were supplied on every invocation.
  Resolution order is now: `--token` flag → `LEARN_SEED_TOKEN` env var →
  `seed.token` in `config.json`. Both interactive `learn push` and the
  auto-push path triggered by `learn ingest` benefit. Store the token once
  with `learn config set seed.token <BEARER>` and forget about it.
  (`crates/learn-cli/src/config.rs`, `crates/learn-cli/src/main.rs`,
  `crates/learn-cli/src/commands.rs`)

- **mDNS discovery dedups duplicate Seed advertisements by IP.**
  A single Cognitum Seed announcing on multiple interfaces (common when the
  USB-gadget link and an mDNS responder both publish) no longer triggers the
  "Multiple Cognitum Seeds found: 1: 169.254.42.1, 2: 169.254.42.1" prompt.
  When mixed addresses remain after dedup, LAN IPs out-rank the
  USB-gadget link-local range (`169.254.0.0/16`), so the routable address
  is selected automatically. (`crates/learn-cli/src/push.rs`)

- **Clearer error when the Seed store dimension does not match the KB.**
  The old hint said "Reset the Seed store or restart the agent with
  `--dimension <N>`" with no migration guidance. The new message extracts
  the `expected N, got M` numbers from the Seed's response, names them
  explicitly, and links to the dimension-migration guide:
  `https://github.com/stuinfla/cognitum-learn/wiki/seed-dimension-migration`.
  Auto-detection now triggers on the response body containing
  "dimension mismatch" — not just on HTTP 409 — which matches the actual
  symptom seen on a fresh Seed locked at 8-dim sensor data.
  (`crates/learn-cli/src/push.rs`)

### Added

- New config key `seed.token` (persisted in `config.json`). View status with
  `learn config get seed.token` (the actual bearer is never printed;
  `(set)` / `(not set)` only).
- `learn config list` and `learn config set` now accept `seed.token`.

## [0.5.5] — 2026-05-23

- Pin RuVector deps, unify embedder name, modernize README.

## [0.5.4] — 2026-05-22

- Convert RuVector path-deps to git-deps so `cargo install --git` works
  end-to-end.

## [0.5.3] and earlier

See `git log` for history prior to the CHANGELOG.
