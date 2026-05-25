//! HTTP server for `learn ui` — serves the dashboard and REST API.

use axum::{
    extract::{Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{sse::Event, sse::KeepAlive, IntoResponse, Sse},
    routing::{get, post},
    Json, Router,
};
use camino::Utf8PathBuf;
use chrono::DateTime;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{convert::Infallible, sync::Arc, time::Duration};
use tokio::sync::mpsc;
use tokio_stream::{wrappers::ReceiverStream, StreamExt};
use tower_http::cors::CorsLayer;

static UI_HTML: &str = include_str!("../ui/index.html");
static UI_VISUAL_HTML: &str = include_str!("../ui/index-visual.html");
static ASSET_SEED_HERO: &[u8] = include_bytes!("../ui/assets/01-seed-hero.png");
static ASSET_CRYSTAL: &[u8] = include_bytes!("../ui/assets/02-crystallization.png");
static ASSET_HANDSHAKE: &[u8] = include_bytes!("../ui/assets/03-seed-handshake.png");
static ASSET_CASCADE: &[u8] = include_bytes!("../ui/assets/04-thumbnail-cascade.png");

// ── Shared state ──────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct AppState {
    pub kb_root: Utf8PathBuf,
}

// ── Router ────────────────────────────────────────────────────────────────────

pub fn build_router(kb_root: Utf8PathBuf) -> Router {
    let state = Arc::new(AppState { kb_root });
    Router::new()
        .route("/", get(serve_ui))
        .route("/visual", get(serve_ui_visual))
        .route(
            "/assets/01-seed-hero.png",
            get(|| async { png_response(ASSET_SEED_HERO) }),
        )
        .route(
            "/assets/02-crystallization.png",
            get(|| async { png_response(ASSET_CRYSTAL) }),
        )
        .route(
            "/assets/03-seed-handshake.png",
            get(|| async { png_response(ASSET_HANDSHAKE) }),
        )
        .route(
            "/assets/04-thumbnail-cascade.png",
            get(|| async { png_response(ASSET_CASCADE) }),
        )
        .route("/api/health", get(health))
        .route("/api/topics", get(list_topics))
        .route("/api/status", get(status))
        .route("/api/ask", post(ask))
        .route("/api/ingest/progress", get(ingest_progress))
        .route("/api/study/progress", get(study_progress))
        .route("/api/playlist/preview", post(playlist_preview))
        .route("/api/seed/discover", post(seed_discover))
        .route("/api/seed/configure", post(seed_configure))
        .with_state(state)
        // CorsLayer goes INNERMOST so its preflight short-circuit fires.
        // PNA middleware goes OUTERMOST so it stamps every response —
        // including the CORS preflight — with
        // Access-Control-Allow-Private-Network: true. This is required for
        // Chrome 122+ Private Network Access: HTTPS pages (Vercel) cannot
        // fetch from this localhost bridge without it.
        .layer(CorsLayer::permissive())
        .layer(axum::middleware::from_fn(add_private_network_header))
}

/// Adds `Access-Control-Allow-Private-Network: true` to every response.
/// Required for HTTPS pages (Vercel-hosted) to fetch from this localhost
/// bridge under Chrome's Private Network Access spec.
async fn add_private_network_header(
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let mut res = next.run(req).await;
    res.headers_mut().insert(
        "access-control-allow-private-network",
        "true".parse().unwrap(),
    );
    res
}

/// Start the HTTP server. Blocks until the process is killed.
pub async fn run(kb_root: Utf8PathBuf, port: u16) -> anyhow::Result<()> {
    let app = build_router(kb_root);
    let addr = format!("127.0.0.1:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("cognitum-learn dashboard → http://{addr}");
    axum::serve(listener, app).await?;
    Ok(())
}

// ── Handlers ─────────────────────────────────────────────────────────────────

fn png_response(bytes: &'static [u8]) -> impl IntoResponse {
    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, "image/png".parse().unwrap());
    headers.insert(
        header::CACHE_CONTROL,
        "public, max-age=31536000, immutable".parse().unwrap(),
    );
    (headers, bytes)
}

