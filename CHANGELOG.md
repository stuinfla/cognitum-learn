# Changelog

All notable user-facing changes to `cognitum-learn` are recorded here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and
the project adheres to [Semantic Versioning](https://semver.org/).

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
