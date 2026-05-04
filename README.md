# Learn-RV

**Turn any YouTube video into a queryable knowledge base — stored in RuVector's RVF format, entirely on your device.**

Point Learn-RV at a URL, a playlist, or a search query. It downloads the transcript, builds a semantic index, and lets you ask questions with cited, timestamped answers. No cloud database. No background services. One file per topic.

```bash
learn ingest "https://youtu.be/QZMljuD10sU" --topic claude-skills
learn ask claude-skills "What does the speaker recommend for skill design?"
# → "Progressive disclosure: start simple, reveal depth on demand [Claude Skills @ 4:32]"
```

Every answer points back to the exact moment in the exact video it came from. Every index is a single `.rvf` file you own, move, share, or delete like any other file.

> **RuVector users:** This is Learn-RV's native language. The `.rvf` binary format that backs all your RuVector KBs is the same one Learn-RV writes. A KB built here is readable by every other tool in the RuVector stack — and by the Cognitum One Seed natively.

---

## 30-second quickstart

```bash
# 1. Download + install (M-series Mac)
curl -L https://github.com/stuinfla/learner-rv/releases/latest/download/learn-aarch64-apple-darwin.tar.gz \
  | tar xz -C /tmp
/tmp/learn-aarch64-apple-darwin/install.sh

# 2. Check your environment
learn doctor

# 3. Ingest a video
learn ingest "https://youtu.be/QZMljuD10sU" --topic my-first-topic

# 4. Ask a question
learn ask my-first-topic "What is the main idea?"

# 5. Chat with the KB (multi-turn, with memory)
learn chat my-first-topic
```

That's it. The knowledge base lives at `~/Docs/KB/my-first-topic.rvf`.

---

## Why RuVector users care about this

RuVector's RVF format gives you: append-only writes, HNSW logarithmic search, witness chains for provenance, single-file portability, and zero services to manage. Learn-RV is a direct demonstration of what that enables:

| You get | Because of RVF |
|---|---|
| Add videos anytime without corrupting the KB | Append-only segments |
| Query a 10,000-chunk KB in milliseconds | HNSW native to the file |
| Citations that are cryptographically anchored | Witness chain per chunk |
| Move the KB to any machine, no migration | Single file = single unit |
| Runs on Cognitum One Seed natively | RVF is the Seed's native vector format |

The design principle: **one topic = one `.rvf` file**. The file *is* the database. No Postgres, no Pinecone, no SQLite sidecar — just an RVF binary you fully own.

---

## Cognitum One Seed

Learn-RV is a natural fit for Cognitum One Seed owners who want to turn video libraries into queryable on-device knowledge.

**Why it fits:**
- RVF is the Seed's native vector store format — KBs built on your Mac run on the Seed without conversion
- The MCP server (`learn serve <topic>`) aligns directly with the Seed's 114-tool MCP proxy
- Every ingest writes a witness chain, matching the Seed's Ed25519 custody model
- The Rust codebase is compatible with the `cognitum-one` SDK

**Typical workflow for Seed users:**
1. Ingest a video collection on your Mac: `learn ingest "https://youtube.com/playlist?list=PLxxx" --topic my-topic`
2. Copy the `.rvf` file to your Seed's RVF store
3. Query from any MCP-capable agent connected to the Seed

The Cog Store listing for Learn-RV is in progress. In the meantime, the binary and source are available at `https://github.com/stuinfla/learner-rv`.

---

## Three ways to use it

### 🤖 As a Claude Code skill (just talk to Claude)

Learn-RV installs as a global Claude Code skill. In any Claude session, just describe what you want:

> "Build me a knowledge base on French cooking technique."
> "Watch this video and remember it: https://youtu.be/QZMljuD10sU"
> "What did the speaker say about progressive disclosure?"
> "Apply what we learned in french-cooking to draft a 3-course menu."

Claude reads the skill, picks the right `learn` subcommand, runs it, and returns a cited answer. No syntax to remember.

### 💻 As a CLI (direct control)

