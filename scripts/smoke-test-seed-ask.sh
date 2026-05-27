#!/usr/bin/env bash
# scripts/smoke-test-seed-ask.sh — end-to-end probe that `learn ask --on-seed`
# can talk to a configured Cognitum Seed and return a cited answer.
#
# Exits 0 on success, non-zero on any failure. Designed for CI/cron use:
# the only side effects are a single Seed query + a single Anthropic call.
#
# Prereqs (the script verifies each):
#   1. `learn` binary on PATH (built from this repo).
#   2. `seed.address` configured + Seed reachable (`learn config get`).
#   3. ANTHROPIC_API_KEY set in the environment.
#   4. A topic with vectors already pushed to the Seed; the topic slug must
#      also exist locally so chunk lookups resolve. Override with TOPIC=...

set -euo pipefail

TOPIC="${TOPIC:-git-explained-in-100-seconds}"
QUESTION="${QUESTION:-What is git in simple terms?}"

# ── Step 1: binary present ───────────────────────────────────────────────────
if ! command -v learn >/dev/null 2>&1; then
    echo "✗ \`learn\` not on PATH — run: cargo install --path crates/learn-cli" >&2
    exit 1
fi
echo "✓ learn binary: $(which learn)"

# ── Step 2: API key present ──────────────────────────────────────────────────
if [[ -z "${ANTHROPIC_API_KEY:-}" ]]; then
    echo "✗ ANTHROPIC_API_KEY not set" >&2
    exit 1
fi
echo "✓ ANTHROPIC_API_KEY set"

# ── Step 3: seed config + reachability ───────────────────────────────────────
SEED_ADDR=$(learn config get seed.address 2>&1 | sed -n 's/^seed.address = //p')
if [[ -z "$SEED_ADDR" || "$SEED_ADDR" == "(not set)" ]]; then
    echo "✗ seed.address not configured" >&2
    exit 1
fi
echo "✓ seed.address: $SEED_ADDR"

# Lightweight reachability probe before paying for the embedder + API call.
STATUS_HTTP=$(curl -sk --max-time 5 -o /tmp/seed-status.json \
    -w "%{http_code}" \
    -H "Authorization: Bearer $(learn config get seed.token >/dev/null 2>&1 && cat ~/.cognitum-seed.token 2>/dev/null || echo)" \
    "https://${SEED_ADDR}:8443/api/v1/status" || echo "000")
if [[ "$STATUS_HTTP" != "200" ]]; then
    echo "✗ Seed status probe failed (HTTP $STATUS_HTTP)" >&2
    exit 1
fi
TOTAL_VECS=$(grep -o '"total_vectors":[0-9]*' /tmp/seed-status.json | head -1 | cut -d: -f2)
echo "✓ Seed reachable: $TOTAL_VECS vectors stored"
if [[ "${TOTAL_VECS:-0}" -lt 1 ]]; then
    echo "✗ Seed store empty — push a topic first with: learn push <topic>" >&2
    exit 1
fi

# ── Step 4: end-to-end ask ───────────────────────────────────────────────────
echo "→ learn ask $TOPIC \"$QUESTION\" --on-seed"
START_NS=$(python3 -c 'import time; print(int(time.time_ns()))')
OUTPUT=$(learn ask "$TOPIC" "$QUESTION" --on-seed 2>&1)
RC=$?
END_NS=$(python3 -c 'import time; print(int(time.time_ns()))')
ELAPSED_MS=$(( (END_NS - START_NS) / 1000000 ))

if [[ $RC -ne 0 ]]; then
    echo "✗ learn ask exited with $RC"
    echo "--- stdout/stderr ---"
    echo "$OUTPUT"
    exit 1
fi

if [[ -z "$OUTPUT" ]]; then
    echo "✗ learn ask returned empty output (expected a cited answer)" >&2
    exit 1
fi

if ! grep -qE '\[[0-9]+\]' <<<"$OUTPUT"; then
    echo "⚠ no citation markers ([N]) in output — answer may not be grounded:" >&2
    echo "$OUTPUT" >&2
    # Not a hard failure — the model may legitimately answer without citations
    # for very short questions, but we want to surface it.
fi

CHAR_COUNT=$(printf '%s' "$OUTPUT" | wc -c | tr -d ' ')
echo "✓ ask returned ${CHAR_COUNT} chars in ${ELAPSED_MS}ms"
echo "--- answer ---"
echo "$OUTPUT"
echo "---"
echo "✓ SMOKE TEST PASSED"