async fn serve_ui_visual() -> impl IntoResponse {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        "text/html; charset=utf-8".parse().unwrap(),
    );
    (headers, UI_VISUAL_HTML)
}

#[derive(Deserialize)]
struct PlaylistPreviewBody {
    url: String,
}

/// Resolve a YouTube playlist (or single video URL) to its real metadata.
///
/// Uses `yt-dlp --flat-playlist --dump-single-json` which is a metadata-only
/// call (no downloads). Returns:
/// - `title`, `uploader`, `count`, `total_duration_s`, `estimated_words`
/// - `videos`: list of `{id, title, duration_s, thumbnail}`
///
/// Used by the visual dashboard's "Reveal" beat so the experience shows
/// the user's actual playlist, not mock data.
async fn playlist_preview(
    Json(body): Json<PlaylistPreviewBody>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let output = tokio::process::Command::new("yt-dlp")
        .args([
            "--flat-playlist",
            "--dump-single-json",
            "--no-warnings",
            "--quiet",
            &body.url,
        ])
        .output()
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("yt-dlp failed to spawn: {e}")})),
            )
        })?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr).to_string();
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": err.lines().next().unwrap_or("yt-dlp error").to_string()})),
        ));
    }

    let raw: Value = serde_json::from_slice(&output.stdout).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("parse error: {e}")})),
        )
    })?;

    let entries = raw["entries"].as_array().cloned().unwrap_or_else(|| {
        // Single-video URL: yt-dlp returns the video object itself
        vec![raw.clone()]
    });

    let title = raw["title"].as_str().unwrap_or("Untitled").to_string();
    let uploader = raw["uploader"]
        .as_str()
        .or_else(|| raw["channel"].as_str())
        .unwrap_or("")
        .to_string();

    let mut total_duration_s: u64 = 0;
    let mut videos = Vec::with_capacity(entries.len());
    for entry in &entries {
        let id = entry["id"].as_str().unwrap_or("").to_string();
        let v_title = entry["title"].as_str().unwrap_or("Untitled").to_string();
        let duration_s = entry["duration"].as_f64().unwrap_or(0.0) as u64;
        total_duration_s += duration_s;

        // Prefer the highest-res thumbnail; fall back to /vi/ID/mqdefault.jpg
        let thumbnail = entry["thumbnails"]
            .as_array()
            .and_then(|arr| arr.last())
            .and_then(|t| t["url"].as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| {
                if !id.is_empty() {
                    format!("https://i.ytimg.com/vi/{id}/mqdefault.jpg")
                } else {
                    String::new()
                }
            });

        videos.push(json!({
            "id": id,
            "title": v_title,
            "duration_s": duration_s,
            "thumbnail": thumbnail,
        }));
    }

    // Spoken-word estimate: ~150 words per minute.
    let estimated_words = (total_duration_s / 60) * 150;

    Ok(Json(json!({
        "title": title,
        "uploader": uploader,
        "count": videos.len(),
        "total_duration_s": total_duration_s,
        "estimated_words": estimated_words,
        "videos": videos,
    })))
}

#[derive(Deserialize)]
struct StudyQuery {
    topic: String,
    #[serde(default = "default_study_videos")]
    max_videos: usize,
    #[serde(default = "default_study_depth")]
    depth: String,
}

fn default_study_videos() -> usize {
    20
}
fn default_study_depth() -> String {
    "deep".to_string()
}