```bash
# Ingest
learn ingest "https://youtu.be/QZMljuD10sU" --topic claude-skills
learn ingest "https://youtube.com/playlist?list=PLxxx" --topic my-playlist

# Ask / apply
learn ask   french-cooking "what is lamination and why does it matter?"
learn apply french-cooking "give me a croissant recipe with weights in grams"

# Chat (multi-turn, session-persistent)
learn chat french-cooking
# > what hydration should I start at?
# Assistant: 78% is what most beginners aim for [1]. Above 80% gets sticky [2].

# Inspect
learn status french-cooking
learn list   french-cooking
learn cloud  french-cooking   # → SVG word cloud of the topic's key concepts
learn map                     # → PCA galaxy SVG across all your topics
```

`learn --help` lists all 19 subcommands.

### 🔌 As an MCP server (the KB drives Claude's tools end-to-end)

```json
// ~/.claude/mcp.json
{
  "mcpServers": {
    "learn-rv": {
      "command": "learn",
      "args": ["serve", "your-topic-name"]
    }
  }
}
```

Claude Code gains three tools: `kb_query`, `kb_synthesize`, `kb_list_videos`. Now you can say *"using my french-cooking topic, walk me through making croissants — write the schedule to disk, adjust if I tell you my kitchen is 68°F"* and Claude calls the KB at each step, grounding every instruction in a specific video moment.

---

## Platform support

| Platform | Binary? | Notes |
|---|---|---|
| M-series Mac (`aarch64-apple-darwin`) | ✅ v0.2.2 | Primary, fully supported |
| Linux x86_64 (`x86_64-unknown-linux-gnu`) | ✅ v0.2.2 | Captions-only path (no local Whisper) |
| Intel Mac (`x86_64-apple-darwin`) | Build from source | macos-13 runner deprecated by GitHub |
| Linux ARM64 | Build from source | Cross-Docker can't reach RuVector path-deps |
| Windows | Build from source | whisper-rs metal feature is Apple-only |

---

<details><summary>📦 All 19 commands</summary>

### Discovery + ingestion

**`learn ingest`** — Tactical: paste a URL, playlist, channel, or local folder.

```bash
learn ingest "https://youtube.com/playlist?list=PLxxx"
learn ingest "https://youtu.be/abc" --topic indexed-arbitrage
learn ingest "/Users/me/lectures/" --topic university-physics
```

**`learn study`** — Strategic: describe what you want to learn. Learn-RV discovers a curriculum, ranks candidates, shows a shortlist, ingests on confirmation.

```bash
learn study "How to make laminated pastry"
learn study "ETF arbitrage strategies" --depth deep
learn study "RAG architectures 2026" --auto
```

### Consumption

**`learn ask`** — Cited answer grounded in the KB.
**`learn apply`** — Uses the KB as prior to produce a grounded artifact (recipe, plan, code).
**`learn chat`** — Multi-turn dialog with session persistence.

```bash
learn ask  french-cooking  "what is the Maillard reaction?"
learn apply french-cooking "give me a laminated dough schedule for 20 croissants"
learn chat  french-cooking   # → interactive REPL
learn chat  french-cooking --resume <session-id>  # → resume a prior session
```

Sessions persist at `~/Docs/KB/_chat/<topic>/<id>.jsonl`.

### Inspection + visualization

```bash
learn status  french-cooking   # chunk count, file size, coherence KPI
learn list    french-cooking   # videos in the topic
learn who-said french-cooking  "Julia Child"  # which videos mention a name
learn timeline french-cooking  "beurrage"     # chronological mentions of a concept
learn compare french-cooking sourdough        # cross-topic concept overlap
learn cloud   french-cooking                  # SVG word cloud of top concepts
learn map                                     # PCA galaxy of all your topics
```

### Maintenance

```bash
learn watch   french-cooking   # monitor a channel for new videos, auto-ingest
learn eval    french-cooking   # run golden Q&A regression against the KB
learn forget  french-cooking <video_id>  # remove one video from the KB
learn compact french-cooking   # defragment the RVF file, reclaim dead space
learn doctor                   # check deps, models, env, latest release version
learn serve   french-cooking   # start MCP server for Claude Code integration
learn summarize french-cooking # summarize key takeaways across the topic
```

</details>

<details><summary>🏗️ How it works</summary>

### Ingest pipeline

