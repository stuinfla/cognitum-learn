---
name: cognitum-learn
description: Build per-topic knowledge bases from YouTube videos and query them with cited Anthropic answers. Pure-Rust pipeline (BGE-large embeddings, hybrid HNSW + BM25 retrieval, MMR diversity, witness-chained RVF storage, SONA per-topic self-learning, AIMDS guardrails). Invoke when the user wants to ingest video content, query a video knowledge base, or run autonomous curriculum discovery.
---

# Cognitum Learn — Video Knowledge Base CLI

A high-performance Rust CLI that turns YouTube videos into queryable knowledge bases stored in RuVector's RVF binary format. Each KB is one `.rvf` file per topic.

## When to Use This Skill

Invoke `learn` when the user:
- Shares a YouTube link or channel handle and wants to learn from it
- Asks a question about video content they've ingested before
- Wants to build a curriculum on a topic ("teach me X")
- Says things like "watch this video and remember it", "what did the speaker in <topic> say about Y", "build a KB on Z"
- Wants to apply lessons from a topic to a real-world task ("use what we learned in <topic> to draft Y")

DO NOT invoke for general web research or queries that don't involve ingesting and querying a KB.

## Binary

Binary name: `learn`.

Lookup order:
1. `which learn` — already on PATH (normal after `cargo install` or the release `install.sh`)
2. `~/.claude/skills/cognitum-learn/bin/learn` — symlinked by `install.sh` from a release tarball
3. Neither found → install from the public release: https://github.com/stuinfla/cognitum-learn/releases

Run `learn --help` to confirm install before first use.

## Key Subcommands (22 total — see `learn --help`)

| Command | Purpose | Common pattern |
|---|---|---|
| `learn ingest <source> --topic <slug>` | Add a single source to a KB | `learn ingest https://youtu.be/XYZ --topic my-topic` |
| `learn import <dir> --topic <slug>` | Bulk-ingest a directory of files (pdf/mp4/mp3/txt/md) | `learn import ~/Downloads/lectures/ --topic ml-course` |
| `learn ask <topic> "<question>"` | Cited answer | `learn ask my-topic "What does the speaker say about X?"` |
| `learn apply <topic> "<task>"` | Apply lessons to a task | `learn apply french-cooking "draft a 3-course menu"` |
| `learn study <topic> --depth quick\|medium\|deep` | Autonomous curriculum | `learn study french-cooking --depth medium` |
| `learn chat <topic>` | Multi-turn grounded conversation | — |
| `learn quiz <topic>` | Flashcard Q&A with spaced repetition | — |
| `learn eval <topic>` | Run golden Q&A regression suite | — |
| `learn list` | All known topics | — |
| `learn status <topic>` | KB stats + coherence KPI | — |
| `learn watch <topic> --cadence weekly\|hourly\|daily\|monthly` | Schedule recurring updates | — |
| `learn forget <topic>` | Delete a KB | — |
| `learn compact <topic>` | Garbage-collect a KB | — |
| `learn who-said <topic> "<exact quote>"` | Find speaker + timestamp | — |
| `learn timeline <topic>` | Chronological view | — |
| `learn compare <topic-a> <topic-b> "<theme>"` | Cross-topic compare | — |
| `learn summarize <topic>` | One-paragraph corpus summary | — |
| `learn serve <topic>` | Start MCP server for Claude Code | — |
| `learn push <topic> --seed <ip>` | Push .rvf to Cognitum Seed | `learn push french-cooking --seed 192.168.1.42` |
| `learn cloud <topic>` | SVG word cloud | — |
| `learn map` | 2-D galaxy map of all KBs | — |
| `learn doctor` | Environment diagnostic | — |

## Source shapes accepted by `ingest`

- Single video: `https://youtu.be/<id>` or `https://www.youtube.com/watch?v=<id>`
- Channel: `https://www.youtube.com/@handle` or just `@handle`
- Playlist: `https://www.youtube.com/playlist?list=<id>`
- Search-as-source: `ytsearch10:french cooking technique` (top-N hits)
- Local file: any local `.mp4` / `.mkv` / `.webm` / `.vtt`

Use `--limit N` with channels/playlists/search to bound ingest. `--since YYYY-MM-DD` is accepted but emits a warning (not yet implemented). Frame captioning is **on by default**; use `--no-frames` to disable or `--with-frames` to be explicit (no-op; conflicts with `--no-frames`).

## Output conventions

- KBs live at `~/Docs/KB/<topic>.rvf` (binary HNSW) + `<topic>.meta.json` (sidecar) + `<topic>.witness.json` (Blake3 audit chain) + `<topic>.emb.bin` (embeddings).
- All `learn ask` / `apply` answers include numbered citations `[1][2][3]` linking to YouTube `youtu.be/<id>?t=<start_seconds>`.
- The `coherence:` line in `learn status` reports `integrated=X.XX workspace=X.XX [Disjoint|Loose|Coherent|HighlyIntegrated]`.

## Environment + safety

- `ANTHROPIC_API_KEY` required for `learn ask` / `apply` / `study`. The skill should NOT hard-fail if missing — surface the requirement and offer the local sovereignty path: `LEARN_SYNTH_LOCAL=1 learn ask <topic> "..."` (uses ruvllm instead of Anthropic).
- AIMDS guardrails scan inputs and outputs. Set `LEARN_AIMDS_REQUIRED=1` for hard-fail mode.
- SONA per-topic adapters at `~/.cache/learn-rs/adapters/<topic>/lora.json` accumulate via `record_feedback` and sharpen retrieval over time. Loaded automatically by `Retriever::for_topic`.

## Common patterns

**User shares a YouTube link cold:**
1. Extract a topic slug from the page title (e.g. "french-cooking", "claude-skills"). Confirm the slug with the user only if ambiguous.
2. `learn ingest <url> --topic <slug>`
3. Report: chunks ingested, KB size, sample question they could ask.

**User asks a question about content they previously ingested:**
1. `learn list` to confirm the topic exists.
2. `learn ask <topic> "<question>"` — return the cited answer verbatim.

**User says "teach me X":**
1. Default to `--depth medium`. Confirm before running deep (long-running, costs API credits).
2. `learn study <topic-slug> --depth medium` — this harvests candidates, scores them, and ingests the top picks.

**User wants to build a recurring KB:**
1. `learn watch <topic> --cadence weekly` outputs a LaunchAgent plist and bootstrap instructions. Show the instructions to the user; do NOT bootstrap without explicit approval.

## Repo + project state

- Public repo: https://github.com/stuinfla/cognitum-learn
- Workspace at: `/Users/stuartkerr/Code/Video watcher skill/learn-rs/`
- Path-deps to: `~/RuVector_Clean` via sibling symlink `../ruvector`
- Phase tracking: `docs/adr/ADR-001-elite-roadmap.md`
- Bounded contexts: `docs/ddd/DDD-001-bounded-contexts.md`
- 17 workspace crates, 311+ tests, witness chain wired

## When NOT to invoke

- User wants generic web research → use WebSearch / WebFetch instead
- User wants to chat ABOUT content without persisting it → just summarise inline
- User wants to query a source they haven't ingested yet → ingest it first, then query

## Don't make up answers

If a topic doesn't exist (`learn list` doesn't show it), say so and offer to ingest. Never fabricate an answer that pretends a query ran. The CLI ALWAYS returns real citations or an explicit error — surface those, don't summarise around them.
