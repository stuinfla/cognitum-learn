# Cognitum Learn

![Hero: videos and content become an instant expert on your Cognitum Seed](assets/hero.svg)

**You have a Cognitum One Seed. You want it to be a genius on something that matters to you.**

Pick any topic — cooking, investing, a medical condition, a sport, a language. Cognitum Learn finds the best videos and content on the internet, downloads everything, reads every word, and turns it into a searchable expert that lives on your Seed. Then ask it anything, in plain language, and it answers with citations back to the exact moment in the exact video.

No cloud account. No subscription. No ongoing fees. Your knowledge, on your device, working offline.

![Cognitum Learn overview](assets/diagrams/top-level-invocation.svg)

<details>
<summary>Overview diagram (text version for accessibility)</summary>

```
Talk to Claude            Use the CLI              Use MCP Server
"Build me a KB on         learn ingest <url>        learn serve <topic>
 French cooking"          learn ask <topic> "q"     → Claude gains
"Watch this video"        learn chat <topic>          kb_query
"What did it say?"        learn apply <topic> "t"     kb_synthesize
        ↓                         ↓                         ↓
                      learn binary
                            ↓
                    <topic>.rvf
               (one file, on your device)
```

</details>

---

## Your Instant Expert in Four Steps

![Quickstart steps](assets/quickstart.svg)

<details>
<summary>Steps (text version)</summary>

```
1. Download  →  2. Pick a topic  →  3. Build your KB  →  4. Ask anything
   learn doctor     learn study "X"    learn ingest <url>   learn ask <topic> "?"
```

</details>

```bash
# 1. Install — one command (requires Rust toolchain: https://rustup.rs)
cargo install --git https://github.com/stuinfla/cognitum-learn learn-cli

# 2. Install system deps for video + audio (one-time)
brew install yt-dlp ffmpeg          # macOS
# sudo apt install yt-dlp ffmpeg    # Debian/Ubuntu

# 3. Verify (auto-fetches the BGE embedder on first use, ~130 MB)
learn doctor

# 4. Open the visual dashboard at http://127.0.0.1:7878/visual
learn ui

# — OR drive it from the CLI —

# 5. Build your first KB from a topic phrase OR YouTube URL
learn study "sous vide cooking techniques"     # autonomous discovery + ingest
# learn ingest "https://youtube.com/playlist?..."  # ingest a specific playlist

# 6. Ask your new expert anything (cited answers w/ timestamps)
learn ask sous-vide "What temperature for a medium-rare steak?"
# → "54°C for 1–4 hours gives perfect medium-rare edge-to-edge [Sous Vide Everything @ 3:12]"

# 7. Multi-turn chat that remembers the conversation
learn chat sous-vide
```

> Your knowledge base lives at `~/Docs/KB/sous-vide.rvf` — one file you own completely.
>
> **Don't want to install Rust?** Download a prebuilt binary from the [latest release](https://github.com/stuinfla/cognitum-learn/releases/latest) — Apple Silicon, Linux x86_64/aarch64, and Windows x86_64 builds are signed and published every tag.

---

## What You Get

![Capability matrix](assets/diagrams/capability-matrix.svg)

<details>
<summary>Capabilities (text version)</summary>

```
Own your data   │  Cited answers  │  Self-learning
No cloud. One   │  Every answer   │  The KB gets
.rvf file you   │  points to the  │  smarter the
control fully.  │  exact moment.  │  more you use it.

On-device       │  RuVector-      │  Scales with
Everything runs │  native         │  you
on your machine.│  .rvf works     │  From one video
Audio never     │  with the whole │  to thousands,
leaves.         │  RuVector stack.│  same commands.
```

</details>

| You get | Because of how it works |
|---|---|
| Add videos anytime without corrupting the KB | Append-only RVF segments |
| Millisecond search across thousands of video chunks | HNSW index native to the file |
| Every answer traces to the exact video moment | Witness chain per chunk, cryptographically anchored |
| Move the whole KB to another machine — nothing to migrate | Single `.rvf` file = single unit |
| Works on Cognitum One Seed without conversion | RVF is the Seed's native vector format |

---

## For Cognitum One Seed Owners