```
Source URL / path
      ↓
  ACQUIRE (yt-dlp)
  captions-first; audio-only fallback
      ↓
  SMART FRAME DECISION
  pHash variance analysis → skip talking heads, extract visual demos
  Sonnet vision captions frames when useful
      ↓
  TRANSCRIBE
  VTT captions (instant) or Whisper.cpp on-device (audio never leaves device)
      ↓
  CHUNK
  Sentence-aware, ~300 tokens, 50-token overlap
      ↓
  EMBED
  BGE-large-en-v1.5 (1024-dim, ONNX, on-device)
      ↓
  INDEX
  RvfStore append-only HNSW + witness chain per chunk
      ↓
  AUTO-SUMMARY
  3–5 key takeaways via Sonnet → ~/Docs/KB/<topic>.summary.md
      ↓
  ~/Docs/KB/<topic>.rvf
```

### Query path

```
User question
      ↓
  EXPAND — HyDE hypothetical answer as second query vector
      ↓
  HYBRID RETRIEVE — dense (BGE) + BM25, RRF fusion → top 50
      ↓
  RERANK — cross-encoder (BGE-base) → top 10
      ↓
  MMR + SOURCE-CAP — diversity λ=0.7, ≤3 chunks per video
      ↓
  SYNTHESIZE — cited prompt, abstain if signal weak, AIMDS scan in/out
      ↓
  Answer with [Title @ MM:SS](url&t=Xs) citations
```

### Architecture: 13 crates, one binary

| Layer | Crate | Responsibility |
|---|---|---|
| CLI | `learn-cli` | 19 subcommands, routing, orientation |
| Ingestion | `learn-acquire`, `learn-asr`, `learn-frames`, `learn-chunk`, `learn-embed`, `learn-index`, `learn-graph` | Full pipeline from URL to `.rvf` |
| Retrieval | `learn-retrieve` | Hybrid BM25+dense, rerank, MMR |
| Synthesis | `learn-synth` | Cited answers, in-tree AIMDS scanner |
| Chat | `learn-chat` | Multi-turn REPL, JSONL sessions |
| MCP | `learn-serve` | JSON-RPC 2.0 server for Claude Code |
| Contracts | `learn-core` | Shared types, errors, topic slug |

### Storage model

```
~/Docs/KB/
├── french-cooking.rvf         ← chunks · embeddings · HNSW · witness chain
├── indexed-arbitrage.rvf
├── french-cooking.summary.md  ← auto-generated key takeaways
├── _graph/
│   └── french-cooking.graphdb ← claims, entities, relations
├── _meta/
│   └── french-cooking.json    ← per-video state (slug → progress)
└── _chat/
    └── french-cooking/        ← session JSONL files
```

Per-topic isolation is total. Drop a topic by deleting one file. Move the whole thing to another machine and it just works.

### The intelligence stack

- **BGE-large-en-v1.5 (1024-dim)** — best-in-class English sentence embedder, on-device ONNX
- **HNSW via RvfStore** — logarithmic search, native to the file format
- **SONA per-topic adapters** — LoRA fine-tuning per topic; the embedder specializes with use
- **In-tree AIMDS** — 12 inbound + 8 outbound regex patterns; scans every query and every answer

### Why each design decision

| Decision | Rationale |
|---|---|
| Captions-first acquisition | Skips multi-MB video download on ~95% of YouTube content |
| Local Whisper | Audio never leaves the device; no per-minute API spend |
| Sentence-aware chunking | Mid-thought splits destroy retrieval coherence |
| BGE-large (1024-dim) | Better separation than 384-dim baselines as corpus grows |
| HNSW via RVF | Logarithmic search; same format as every other RuVector tool |
| Hybrid retrieval (dense + BM25) | Dense misses exact-keyword; BM25 misses paraphrase. RRF covers both |
| Cross-encoder reranker | Fixes order errors when concept and wording diverge |
| MMR + source-cap | Prevents one chatty video monopolizing results |
| Citation-grounded synthesis | Every claim → `[Title @ MM:SS](url&t=Xs)`. One-click verification |
| Abstain rule | When the corpus doesn't cover a question, says so instead of inventing |
| Witness chain | Citations are cryptographically anchored on insert, not just text |

