//! `learn doctor` — first-60-seconds environment diagnostic.
//!
//! Each check is a small function that returns a [`Check`] result.
//! All checks run sequentially; network checks carry a 3-second timeout.
//! Exit code: 0 when all required checks pass, 1 otherwise.

use colored::Colorize;
use std::path::{Path, PathBuf};
use std::time::Duration;

// ── Types ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum Status {
    /// Required and present.
    Pass,
    /// Required but absent — blocks exit 0.
    Fail,
    /// Not required; absent is acceptable.
    Warn,
    /// Explicitly expected to be absent (e.g. AIMDS).
    ExpectedFail,
}

#[derive(Debug, Clone)]
pub struct Check {
    pub name: String,
    pub status: Status,
    pub detail: String,
}

impl Check {
    fn pass(name: &str, detail: impl Into<String>) -> Self {
        Self {
            name: name.to_owned(),
            status: Status::Pass,
            detail: detail.into(),
        }
    }
    fn fail(name: &str, detail: impl Into<String>) -> Self {
        Self {
            name: name.to_owned(),
            status: Status::Fail,
            detail: detail.into(),
        }
    }
    fn warn(name: &str, detail: impl Into<String>) -> Self {
        Self {
            name: name.to_owned(),
            status: Status::Warn,
            detail: detail.into(),
        }
    }
    fn expected_fail(name: &str, detail: impl Into<String>) -> Self {
        Self {
            name: name.to_owned(),
            status: Status::ExpectedFail,
            detail: detail.into(),
        }
    }
}

// ── Dependency checks ─────────────────────────────────────────────────────────

/// Resolve a binary name to its path using PATH-walking.
/// Abstracted as a closure parameter so tests can inject a mock.
pub fn which_bin(name: &str) -> Option<PathBuf> {
    which::which(name).ok()
}

/// Check for a binary; read version via `--version`.
pub fn check_bin(
    name: &str,
    which_fn: impl Fn(&str) -> Option<PathBuf>,
    version_flag: &str,
) -> Check {
    match which_fn(name) {
        None => Check::fail(name, format!("not found — install {name}")),
        Some(path) => {
            let ver = std::process::Command::new(&path)
                .arg(version_flag)
                .output()
                .ok()
                .and_then(|o| {
                    let raw = String::from_utf8_lossy(&o.stdout).to_string();
                    let raw = if raw.trim().is_empty() {
                        String::from_utf8_lossy(&o.stderr).to_string()
                    } else {
                        raw
                    };
                    raw.lines().next().map(|l| l.trim().to_owned())
                })
                .unwrap_or_default();
            Check::pass(name, format!("{} ({})", path.display(), ver))
        }
    }
}

/// Check that an env var is set to a non-empty value.
pub fn check_env(name: &str, failure_hint: &str, env_fn: impl Fn(&str) -> Option<String>) -> Check {
    match env_fn(name) {
        Some(v) if !v.is_empty() => Check::pass(name, "set"),
        _ => Check::fail(name, failure_hint.to_owned()),
    }
}

/// Check for a binary that is optional (warning if absent, not a hard fail).
pub fn check_optional_bin(
    name: &str,
    absent_hint: &str,
    which_fn: impl Fn(&str) -> Option<PathBuf>,
) -> Check {
    match which_fn(name) {
        None => Check::expected_fail(name, absent_hint.to_owned()),
        Some(path) => Check::pass(name, path.display().to_string()),
    }
}

// ── Storage checks ────────────────────────────────────────────────────────────

/// Check a directory: must exist and be writable, report vector count + size.
pub fn check_kb_root(
    root: &Path,
    exists_fn: impl Fn(&Path) -> bool,
    read_dir_fn: impl Fn(&Path) -> Vec<PathBuf>,
    writable_fn: impl Fn(&Path) -> bool,
) -> Check {
    if !exists_fn(root) {
        return Check::fail(
            "KB root",
            format!(
                "{} not found — run `learn ingest` to create it",
                root.display()
            ),
        );
    }
    if !writable_fn(root) {
        return Check::fail("KB root", format!("{} is not writable", root.display()));
    }
    let rvf_files = read_dir_fn(root);
    let topics = rvf_files.len();
    let size_mb = dir_size_mb(root);
    Check::pass(
        "KB root",
        format!(
            "{} (writable, {} topic{}, {:.1} MB)",
            root.display(),
            topics,
            if topics == 1 { "" } else { "s" },
            size_mb
        ),
    )
}