**Your Seed is where all your knowledge lives.** Build a knowledge base on your computer, and it lands on the Seed automatically — no cloud, no subscription, no conversion. Just your hardware.

![Cognitum One Seed workflow](assets/seed-workflow.svg)

<details>
<summary>Seed workflow (text version for accessibility)</summary>

```
YOUR COMPUTER                    AUTO-PUSH             COGNITUM ONE SEED
─────────────────────────────    ────────────────────  ─────────────────────────────
learn ingest <video URLs>                              Seed RVF Store
learn study "your topic"       → every ingest pushes  native vector format
learn ask / chat / apply          automatically   →   zero conversion needed

~/Docs/KB/<topic>.rvf                                 114-tool MCP proxy
one file · fully portable                             any MCP-capable agent
                                                       Ed25519 witness chain
                                                       cryptographic provenance
```

</details>

**One-time setup — bind your Seed and forget it:**

```bash
learn config set seed.address 192.168.1.42    # your Seed's IP (or mDNS name)
learn config set seed.auto_push true          # push automatically after every ingest
learn doctor                                  # confirm Seed is reachable
```

After this, every `learn ingest` and `learn study` automatically pushes to your Seed. You never need to remember to push.

**Manual push (if you prefer explicit control):**

```bash
learn push knife-sharpening                   # push on demand, auto-discovers Seed
learn push knife-sharpening --seed 192.168.1.42  # explicit address
```

**Full workflow example:**

```bash
# Build the expert
learn study "Japanese knife sharpening"       # finds + ingests best videos
learn ask knife-sharpening "What angle for a 210mm gyuto?"

# If auto-push is enabled, the Seed already has it.
# If not, push manually:
learn push knife-sharpening

# Now any AI agent connected to the Seed can query it
```

**Why it fits the Seed:**
- RVF is the Seed's native vector store — no conversion, no export, no migration
- `learn serve <topic>` aligns with the Seed's 114-tool MCP proxy  
- Every ingest writes an Ed25519 witness chain, matching the Seed's custody model
- The Rust binary is compatible with the `cognitum-one` SDK

---

## Dimensions & the Cognitum Seed

**The Seed is not limited to any particular vector dimension.** This is worth stating plainly because it is easy to misread: if you inspect a fresh Seed you may see a store reporting `dimension: 8`, and conclude the device only handles 8-dimensional vectors. That is wrong. The "8" is just the dimension that store happened to be initialized with — typically by the on-device sensor pipeline, which writes small vectors.

How a Cognitum Seed decides its store dimension:

1. **The agent's `--dimension N` launch flag is authoritative.** The Cognitum agent runs with a configured dimension (set in `/etc/systemd/system/cognitum-agent.service.d/override-dimension.conf`). Its proof-verifier rejects any vector whose length ≠ `N`. This is a *configuration value*, not a hardware ceiling.
2. **A fresh store also locks to the first vector written.** If no flag is set, the dimension is fixed by whatever writes first.
3. **The store is single-tenant.** One active store holds one dimension. The sensor pipeline and a knowledge base cannot share a store at different dimensions — so for the "build an expert" use case, the store is given over to the KB.

Cognitum Learn embeds at **384 dimensions** (BGE-small-en-v1.5) by default. To make a Seed accept those vectors:

```bash
# On the Seed (over SSH), point the agent at 384 and start with a clean store:
sudo sed -i 's/--dimension [0-9]*/--dimension 384/' \
  /etc/systemd/system/cognitum-agent.service.d/override-dimension.conf
sudo systemctl daemon-reload
sudo systemctl stop cognitum-agent
sudo rm -f /var/lib/cognitum/rvf-store/*.rvf      # clears the old-dimension store
sudo systemctl start cognitum-agent

# Then from your computer, push a 384-dim KB:
learn push my-topic --seed <SEED_IP>              # → store reports "dimension": 384
```

A push to a Seed whose store is configured for a different dimension fails fast with a clear error (`dimension mismatch: expected N, got 384`) — it never silently corrupts the store. If you want a different embedding dimension, set both the model (`LEARN_EMBED_MODEL_DIR`) and the Seed's `--dimension` flag to match.

---

## Four Ways to Use It

Whether you prefer a point-and-click dashboard, talking to Claude, typing commands, or wiring it into an AI workflow — it all leads to the same place: your knowledge, cited, on your device.