</details>

<details><summary>📂 One-time setup</summary>

**Easy path (M-series Mac or Linux x86_64 — no Rust required):**

```bash
# M-series Mac
curl -L https://github.com/stuinfla/learner-rv/releases/latest/download/learn-aarch64-apple-darwin.tar.gz \
  | tar xz -C /tmp && /tmp/learn-aarch64-apple-darwin/install.sh

# Linux x86_64
curl -L https://github.com/stuinfla/learner-rv/releases/latest/download/learn-x86_64-unknown-linux-gnu.tar.gz \
  | tar xz -C /tmp && /tmp/learn-x86_64-unknown-linux-gnu/install.sh
```

`install.sh` symlinks the binary to `~/.cargo/bin/learn` and drops the Claude Code skill into `~/.claude/skills/learn-rv/`.

**Build from source (any platform, Rust toolchain required):**

```bash
git clone https://github.com/stuinfla/learner-rv.git
cd learner-rv
git clone https://github.com/ruvnet/RuVector.git ../RuVector
cargo install --path crates/learn-cli
mkdir -p ~/.claude/skills/learn-rv
cp .claude/skills/learn-rv/SKILL.md ~/.claude/skills/learn-rv/SKILL.md
```

**Runtime dependencies:**
```bash
brew install yt-dlp ffmpeg   # macOS
# apt install yt-dlp ffmpeg  # Debian/Ubuntu
```

Whisper and BGE-large models auto-fetch into `~/.cache/learn-rs/models/` on first use (`learn doctor` shows status).

</details>

<details><summary>⚙️ Configuration</summary>

| Variable | Purpose | Default |
|---|---|---|
| `ANTHROPIC_API_KEY` | Required for `learn ask` / `learn apply` synthesis | unset |
| `LEARN_SYNTH_LOCAL` | `1` → use local RuVLLM instead of Anthropic. Fully on-device | `0` |
| `LEARN_AIMDS_REQUIRED` | `1` → fail closed on any `Blocked` AIMDS verdict | `0` |
| `LEARN_KB_ROOT` | Where `.rvf` files live | `~/Docs/KB` |
| `LEARN_MODEL_CACHE` | Where Whisper + BGE models cache | `~/.cache/learn-rs/models` |
| `LEARN_LOG` | Tracing filter (`info`, `debug`, `learn_synth=trace`) | `info` |

**Sovereignty defaults:** Every byte of audio, every transcript, every embedding, and every index stays on the machine. The only outbound call is `learn ask`/`learn apply` to Anthropic — swap for local RuVLLM with `LEARN_SYNTH_LOCAL=1`.

</details>

<details><summary>⚠️ Honest caveats</summary>

Current state: v0.2.2 (2026-05-04)

- **Linux ARM64 + Windows binaries are not published.** Build from source. Reasons: `whisper-rs` metal feature is Apple-only; `cross` Docker cannot reach the `../ruvector` sibling path-dep on aarch64-linux.
- **Coherence KPI** uses Fiedler eigenvalue × NN-cosine density — a useful relative health signal, not a research-grade IIT Φ.
- **AIMDS guardrails** are in-tree regex patterns (12 inbound, 8 outbound). Synchronous, zero-subprocess, intentionally lightweight.
- **SONA self-learning** works but the feedback signal that updates the LoRA adapter requires explicit `record_feedback` API calls — not yet wired into a passive thumbs-up/down on `learn ask`.
- **Smart frame decision** runs pHash variance; low-variance (talking-head) videos skip frame extraction automatically to save API budget. Ambiguous videos optionally probe Sonnet vision.

</details>

<details><summary>🧪 Testing</summary>

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --release --workspace
```

CI requires all four green before merge. 316 unit + integration tests.

</details>

<details><summary>📜 License + contributing</summary>

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE) at your option.

Contributions welcome. Open an issue before sending a PR larger than ~50 lines so we can align on approach. CI gate must be green.

</details>

---

*Built with [RuVector](https://github.com/ruvnet/ruvector) · MIT/Apache-2.0 · [Releases](https://github.com/stuinfla/learner-rv/releases)*