/// Check a file path exists.
pub fn check_file(
    label: &str,
    path: &Path,
    absent_hint: &str,
    exists_fn: impl Fn(&Path) -> bool,
) -> Check {
    if exists_fn(path) {
        Check::pass(label, path.display().to_string())
    } else {
        Check::fail(label, format!("{} — {}", path.display(), absent_hint))
    }
}

/// Check a directory exists (optional — warn if absent).
pub fn check_optional_dir(
    label: &str,
    path: &Path,
    absent_hint: &str,
    exists_fn: impl Fn(&Path) -> bool,
) -> Check {
    if exists_fn(path) {
        let count = count_subdirs(path);
        let detail = if count > 0 {
            format!(
                "{} ({} topic{} with feedback persisted)",
                path.display(),
                count,
                if count == 1 { "" } else { "s" }
            )
        } else {
            path.display().to_string()
        };
        Check::pass(label, detail)
    } else {
        Check::warn(label, format!("{} — {}", path.display(), absent_hint))
    }
}

/// Check that a model cache directory exists and report its size.
pub fn check_model_cache(label: &str, path: &Path, exists_fn: impl Fn(&Path) -> bool) -> Check {
    if !exists_fn(path) {
        return Check::warn(
            label,
            format!(
                "{} not found — will be downloaded on first use",
                path.display()
            ),
        );
    }
    let size_mb = dir_size_mb(path);
    Check::pass(label, format!("{} ({:.1} MB)", path.display(), size_mb))
}

// ── Network checks ────────────────────────────────────────────────────────────

/// HEAD-request a URL; pass/fail based on whether the server is reachable.
/// `fetch_fn` is injected so tests can mock it without real HTTP.
pub fn check_url(label: &str, url: &str, reachable: bool) -> Check {
    if reachable {
        Check::pass(label, "reachable")
    } else {
        Check::warn(label, format!("{url} unreachable (offline or blocked)"))
    }
}

/// Perform a HEAD request with a 3-second timeout.  Returns true if we get
/// any HTTP response (even 4xx — the server is reachable).
pub async fn probe_url(url: &str) -> bool {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()
        .unwrap_or_default();
    client.head(url).send().await.is_ok()
}

// ── Version check ─────────────────────────────────────────────────────────────

/// Compare binary version to GitHub latest release tag.
pub fn check_version(binary_version: &str, github_latest: Option<&str>, repo_url: &str) -> Check {
    let current = format!("v{binary_version}");
    match github_latest {
        None => Check::warn(
            "version",
            format!("{current} (GitHub unreachable — cannot verify)"),
        ),
        Some(latest) if latest == current => {
            Check::pass("version", format!("{current} — up to date ({repo_url})"))
        }
        Some(latest) => Check::warn(
            "version",
            format!(
                "{current} installed, {latest} available — `cargo install --path crates/learn-cli`"
            ),
        ),
    }
}

// ── Config summary ────────────────────────────────────────────────────────────

pub struct ConfigSummary {
    pub kb_root: String,
    pub synth_local: bool,
    pub aimds_required: bool,
}

pub fn build_config_summary(
    kb_root: &Path,
    env_fn: &dyn Fn(&str) -> Option<String>,
) -> ConfigSummary {
    let synth_local = env_fn("LEARN_SYNTH_LOCAL")
        .map(|v| v == "1")
        .unwrap_or(false);
    let aimds_required = env_fn("LEARN_AIMDS_REQUIRED")
        .map(|v| v == "1")
        .unwrap_or(false);
    ConfigSummary {
        kb_root: kb_root.display().to_string(),
        synth_local,
        aimds_required,
    }
}

// ── Display helpers ───────────────────────────────────────────────────────────

fn symbol(s: &Status) -> String {
    match s {
        Status::Pass => "✓".green().to_string(),
        Status::Fail => "✗".red().to_string(),
        Status::Warn => "⚠".yellow().to_string(),
        Status::ExpectedFail => "⚠".yellow().to_string(),
    }
}

fn print_section(title: &str) {
    println!("\n{}", title.bold());
}

fn print_check(c: &Check) {
    println!("  {} {:<22} {}", symbol(&c.status), c.name, c.detail);
}

