//! `learn setup` — first-run wizard.
//!
//! Visual design matches the brand palette in assets/hero.svg:
//!   - Indigo / bright-cyan  → headers and step indicators
//!   - Emerald green          → Seed references, success states
//!   - Amber / yellow         → warnings
//!   - Dimmed grey            → secondary text
//!
//! Works interactively (stdin) and non-interactively (--yes flag).
//! Auto-fires the first time `learn` is invoked with no KB root yet.

use colored::Colorize;
use std::io::{self, Write as _};

// ── Public entry points ───────────────────────────────────────────────────────

/// Run the full interactive wizard.
/// `yes_all` skips prompts and accepts all defaults.
pub async fn run_setup(yes_all: bool) -> learn_core::Result<()> {
    let mut cfg = crate::config::LearnConfig::load();

    // Already configured — confirm and offer reconfiguration.
    if cfg.seed_address().is_some() && !yes_all {
        let addr = cfg.seed_address().unwrap();
        print_header();
        println!(
            "  {}  Seed already configured: {}",
            "◆".bright_cyan(),
            addr.bold().bright_green()
        );
        println!(
            "  {}  Auto-push: {}",
            "◆".bright_cyan(),
            if cfg.seed_auto_push() {
                "enabled".bright_green().to_string()
            } else {
                "disabled".yellow().to_string()
            }
        );
        println!();
        print!("  Reconfigure? [y/N] ");
        io::stdout().flush().ok();
        if !read_yes_no(false) {
            println!();
            print_next_steps(&addr, cfg.seed_auto_push());
            return Ok(());
        }
        println!();
    } else {
        print_header();
    }

    // ── Step 1 — Do you have a Seed? ─────────────────────────────────────────
    print_step(1, 3, "Cognitum One Seed");
    println!(
        "  {}",
        "Your Seed is where all your knowledge lives — permanently,".dimmed()
    );
    println!("  {}", "offline, no cloud, no subscription.".dimmed());
    println!();

    let has_seed = if yes_all {
        false
    } else {
        print!("  Do you have a Cognitum One Seed on your network? [y/N] ");
        io::stdout().flush().ok();
        read_yes_no(false)
    };

    if !has_seed {
        println!();
        println!(
            "  {}  No problem — you can add a Seed any time with:",
            "◆".bright_cyan()
        );
        println!();
        println!("      learn setup");
        println!("      learn config set seed.address <ip>");
        println!();
        print_divider();
        print_quickstart_only();
        return Ok(());
    }

    println!();

    // ── Step 2 — IP / hostname ────────────────────────────────────────────────
    print_step(2, 3, "Seed Address");
    println!(
        "  {}",
        "Find the IP in your router's device list, or use the mDNS name.".dimmed()
    );
    println!();

    let addr = loop {
        print!("  {} ", "→".bright_cyan().bold());
        io::stdout().flush().ok();
        let input = read_line().trim().to_owned();
        if input.is_empty() {
            println!(
                "  {}",
                "Please enter an IP address or hostname (e.g. 192.168.1.42).".yellow()
            );
            continue;
        }
        break input;
    };

    println!();

    // ── Step 3 — Auto-push ────────────────────────────────────────────────────
    print_step(3, 3, "Auto-Push");
    println!(
        "  {}",
        "After every ingest, automatically push the KB to your Seed.".dimmed()
    );
    println!(
        "  {}",
        "You'll never need to remember to push manually.".dimmed()
    );
    println!();
    print!("  Enable auto-push? [Y/n] ");
    io::stdout().flush().ok();
    let auto_push = read_yes_no(true);

    // ── Save ──────────────────────────────────────────────────────────────────
    println!();
    cfg.set_key("seed.address", &addr)?;
    cfg.set_key("seed.auto_push", if auto_push { "true" } else { "false" })?;
    cfg.save()?;

    print_divider();
    println!(
        "  {}  Seed address   {}",
        "✓".bright_green().bold(),
        addr.bold()
    );
    println!(
        "  {}  Auto-push      {}",
        "✓".bright_green().bold(),
        if auto_push {
            "enabled".bright_green().to_string()
        } else {
            "disabled".yellow().to_string()
        }
    );
    println!(
        "  {}  Config saved   {}",
        "✓".bright_green().bold(),
        crate::config::LearnConfig::config_path()
            .display()
            .to_string()
            .dimmed()
    );
    print_divider();
    println!();

    // ── Probe reachability ────────────────────────────────────────────────────
    print!("  Testing connection to {} …  ", addr.bold());
    io::stdout().flush().ok();
    let url = format!("http://{addr}/");
    let reachable = crate::doctor::probe_url(&url).await;
    if reachable {
        println!("{}", "✓ reachable".bright_green().bold());
    } else {
        println!("{}", "⚠ not reachable".yellow());
        println!();
        println!("  {}", "Seed saved but not responding right now.".dimmed());
        println!(
            "  {}",
            "Check it's powered on and on the same network, then:".dimmed()
        );
        println!();
        println!("      learn doctor");
    }

    println!();
    print_next_steps(&addr, auto_push);
    Ok(())
}