![Three ways to use Cognitum Learn](assets/three-modes.svg)

<details>
<summary>Three modes (text version for accessibility)</summary>

```
Claude Skill              CLI                        MCP Server
──────────────────        ─────────────────────────  ──────────────────────────────
Talk naturally:           learn ingest <url>          learn serve <topic>
"Build me a KB on         learn ask <topic> "q"
 French cooking"          learn chat <topic>          Claude gains:
"Watch this video"        learn apply <topic> "t"       · kb_query
"What did it say?"                                     · kb_synthesize
                          learn status / list           · kb_list_videos
Claude picks the          learn cloud / map / ui
right command and         26 subcommands total        Grounded multi-step
runs it for you.                                      workflows — every answer
No syntax needed.                                     anchored to a video moment.
```

</details>

### 🤖 As a Claude Code skill (just talk to Claude)

Cognitum Learn installs as a global Claude Code skill. In any Claude session, just describe what you want:

> "Build me a knowledge base on Japanese knife sharpening."  
> "Watch this video and remember it: https://youtu.be/QZMljuD10sU"  
> "What did the speaker say about sharpening angle?"  
> "Apply what we learned in knife-sharpening to draft a sharpening routine for my 3 knives."

Claude reads the skill, picks the right `learn` subcommand, runs it, and returns a cited answer. No syntax to remember.

### 💻 As a CLI (direct control)

```bash
# Build a knowledge base
learn ingest "https://youtu.be/QZMljuD10sU" --topic claude-skills
learn ingest "https://youtube.com/playlist?list=PLxxx" --topic my-playlist
learn import ~/Downloads/lectures/ --topic university-physics   # local files

# Ask / apply / chat
learn ask   french-cooking "what is lamination and why does it matter?"
learn apply french-cooking "give me a croissant recipe with weights in grams"
learn chat  french-cooking                       # multi-turn dialog, session-persistent

# Inspect and visualize
learn status french-cooking                      # chunk count, coherence score
learn cloud  french-cooking                      # → SVG word cloud of key concepts
learn map                                        # → PCA galaxy of all your topics

# Push to your Cognitum Seed
learn push french-cooking
```

### 🔌 As an MCP server (Claude drives the KB end-to-end)

```json
// ~/.claude/mcp.json
{
  "mcpServers": {
    "cognitum-learn": {
      "command": "learn",
      "args": ["serve", "your-topic-name"]
    }
  }
}
```

Claude Code gains three tools: `kb_query`, `kb_synthesize`, `kb_list_videos`. Now you can say _"using my french-cooking topic, walk me through making croissants — write the schedule to disk, adjust if I tell you my kitchen is 68°F"_ and Claude calls the KB at each step, grounding every instruction in a specific video moment.

### 🖥️ As a web dashboard (point-and-click, no terminal)

```bash
learn ui          # starts a local server at http://127.0.0.1:7878 and opens your browser
```

A self-contained React dashboard served by the built-in Axum bridge — everything stays on your machine. It includes a guided onboarding wizard (discover your Seed → pick a topic → watch the ingest progress live → chat with your new expert), so a brand-new Cognitum Seed owner can go from zero to a working expert without touching the command line.

---

<details><summary>📦 All 26 commands</summary>

### Discovery + ingestion

**`learn study`** — Strategic: describe what you want to learn. Cognitum Learn discovers a curriculum, ranks candidates, shows a shortlist, ingests on confirmation.

```bash
learn study "How to make laminated pastry"
learn study "ETF arbitrage strategies" --depth deep
learn study "RAG architectures 2026" --auto
```

**`learn ingest`** — Tactical: paste a URL, playlist, channel, or search query.

```bash
learn ingest "https://youtube.com/playlist?list=PLxxx"
learn ingest "https://youtu.be/abc" --topic indexed-arbitrage
```

**`learn import`** — Bulk ingest a local directory of files (PDF, MP4, MP3, TXT, MD).

```bash
learn import ~/Downloads/lectures/ --topic university-physics
learn import ~/Documents/recipes/ --topic french-cooking
```

### Consumption