/// Autonomous curriculum discovery + ingest.
///
/// Wraps `learn study <topic> --depth <depth> --max-videos N --auto` and
/// streams progress as SSE events with the same `{message, level, progress,
/// done}` shape as `/api/ingest/progress`. Used by the visual dashboard's
/// "Expand to a 20-video deep-dive" button when the user starts from a
/// single video or a topic phrase.
async fn study_progress(
    State(state): State<Arc<AppState>>,
    Query(q): Query<StudyQuery>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let (tx, rx) = mpsc::channel::<String>(64);
    let kb_root = state.kb_root.to_string();
    let topic = q.topic.clone();
    let max_videos = q.max_videos.to_string();
    let depth = q.depth.clone();

    tokio::spawn(async move {
        let send = |msg: &str, level: &str, pct: u8, done: bool| {
            let _ = tx.try_send(
                json!({"message": msg, "level": level, "progress": pct, "done": done}).to_string(),
            );
        };

        send(
            &format!("Starting autonomous study: \"{topic}\" · depth={depth} · target={max_videos} videos…"),
            "info", 2, false,
        );

        let learn_bin =
            std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("learn"));
        let mut child = match tokio::process::Command::new(&learn_bin)
            .args([
                "study",
                &topic,
                "--depth",
                &depth,
                "--max-videos",
                &max_videos,
                "--auto",
                "--kb-root",
                kb_root.as_str(),
            ])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                send(
                    &format!("Failed to spawn learn study: {e}"),
                    "error",
                    100,
                    true,
                );
                return;
            }
        };

        if let Some(stderr) = child.stderr.take() {
            use tokio::io::{AsyncBufReadExt, BufReader};
            let mut reader = BufReader::new(stderr).lines();
            let mut pct: u8 = 5;
            while let Ok(Some(line)) = reader.next_line().await {
                if line.trim().is_empty() {
                    continue;
                }
                // Crude phase-to-percent mapping; the front-end's beat router
                // does the visual stage routing from these messages.
                pct = (pct.saturating_add(1)).min(95);
                send(&line, "info", pct, false);
            }
        }

        let status = child.wait().await;
        match status {
            Ok(s) if s.success() => send("Study complete.", "info", 100, true),
            Ok(s) => send(
                &format!("Study failed: exit {:?}", s.code()),
                "error",
                100,
                true,
            ),
            Err(e) => send(&format!("Study wait error: {e}"), "error", 100, true),
        }
    });

    let stream = ReceiverStream::new(rx).map(|s| Ok(Event::default().data(s)));
    Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
}

async fn serve_ui() -> impl IntoResponse {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        "text/html; charset=utf-8".parse().unwrap(),
    );
    (headers, UI_HTML)
}

async fn health() -> Json<Value> {
    Json(json!({"ok": true}))
}

#[derive(Serialize)]
struct TopicEntry {
    slug: String,
    video_count: usize,
    chunks: u64,
    size_kb: u64,
    updated_at: String,
}

async fn list_topics(State(state): State<Arc<AppState>>) -> Json<Value> {
    let mut topics: Vec<TopicEntry> = Vec::new();

    if let Ok(mut rd) = tokio::fs::read_dir(state.kb_root.as_std_path()).await {
        while let Ok(Some(entry)) = rd.next_entry().await {
            let p = entry.path();
            if p.extension().and_then(|e| e.to_str()) != Some("rvf") {
                continue;
            }
            let slug = match p.file_stem().and_then(|s| s.to_str()) {
                Some(s) if !s.is_empty() && !s.starts_with('_') => s.to_string(),
                _ => continue,
            };
            let meta = entry.metadata().await.ok();
            let size_kb = meta.as_ref().map(|m| m.len() / 1024).unwrap_or(0);
            let updated_at = meta
                .as_ref()
                .and_then(|m| m.modified().ok())
                .map(|t| {
                    let secs = t
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    DateTime::from_timestamp(secs as i64, 0)
                        .map(|dt| dt.to_rfc3339())
                        .unwrap_or_default()
                })
                .unwrap_or_default();

            let (video_count, chunks) = read_topic_stats(&state.kb_root, &slug).await;
            topics.push(TopicEntry {
                slug,
                video_count,
                chunks,
                size_kb,
                updated_at,
            });
        }
    }

    topics.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Json(json!({"topics": topics}))
}