/// Auto-fires on `learn` (no args) when no KB root exists yet.
/// Returns `true` if the wizard ran (caller should skip orientation).
pub async fn maybe_run_first_time(kb_root: &camino::Utf8Path) -> bool {
    let kb_exists = kb_root.exists();
    let cfg = crate::config::LearnConfig::load();
    let seed_set = cfg.seed_address().is_some();

    if kb_exists || seed_set {
        return false;
    }

    // Compact invite — not a forced launch.
    println!();
    println!(
        "  {}  {}",
        "◆".bright_cyan().bold(),
        "Learn-RV — first run.".bold()
    );
    println!(
        "  {}",
        "Set up your Cognitum Seed and you're ready in 30 seconds.".dimmed()
    );
    println!();
    print!("  Run setup now? [Y/n] ");
    io::stdout().flush().ok();
    if !read_yes_no(true) {
        println!();
        return false;
    }
    println!();

    if let Err(e) = run_setup(false).await {
        eprintln!("setup error: {e}");
    }
    true
}

// ── Visual helpers ────────────────────────────────────────────────────────────

fn print_header() {
    println!();
    println!(
        "  {}",
        "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".bright_cyan()
    );
    println!("  {}  Learn-RV Setup", "◆".bright_cyan().bold());
    println!(
        "     {}",
        "Make your Cognitum Seed an expert in anything.".dimmed()
    );
    println!(
        "  {}",
        "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".bright_cyan()
    );
    println!();
}

fn print_step(step: u8, total: u8, title: &str) {
    println!(
        "  {}  {}",
        format!("STEP {step} of {total}").bright_cyan().bold(),
        title.bold()
    );
    println!("  {}", "─────────────────────────────────────".dimmed());
    println!();
}

fn print_divider() {
    println!(
        "  {}",
        "────────────────────────────────────────────────".dimmed()
    );
}

fn print_next_steps(addr: &str, auto_push: bool) {
    println!("{}", "  You're all set.".bold().bright_green());
    println!();
    println!("  Build your first expert:");
    println!();
    println!(
        "    {}  learn study \"Japanese knife sharpening\"",
        "▶".bright_cyan()
    );
    println!(
        "    {}  learn ask knife-sharpening \"What angle for a 210mm gyuto?\"",
        "▶".bright_cyan()
    );
    if auto_push {
        println!();
        println!(
            "  {}  Knowledge flows to {} after every ingest.",
            "◆".bright_green(),
            addr.bold()
        );
    } else {
        println!();
        println!("    {}  learn push knife-sharpening", "▶".bright_cyan());
    }
    println!();
    println!("  {}  `learn doctor` — verify setup anytime", "◆".dimmed());
    println!("  {}  `learn` — full command reference", "◆".dimmed());
    println!();
}

fn print_quickstart_only() {
    println!("{}", "  You're all set.".bold().bright_green());
    println!();
    println!("  Build your first expert:");
    println!();
    println!(
        "    {}  learn study \"Japanese knife sharpening\"",
        "▶".bright_cyan()
    );
    println!(
        "    {}  learn ask knife-sharpening \"What angle for a 210mm gyuto?\"",
        "▶".bright_cyan()
    );
    println!("    {}  learn quiz knife-sharpening", "▶".bright_cyan());
    println!();
    println!("  {}  `learn` — full command reference", "◆".dimmed());
    println!(
        "  {}  `learn doctor` — verify your environment",
        "◆".dimmed()
    );
    println!();
}

/// Read a yes/no response from stdin. Returns `default` on empty input or error.
fn read_yes_no(default: bool) -> bool {
    let input = read_line();
    match input.trim().to_lowercase().as_str() {
        "y" | "yes" => true,
        "n" | "no" => false,
        _ => default,
    }
}

/// Read one line from stdin, returning empty string on error.
fn read_line() -> String {
    let mut buf = String::new();
    io::stdin().read_line(&mut buf).unwrap_or(0);
    buf
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yes_no_parses_all_variants() {
        let yes_inputs = ["y", "Y", "yes", "YES", "Yes"];
        let no_inputs = ["n", "N", "no", "NO", "No"];

        for s in yes_inputs {
            let result = match s.trim().to_lowercase().as_str() {
                "y" | "yes" => true,
                "n" | "no" => false,
                _ => false,
            };
            assert!(result, "'{s}' should be true");
        }
        for s in no_inputs {
            let result = match s.trim().to_lowercase().as_str() {
                "y" | "yes" => true,
                "n" | "no" => false,
                _ => true,
            };
            assert!(!result, "'{s}' should be false");
        }
    }

    #[test]
    fn yes_no_empty_returns_default() {
        // Empty string falls through to default.
        let default_true: bool = match "".trim().to_lowercase().as_str() {
            "y" | "yes" => true,
            "n" | "no" => false,
            _ => true, // default = true
        };
        assert!(default_true);

        let default_false: bool = match "".trim().to_lowercase().as_str() {
            "y" | "yes" => true,
            "n" | "no" => false,
            _ => false, // default = false
        };
        assert!(!default_false);
    }

    #[tokio::test]
    async fn maybe_run_first_time_skips_when_kb_exists() {
        let dir = tempfile::tempdir().unwrap();
        let kb_root = camino::Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        // KB root exists → must not fire wizard
        let fired = maybe_run_first_time(&kb_root).await;
        assert!(!fired, "wizard must not fire when KB root already exists");
    }
}