**`learn ask`** — Cited answer grounded in the KB.  
**`learn apply`** — Uses the KB as prior to produce a grounded artifact (recipe, plan, code).  
**`learn chat`** — Multi-turn dialog with session persistence.  
**`learn quiz`** — Generates quiz questions from the KB to test your knowledge.

```bash
learn ask   french-cooking "what is the Maillard reaction?"
learn apply french-cooking "give me a laminated dough schedule for 20 croissants"
learn chat  french-cooking                                    # → interactive REPL
learn chat  french-cooking --resume <session-id>             # → resume a prior session
learn quiz  french-cooking                                    # → 5 questions with answers
```

Sessions persist at `~/Docs/KB/_chat/<topic>/<id>.jsonl`.

### Inspection + visualization

```bash
learn status   french-cooking   # chunk count, file size, coherence KPI
learn list     french-cooking   # videos in the topic
learn who-said french-cooking "Julia Child"          # which videos mention a name
learn timeline french-cooking "beurrage"             # chronological mentions
learn compare  french-cooking sourdough              # cross-topic concept overlap
learn cloud    french-cooking                        # SVG word cloud of top concepts
learn map                                            # PCA galaxy of all your topics
learn summarize french-cooking                       # key takeaways across the topic
```

### Distribution + maintenance

```bash
learn push    french-cooking   # push KB to Cognitum One Seed on local network
learn serve   french-cooking   # start MCP server for Claude Code integration
learn ui                       # local web dashboard (onboarding wizard + chat) in your browser
learn watch   french-cooking   # monitor a channel for new videos, auto-ingest
learn eval    french-cooking   # run golden Q&A regression against the KB
learn forget  french-cooking <video_id>    # remove one video from the KB
learn compact french-cooking               # dedupe + optimize the RVF HNSW index
learn doctor                               # check deps, models, env, release version
```

### Setup + configuration

```bash
learn setup                                # guided first-run wizard (deps, model, Seed binding)
learn config set seed.address 192.168.1.42 # persist your Seed's address
learn config set seed.auto_push true       # auto-push after every ingest
learn config get seed                      # read current configuration
```

</details>

<details><summary>🏗️ How it works</summary>

### Ingest pipeline

![Ingest pipeline](assets/diagrams/ingest-pipeline.svg)

<details>
<summary>Pipeline (text version for accessibility)</summary>

```
Source URL / path
      ↓
  ACQUIRE (yt-dlp) — captions-first; audio-only fallback
      ↓
  SMART FRAME DECISION
  pHash variance → skip talking heads, extract visual demos
  Sonnet vision captions frames when useful
      ↓
  TRANSCRIBE — VTT captions (instant) or Whisper.cpp on-device
      ↓
  CHUNK — sentence-aware, ~300 tokens, 50-token overlap
      ↓
  EMBED — BGE-small-en-v1.5 (384-dim, ONNX, on-device)
      ↓
  INDEX — RvfStore append-only HNSW + Ed25519 witness chain per chunk
      ↓
  AUTO-SUMMARY — 3–5 key takeaways via Sonnet
      ↓
  ~/Docs/KB/<topic>.rvf
```

</details>

### Query path

![Query path](assets/diagrams/query-path.svg)

<details>
<summary>Query path (text version for accessibility)</summary>

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

</details>

### Architecture: 17 crates, one binary

![Architecture diagram](assets/diagrams/architecture.svg)

| Layer | Crate | Responsibility |
|---|---|---|
| CLI | `learn-cli` | 26 subcommands, routing, orientation |
| Ingestion | `learn-acquire`, `learn-asr`, `learn-frames`, `learn-chunk`, `learn-embed`, `learn-index`, `learn-graph` | Full pipeline from URL to `.rvf` |
| Retrieval | `learn-retrieve` | Hybrid BM25+dense, rerank, MMR |
| Synthesis | `learn-synth` | Cited answers, in-tree AIMDS scanner |
| Chat | `learn-chat` | Multi-turn REPL, JSONL sessions |
| MCP | `learn-serve` | JSON-RPC 2.0 server for Claude Code |
| Contracts | `learn-core` | Shared types, errors, topic slug |

### Storage model

![Storage model](assets/diagrams/storage-model.svg)

<details>
<summary>Storage layout (text version)</summary>