async fn read_topic_stats(kb_root: &Utf8PathBuf, slug: &str) -> (usize, u64) {
    // The ingest pipeline writes `<slug>.meta.json`, NOT `<slug>.manifest.json`.
    // Its shape is { "dimension": N, "chunks": { "<hash>": { "video_id": ..., ... } } }.
    // chunk count = number of entries; video count = unique video_ids.
    let meta_path = kb_root.join(format!("{slug}.meta.json"));
    if let Ok(bytes) = tokio::fs::read(&meta_path).await {
        if let Ok(v) = serde_json::from_slice::<Value>(&bytes) {
            if let Some(chunks) = v["chunks"].as_object() {
                let chunk_count = chunks.len() as u64;
                let mut videos = std::collections::HashSet::new();
                for chunk in chunks.values() {
                    if let Some(vid) = chunk["video_id"].as_str() {
                        videos.insert(vid.to_string());
                    }
                }
                return (videos.len(), chunk_count);
            }
        }
    }
    (0, 0)
}

async fn status(State(state): State<Arc<AppState>>) -> Json<Value> {
    let seed_addr: Option<String> = std::env::var("LEARN_SEED_ADDRESS").ok().or_else(|| {
        let path = dirs::config_dir()
            .unwrap_or_default()
            .join("learn-rs/config.json");
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str::<Value>(&s).ok())
            .and_then(|v| v["seed"]["address"].as_str().map(str::to_string))
            .filter(|s| !s.is_empty())
    });

    // Lightweight TCP probe instead of HTTP client dep
    let seed_connected = if let Some(ref addr) = seed_addr {
        let addr_with_port = if addr.contains(':') {
            addr.clone()
        } else {
            format!("{addr}:80")
        };
        tokio::time::timeout(
            Duration::from_millis(800),
            tokio::net::TcpStream::connect(&addr_with_port),
        )
        .await
        .ok()
        .and_then(|r| r.ok())
        .is_some()
    } else {
        false
    };

    Json(json!({
        "model": "BGE-small-en-v1.5",
        "kb_root": state.kb_root.as_str(),
        "seed": { "connected": seed_connected, "ip": seed_addr }
    }))
}

#[derive(Deserialize)]
struct AskBody {
    question: String,
    topic: String,
}

async fn ask(
    State(state): State<Arc<AppState>>,
    Json(body): Json<AskBody>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    // Use the same binary that's serving `learn ui` rather than whatever `learn`
    // is on $PATH — avoids picking up an older globally-installed version with
    // a different default embedder (which surfaces as DimensionMismatch at
    // query time against a KB built with the current version).
    let bin = std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("learn"));
    let output = tokio::process::Command::new(&bin)
        .args([
            "ask",
            &body.topic,
            &body.question,
            "--kb-root",
            state.kb_root.as_str(),
        ])
        .output()
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
        })?;

    if output.status.success() {
        let text = String::from_utf8_lossy(&output.stdout).to_string();
        let mut citations = Vec::new();
        let mut answer_lines = Vec::new();
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('·') || trimmed.starts_with("  ·") {
                citations.push(trimmed.trim_start_matches(['·', ' ']).to_string());
            } else {
                answer_lines.push(line);
            }
        }
        Ok(Json(json!({
            "answer": answer_lines.join("\n").trim(),
            "citations": citations
        })))
    } else {
        let err = String::from_utf8_lossy(&output.stderr).to_string();
        // "Abstain" is a legitimate answer — the model is saying "I don't have
        // enough evidence in the KB to answer that." Surface it as a normal
        // response with an `abstained: true` flag so the UI can render it
        // calmly (yellow), not as a hard red error. Hard errors (Dimension
        // Mismatch, IO, etc.) still get a 500.
        let lower = err.to_lowercase();
        if lower.contains("abstain") || lower.contains("insufficient evidence") {
            Ok(Json(json!({
                "answer": "(The model abstained — your KB has chunks but none were relevant enough to answer this question. Try rephrasing, or ingest more content on this topic.)",
                "citations": Vec::<String>::new(),
                "abstained": true
            })))
        } else {
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": err})),
            ))
        }
    }
}

#[derive(Deserialize)]
struct IngestQuery {
    source: String,
    #[serde(default)]
    topic: String,
    /// How many videos to pull from a channel/playlist/search. Defaults to 20
    /// (the wizard's "build me an expert" depth — gives ~10-20 hours of content).
    #[serde(default)]
    limit: Option<usize>,
}