// ── Orchestrator ──────────────────────────────────────────────────────────────

/// Run all doctor checks and print the report.  Returns `true` when all
/// required checks pass (exit 0 condition).
pub async fn run_doctor(kb_root: &Path) -> bool {
    println!("{}", "Learn-RV — Doctor (v0.1.3)".bold());

    // -- DEPENDENCIES --
    print_section("DEPENDENCIES");

    let checks_dep: Vec<Check> = vec![
        check_bin("ffmpeg", which_bin, "-version"),
        check_bin("yt-dlp", which_bin, "--version"),
        check_bin("git", which_bin, "--version"),
        check_env(
            "ANTHROPIC_API_KEY",
            "not set — `learn ask` will fail until you `export ANTHROPIC_API_KEY=...`",
            |k| std::env::var(k).ok(),
        ),
        check_optional_bin(
            "AIMDS binary",
            "@ruflo/aidefence not found — outbound safety scan will be skipped (acceptable)",
            |_| which_bin("aidefence"),
        ),
    ];
    for c in &checks_dep {
        print_check(c);
    }

    // -- STORAGE --
    print_section("STORAGE");

    let adapter_cache = dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from(".cache"))
        .join("learn-rs")
        .join("adapters");
    let model_cache = dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from(".cache"))
        .join("learn-rs")
        .join("models")
        .join("bge-large-en-v15");
    let skill_file = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".claude")
        .join("skills")
        .join("learn-rv")
        .join("SKILL.md");

    let checks_storage: Vec<Check> = vec![
        check_kb_root(
            kb_root,
            |p| p.exists(),
            |p| {
                p.read_dir()
                    .map(|rd| {
                        rd.flatten()
                            .filter(|e| e.path().extension().map(|x| x == "rvf").unwrap_or(false))
                            .map(|e| e.path())
                            .collect()
                    })
                    .unwrap_or_default()
            },
            is_writable,
        ),
        check_file(
            "Skill folder",
            &skill_file,
            "run `learn install-skill` or check ~/.claude/skills/learn-rv/",
            |p| p.exists(),
        ),
        check_optional_dir(
            "Adapter cache",
            &adapter_cache,
            "no feedback persisted yet (normal on first use)",
            |p| p.exists(),
        ),
        check_model_cache("Model cache", &model_cache, |p| p.exists()),
    ];
    for c in &checks_storage {
        print_check(c);
    }

    // -- NETWORK --
    print_section("NETWORK");

    let yt_ok = probe_url("https://www.youtube.com").await;
    let api_ok = probe_url("https://api.anthropic.com").await;

    let checks_net: Vec<Check> = vec![
        check_url("youtube.com", "https://www.youtube.com", yt_ok),
        check_url("api.anthropic.com", "https://api.anthropic.com", api_ok),
    ];
    for c in &checks_net {
        print_check(c);
    }

    // -- VERSION --
    print_section("VERSION");

    let binary_version = env!("CARGO_PKG_VERSION");
    let repo = "https://github.com/stuinfla/learner-rv";
    let github_tag = fetch_github_latest(repo).await;
    let ver_check = check_version(
        binary_version,
        github_tag.as_deref(),
        &format!("{repo}/releases/tag/v{binary_version}"),
    );

    println!(
        "  {} {:<22} {}",
        symbol(&Status::Pass),
        "binary",
        binary_version
    );
    println!(
        "  {} {:<22} {}",
        symbol(&ver_check.status),
        "GitHub latest",
        ver_check.detail
    );

    // -- CONFIG --
    print_section("CONFIG");

    let cfg = build_config_summary(kb_root, &|k| std::env::var(k).ok());
    println!(
        "  {} {:<22} {} (override with --kb-root or LEARN_KB_ROOT)",
        symbol(&Status::Pass),
        "KB root",
        cfg.kb_root
    );
    let synth_label = if cfg.synth_local {
        "LEARN_SYNTH_LOCAL=1 (using ruvllm)"
    } else {
        "LEARN_SYNTH_LOCAL=0 (using Anthropic; set =1 for ruvllm)"
    };
    println!(
        "  {} {:<22} {}",
        symbol(&Status::Pass),
        "Sovereign LLM",
        synth_label
    );
    let aimds_label = if cfg.aimds_required {
        "LEARN_AIMDS_REQUIRED=1 (hard-fail when AIMDS unavailable)"
    } else {
        "LEARN_AIMDS_REQUIRED=0 (set =1 to fail when AIMDS unavailable)"
    };
    println!(
        "  {} {:<22} {}",
        symbol(&Status::Pass),
        "AIMDS hard-fail",
        aimds_label
    );

    // -- SUMMARY --
    let all_checks: Vec<&Check> = checks_dep
        .iter()
        .chain(checks_storage.iter())
        .chain(checks_net.iter())
        .collect();
    let passes = all_checks
        .iter()
        .filter(|c| c.status == Status::Pass)
        .count();
    let fails = all_checks
        .iter()
        .filter(|c| c.status == Status::Fail)
        .count();
    let warns = all_checks
        .iter()
        .filter(|c| c.status == Status::Warn)
        .count();
    let expected = all_checks
        .iter()
        .filter(|c| c.status == Status::ExpectedFail)
        .count();

    println!();
    if fails == 0 {
        let summary = format!(
            "OVERALL: {} READY ({} checks pass{}{})",
            "✓".green(),
            passes,
            if warns > 0 {
                format!(", {warns} warning{}", if warns == 1 { "" } else { "s" })
            } else {
                String::new()
            },
            if expected > 0 {
                format!(
                    ", {expected} expected-fail{}",
                    if expected == 1 { "" } else { "s" }
                )
            } else {
                String::new()
            },
        );
        println!("{}", summary.bold());
    } else {
        let summary = format!(
            "OVERALL: {} NOT READY ({} check{} failed, {} pass{}{})",
            "✗".red(),
            fails,
            if fails == 1 { "" } else { "s" },
            passes,
            if warns > 0 {
                format!(", {warns} warning{}", if warns == 1 { "" } else { "s" })
            } else {
                String::new()
            },
            if expected > 0 {
                format!(
                    ", {expected} expected-fail{}",
                    if expected == 1 { "" } else { "s" }
                )
            } else {
                String::new()
            },
        );
        println!("{}", summary.bold());
    }

    fails == 0
}