```
~/Docs/KB/
├── french-cooking.rvf          ← chunks · embeddings · HNSW · witness chain
├── indexed-arbitrage.rvf
├── french-cooking.summary.md   ← auto-generated key takeaways
├── _graph/
│   └── french-cooking.graphdb  ← claims, entities, relations
├── _meta/
│   └── french-cooking.json     ← per-video state (slug → progress)
└── _chat/
    └── french-cooking/         ← session JSONL files
```

</details>

Per-topic isolation is total. Drop a topic by deleting one file. Move the whole thing to another machine and it just works.

### Self-learning

![Learning flywheel](assets/diagrams/learning-flywheel.svg)

- **BGE-small-en-v1.5 (384-dim)** — compact, fast, on-device ONNX embedder; 384 dims is the default so a knowledge base is small and quick to store on a Cognitum Seed (≈37% the size of a 1024-dim store). Override with `LEARN_EMBED_MODEL_DIR` if you want a different model.
- **HNSW via RvfStore** — logarithmic search, native to the file format
- **SONA per-topic adapters** — LoRA fine-tuning per topic; the embedder specializes with use
- **In-tree AIMDS** — 12 inbound + 8 outbound regex patterns; scans every query and every answer

</details>

<details><summary>📂 One-time setup</summary>

**Recommended — install from GitHub via cargo (one command, any platform, Rust toolchain required):**

```bash
cargo install --git https://github.com/stuinfla/cognitum-learn learn-cli
```

This fetches `cognitum-learn` and its RuVector dependencies directly from GitHub — no manual sibling-clone, no `--path`. Compiles in ~3-5 min on a modern laptop, installs the `learn` binary into `~/.cargo/bin/`.

**Optional — pin a specific release tag** for reproducibility:

```bash
cargo install --git https://github.com/stuinfla/cognitum-learn --tag v0.5.4 learn-cli
```

**Build from a local checkout instead** (only needed if you want to hack on the workspace):

```bash
git clone https://github.com/stuinfla/cognitum-learn.git
cd cognitum-learn
cargo install --path crates/learn-cli
mkdir -p ~/.claude/skills/cognitum-learn
cp .claude/skills/cognitum-learn/SKILL.md ~/.claude/skills/cognitum-learn/SKILL.md
```

Note: as of v0.5.4 you no longer need to clone the sibling `RuVector` repo — cargo fetches it from `github.com/ruvnet/RuVector` automatically.

**Prebuilt binaries (no Rust toolchain required — M-series Mac / Linux x86_64 / Linux aarch64 / Windows x86_64):**

```bash
# M-series Mac
T=$(mktemp -d) && curl -L https://github.com/stuinfla/cognitum-learn/releases/latest/download/learn-aarch64-apple-darwin.tar.gz | tar xz -C "$T" && "$T/learn-aarch64-apple-darwin/install.sh"

# Linux x86_64
T=$(mktemp -d) && curl -L https://github.com/stuinfla/cognitum-learn/releases/latest/download/learn-x86_64-unknown-linux-gnu.tar.gz | tar xz -C "$T" && "$T/learn-x86_64-unknown-linux-gnu/install.sh"

# Linux aarch64 (Raspberry Pi 5, Jetson, ARM servers)
T=$(mktemp -d) && curl -L https://github.com/stuinfla/cognitum-learn/releases/latest/download/learn-aarch64-unknown-linux-gnu.tar.gz | tar xz -C "$T" && "$T/learn-aarch64-unknown-linux-gnu/install.sh"

# Windows x86_64 — see the .zip artifact on the GitHub releases page
```

`install.sh` symlinks the binary to `~/.cargo/bin/learn` and drops the Claude Code skill into `~/.claude/skills/cognitum-learn/`.

**Runtime dependencies:**
```bash
brew install yt-dlp ffmpeg   # macOS
# apt install yt-dlp ffmpeg  # Debian/Ubuntu
```

Whisper and the BGE-small embedder auto-fetch into `~/.cache/learn-rs/models/` on first use (`learn doctor` shows status).

**Environment setup:**

Copy `.env.example` to `.env` and fill in your Anthropic API key (required for `learn ask`, `learn apply`, and `learn chat`):

```bash
cp .env.example .env
# edit .env and add: ANTHROPIC_API_KEY=sk-ant-...
```