async fn ingest_progress(
    State(state): State<Arc<AppState>>,
    Query(q): Query<IngestQuery>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let (tx, rx) = mpsc::channel::<String>(64);
    let kb_root = state.kb_root.to_string();
    let source = q.source.clone();
    let topic = q.topic.clone();

    tokio::spawn(async move {
        let send = |msg: &str, level: &str, pct: u8, done: bool| {
            let _ = tx.try_send(
                json!({"message": msg, "level": level, "progress": pct, "done": done}).to_string(),
            );
        };

        // Source may be a comma-separated list of channel/playlist URLs.
        // Each gets ingested sequentially with a per-source limit so that the
        // total volume across N sources stays in the 15–25 video range.
        let sources: Vec<String> = source
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        let n = sources.len().max(1);
        // Total target ~20 videos. Per-source = ceil(20 / n).
        let total_target = q.limit.unwrap_or(20);
        let per_source = ((total_target as f32) / (n as f32)).ceil() as usize;
        let per_source = per_source.max(3); // don't go below 3 per source
        let per_source_str = per_source.to_string();

        if n > 1 {
            send(
                &format!(
                    "Building expert from {n} sources · ~{per_source} videos each ({} total)…",
                    per_source * n
                ),
                "info",
                2,
                false,
            );
        } else {
            send(
                &format!("Starting ingest pipeline (target: {per_source} videos)…"),
                "info",
                2,
                false,
            );
        }

        let mut overall_pct = 5u8;
        let mut any_succeeded = false;
        let chunk_pct = if n > 1 { (90 / n) as u8 } else { 90 };

        for (idx, src) in sources.iter().enumerate() {
            if n > 1 {
                send(
                    &format!("─ source {}/{n} · {} ─", idx + 1, src),
                    "info",
                    overall_pct,
                    false,
                );
            }

            let mut args = vec![
                "ingest",
                src.as_str(),
                "--kb-root",
                kb_root.as_str(),
                "--limit",
                per_source_str.as_str(),
            ];
            if !topic.is_empty() {
                args.extend(["--topic", topic.as_str()]);
            }

            // Spawn the same binary running `learn ui` so dashboard and ingest
            // share one tool surface (avoids old-version-in-PATH skew).
            let learn_bin =
                std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("learn"));
            let mut child = match tokio::process::Command::new(&learn_bin)
                .args(&args)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::piped())
                .spawn()
            {
                Ok(c) => c,
                Err(e) => {
                    send(
                        &format!("Source {}: failed to start — {e}", idx + 1),
                        "warn",
                        overall_pct,
                        false,
                    );
                    continue;
                }
            };

            let start_pct = overall_pct;
            if let Some(stderr) = child.stderr.take() {
                use tokio::io::{AsyncBufReadExt, BufReader};
                let mut lines = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let level = if line.contains("error") || line.contains("Error") {
                        "warn"
                    } else if line.contains("Done") || line.contains("indexed") {
                        "success"
                    } else if line.contains("…")
                        || line.contains("Embedding")
                        || line.contains("Captioning")
                    {
                        "active"
                    } else {
                        "info"
                    };
                    overall_pct = (overall_pct + 2).min(start_pct + chunk_pct).min(95);
                    send(&line, level, overall_pct, false);
                }
            }

            match child.wait().await {
                Ok(s) if s.success() => {
                    any_succeeded = true;
                    if n > 1 {
                        send(
                            &format!("✓ source {}/{n} done.", idx + 1),
                            "success",
                            overall_pct,
                            false,
                        );
                    }
                }
                Ok(_) => send(
                    &format!("Source {}: finished with errors — continuing.", idx + 1),
                    "warn",
                    overall_pct,
                    false,
                ),
                Err(e) => send(
                    &format!("Source {}: process error — {e}", idx + 1),
                    "warn",
                    overall_pct,
                    false,
                ),
            }
            overall_pct = (start_pct + chunk_pct).min(95);
        }

        if any_succeeded {
            send("Ingest complete.", "success", 97, false);
            stream_seed_push(&send, &topic, &kb_root).await;
        } else {
            send(
                "All sources failed — check `learn doctor`.",
                "warn",
                100,
                true,
            );
        }
    });

    let stream = ReceiverStream::new(rx).map(|data| Ok(Event::default().data(data)));
    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("ping"),
    )
}