// ── Private helpers ───────────────────────────────────────────────────────────

fn dir_size_mb(path: &Path) -> f64 {
    fn walk(p: &Path) -> u64 {
        let Ok(rd) = std::fs::read_dir(p) else {
            return 0;
        };
        rd.flatten()
            .map(|e| {
                let ep = e.path();
                if ep.is_dir() {
                    walk(&ep)
                } else {
                    ep.metadata().map(|m| m.len()).unwrap_or(0)
                }
            })
            .sum()
    }
    walk(path) as f64 / (1024.0 * 1024.0)
}

fn count_subdirs(path: &Path) -> usize {
    std::fs::read_dir(path)
        .map(|rd| rd.flatten().filter(|e| e.path().is_dir()).count())
        .unwrap_or(0)
}

fn is_writable(path: &Path) -> bool {
    let test_file = path.join(".learn_doctor_probe");
    let ok = std::fs::write(&test_file, b"").is_ok();
    let _ = std::fs::remove_file(&test_file);
    ok
}

async fn fetch_github_latest(repo_base: &str) -> Option<String> {
    // Derive api URL from the repo HTML URL.
    // e.g. "https://github.com/stuinfla/learner-rv" →
    //      "https://api.github.com/repos/stuinfla/learner-rv/releases/latest"
    let parts: Vec<&str> = repo_base.trim_end_matches('/').rsplitn(3, '/').collect();
    if parts.len() < 2 {
        return None;
    }
    let api_url = format!(
        "https://api.github.com/repos/{}/{}/releases/latest",
        parts[1], parts[0]
    );
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .user_agent("learn-rv-doctor/0.1.3")
        .build()
        .ok()?;
    let resp = client.get(&api_url).send().await.ok()?;
    let json: serde_json::Value = resp.json().await.ok()?;
    json["tag_name"].as_str().map(|s| s.to_owned())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // --- check_bin ---

    #[test]
    fn check_bin_found_returns_pass() {
        let result = check_bin(
            "mybin",
            |_| Some(PathBuf::from("/usr/bin/mybin")),
            "--version",
        );
        assert_eq!(result.status, Status::Pass);
        assert!(result.detail.contains("/usr/bin/mybin"));
    }

    #[test]
    fn check_bin_not_found_returns_fail() {
        let result = check_bin("ghost", |_| None, "--version");
        assert_eq!(result.status, Status::Fail);
    }

    // --- check_env ---

    #[test]
    fn check_env_set_returns_pass() {
        let result = check_env("MY_KEY", "not set", |k| {
            if k == "MY_KEY" {
                Some("abc123".to_owned())
            } else {
                None
            }
        });
        assert_eq!(result.status, Status::Pass);
    }

    #[test]
    fn check_env_missing_returns_fail() {
        let result = check_env("MISSING_KEY", "please set it", |_| None);
        assert_eq!(result.status, Status::Fail);
        assert!(result.detail.contains("please set it"));
    }

    #[test]
    fn check_env_empty_string_returns_fail() {
        let result = check_env("EMPTY_KEY", "empty", |_| Some(String::new()));
        assert_eq!(result.status, Status::Fail);
    }

    // --- check_optional_bin ---

    #[test]
    fn check_optional_bin_absent_is_expected_fail() {
        let result = check_optional_bin("aidefence", "not found — skipping", |_| None);
        assert_eq!(result.status, Status::ExpectedFail);
    }

    #[test]
    fn check_optional_bin_present_is_pass() {
        let result = check_optional_bin("aidefence", "not found", |_| {
            Some(PathBuf::from("/usr/local/bin/aidefence"))
        });
        assert_eq!(result.status, Status::Pass);
    }

    // --- check_kb_root ---

    #[test]
    fn check_kb_root_missing_returns_fail() {
        let result = check_kb_root(Path::new("/no/such/path"), |_| false, |_| vec![], |_| false);
        assert_eq!(result.status, Status::Fail);
    }

    #[test]
    fn check_kb_root_not_writable_returns_fail() {
        let result = check_kb_root(Path::new("/some/path"), |_| true, |_| vec![], |_| false);
        assert_eq!(result.status, Status::Fail);
    }

    #[test]
    fn check_kb_root_ok_reports_topic_count() {
        let result = check_kb_root(
            Path::new("/home/user/Docs/KB"),
            |_| true,
            |_| {
                vec![
                    PathBuf::from("french-cooking.rvf"),
                    PathBuf::from("guitar.rvf"),
                    PathBuf::from("rust.rvf"),
                ]
            },
            |_| true,
        );
        assert_eq!(result.status, Status::Pass);
        assert!(
            result.detail.contains("3 topics"),
            "detail: {}",
            result.detail
        );
    }

    // --- check_url ---

    #[test]
    fn check_url_reachable_is_pass() {
        let c = check_url("youtube.com", "https://youtube.com", true);
        assert_eq!(c.status, Status::Pass);
    }

    #[test]
    fn check_url_unreachable_is_warn() {
        let c = check_url("youtube.com", "https://youtube.com", false);
        assert_eq!(c.status, Status::Warn);
    }

    // --- check_version ---

    #[test]
    fn check_version_up_to_date_is_pass() {
        let c = check_version("0.1.3", Some("v0.1.3"), "https://github.com/x/y");
        assert_eq!(c.status, Status::Pass);
        assert!(c.detail.contains("up to date"));
    }

    #[test]
    fn check_version_outdated_is_warn() {
        let c = check_version("0.1.2", Some("v0.1.3"), "https://github.com/x/y");
        assert_eq!(c.status, Status::Warn);
        assert!(c.detail.contains("v0.1.3"));
    }

    #[test]
    fn check_version_github_unreachable_is_warn() {
        let c = check_version("0.1.3", None, "https://github.com/x/y");
        assert_eq!(c.status, Status::Warn);
        assert!(c.detail.contains("unreachable"));
    }

    // --- build_config_summary ---

    #[test]
    fn config_summary_defaults_to_anthropic() {
        let cfg = build_config_summary(Path::new("/tmp/kb"), &|_| None);
        assert!(!cfg.synth_local);
        assert!(!cfg.aimds_required);
    }

    #[test]
    fn config_summary_reads_env() {
        let cfg = build_config_summary(Path::new("/tmp/kb"), &|k| match k {
            "LEARN_SYNTH_LOCAL" => Some("1".to_owned()),
            "LEARN_AIMDS_REQUIRED" => Some("1".to_owned()),
            _ => None,
        });
        assert!(cfg.synth_local);
        assert!(cfg.aimds_required);
    }
}