</details>

<details><summary>⚙️ Configuration</summary>

| Variable | Purpose | Default |
|---|---|---|
| `ANTHROPIC_API_KEY` | Required for `learn ask` / `learn apply` / `learn chat` synthesis | unset |
| `LEARN_SYNTH_LOCAL` | `1` → use local RuVLLM instead of Anthropic. Fully on-device | `0` |
| `LEARN_AIMDS_REQUIRED` | `1` → fail closed on any `Blocked` AIMDS verdict | `0` |
| `LEARN_KB_ROOT` | Where `.rvf` files live | `~/Docs/KB` |
| `LEARN_MODEL_CACHE` | Where Whisper + BGE models cache | `~/.cache/learn-rs/models` |
| `RUST_LOG` | Tracing filter (`info`, `debug`, `learn_synth=trace`) | `warn` |

**Sovereignty defaults:** Every byte of audio, every transcript, every embedding, and every index stays on the machine. The only outbound call is `learn ask`/`learn apply` to Anthropic — swap for local RuVLLM with `LEARN_SYNTH_LOCAL=1`.

</details>

<details><summary>🖥️ Platform support</summary>

| Platform | Binary? | Notes |
|---|---|---|
| M-series Mac (`aarch64-apple-darwin`) | ✅ v0.5.4+ | Primary, fully supported |
| Linux x86_64 (`x86_64-unknown-linux-gnu`) | ✅ v0.5.4+ | Captions-only (no local Whisper on Linux) |
| Windows (`x86_64-pc-windows-msvc`) | ✅ v0.5.4+ | No on-device ASR (whisper-rs is Apple-only) |
| Intel Mac (`x86_64-apple-darwin`) | Build from source | macOS-13 runner deprecated by GitHub |
| Linux ARM64 | Build from source | cross-Docker can't reach RuVector path-deps |

</details>

<details><summary>⚠️ Honest caveats</summary>

Current state: v0.5.4 (2026-05-25)

- **Linux ARM64 + Intel Mac binaries are not published.** Build from source. Reasons: `cross` Docker cannot reach the `../ruvector` sibling path-dep; macOS-13 runner deprecated.
- **Coherence KPI** uses Fiedler eigenvalue × NN-cosine density — a useful relative health signal, not a research-grade IIT Φ.
- **AIMDS guardrails** are in-tree regex patterns (12 inbound, 8 outbound). Synchronous, zero-subprocess, intentionally lightweight.
- **SONA self-learning** works but the feedback signal that updates the LoRA adapter requires explicit `record_feedback` API calls — not yet wired into a passive thumbs-up/down on `learn ask`.
- **Smart frame decision** runs pHash variance; low-variance (talking-head) videos skip frame extraction automatically to save API budget.
- **Windows** builds and runs but omits on-device speech recognition (`learn-asr` is Apple-only due to whisper-rs metal feature).
- **Embedder dimension changed to 384 (BGE-small) in v0.2.17.** Knowledge bases built by earlier versions are 1024-dim (BGE-large) and will not match new 384-dim query vectors. Re-ingest a topic (`learn ingest …` / `learn study …`) to rebuild it at 384. `learn compact` does **not** re-embed — it only dedupes and optimizes the existing HNSW index.
- **A Cognitum Seed enforces one dimension per store**, set by its agent's `--dimension` flag and the first vector written. Point it at 384 to match the new default (see [Dimensions & the Cognitum Seed](#dimensions--the-cognitum-seed)). The Seed is **not** limited to any particular dimension — it stores whatever its store is configured for.

</details>

<details><summary>🧪 Testing</summary>

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --release --workspace
```

CI requires all four green before merge. 311+ unit + integration tests.

</details>

<details><summary>📜 License + contributing</summary>

Licensed under the [PolyForm Noncommercial License 1.0.0](LICENSE). Free for personal, research, and other noncommercial use. For a commercial license, contact the author.

Contributions welcome. Open an issue before sending a PR larger than ~50 lines so we can align on approach. CI gate must be green.

</details>

---

*Built with [RuVector](https://github.com/ruvnet/ruvector) · PolyForm Noncommercial 1.0.0 · [Releases](https://github.com/stuinfla/cognitum-learn/releases)*