/// If a Seed is configured, push the topic to it and stream progress events.
async fn stream_seed_push(send: &impl Fn(&str, &str, u8, bool), topic: &str, kb_root: &str) {
    let seed_addr: Option<String> = std::env::var("LEARN_SEED_ADDRESS").ok().or_else(|| {
        let path = dirs::config_dir()
            .unwrap_or_default()
            .join("learn-rs/config.json");
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .and_then(|v| v["seed"]["address"].as_str().map(str::to_string))
            .filter(|s| !s.is_empty())
    });

    let Some(addr) = seed_addr else {
        send("Seed not configured — skipping push", "warn", 100, true);
        return;
    };

    // Derive the topic slug if not explicitly provided
    let topic_arg = if topic.is_empty() {
        // Without a slug we cannot push — auto_push in the CLI handles this case
        send(
            &format!("Stored locally · push with: learn push <topic> --seed {addr}"),
            "info",
            100,
            true,
        );
        return;
    } else {
        topic.to_string()
    };

    send(
        &format!("Pushing to Cognitum Seed {addr}…"),
        "active",
        98,
        false,
    );

    let result = tokio::process::Command::new("learn")
        .args(["push", &topic_arg, "--seed", &addr, "--kb-root", kb_root])
        .output()
        .await;

    match result {
        Ok(o) if o.status.success() => {
            send(&format!("Synced to Seed {addr}"), "success", 100, true);
        }
        Ok(o) => {
            let err = String::from_utf8_lossy(&o.stderr);
            send(
                &format!(
                    "Push failed: {}",
                    err.lines().next().unwrap_or("unknown error")
                ),
                "warn",
                100,
                true,
            );
        }
        Err(e) => {
            send(&format!("Push error: {e}"), "warn", 100, true);
        }
    }
}

// ── Seed discovery & config ──────────────────────────────────────────────────

#[derive(Deserialize, Default)]
struct DiscoverBody {
    #[serde(default = "default_discover_timeout")]
    timeout_secs: u64,
}
fn default_discover_timeout() -> u64 {
    3
}

async fn seed_discover(body: Option<Json<DiscoverBody>>) -> Json<Value> {
    let timeout = body.map(|Json(b)| b.timeout_secs).unwrap_or(3).clamp(1, 10);

    let task = tokio::task::spawn_blocking(move || -> Vec<String> {
        use mdns_sd::{ServiceDaemon, ServiceEvent};

        let Ok(daemon) = ServiceDaemon::new() else {
            return vec![];
        };
        let Ok(receiver) = daemon.browse("_cognitum._tcp.local.") else {
            return vec![];
        };

        let deadline = std::time::Instant::now() + Duration::from_secs(timeout);
        let mut found: Vec<String> = Vec::new();
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
        found
    });

    let addrs = task.await.unwrap_or_default();
    Json(json!({ "found": addrs }))
}

#[derive(Deserialize)]
struct ConfigureBody {
    address: String,
    #[serde(default)]
    auto_push: bool,
}

async fn seed_configure(
    Json(body): Json<ConfigureBody>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let path = dirs::config_dir()
        .unwrap_or_default()
        .join("learn-rs/config.json");

    if let Some(dir) = path.parent() {
        if let Err(e) = std::fs::create_dir_all(dir) {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            ));
        }
    }

    // Preserve any unknown fields by reading-modify-writing.
    let mut current: Value = std::fs::read(&path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_else(|| json!({}));
    let seed = current
        .as_object_mut()
        .expect("just created as object")
        .entry("seed")
        .or_insert_with(|| json!({}));
    seed["address"] = json!(body.address);
    seed["auto_push"] = json!(body.auto_push);

    let bytes = serde_json::to_vec_pretty(&current).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
    })?;
    std::fs::write(&path, &bytes).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
    })?;

    Ok(Json(json!({"ok": true, "path": path.to_string_lossy()})))
}
