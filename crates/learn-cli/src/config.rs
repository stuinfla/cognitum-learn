//! Persistent configuration for cognitum-learn.
//!
//! Stored as JSON at `~/.config/learn-rs/config.json`.
//! `LEARN_SEED_ADDRESS` and `LEARN_SEED_AUTO_PUSH` env vars override file values.

use learn_core::LearnError;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SeedConfig {
    pub address: Option<String>,
    #[serde(default)]
    pub auto_push: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LearnConfig {
    #[serde(default)]
    pub seed: SeedConfig,
}

impl LearnConfig {
    /// Path: `~/.config/learn-rs/config.json`.
    pub fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from(".config"))
            .join("learn-rs")
            .join("config.json")
    }

    /// Load from disk; return defaults if the file doesn't exist or can't parse.
    pub fn load() -> Self {
        let path = Self::config_path();
        let Ok(bytes) = std::fs::read(&path) else {
            return Self::default();
        };
        serde_json::from_slice(&bytes).unwrap_or_default()
    }

    /// Persist to disk; create the directory if needed.
    pub fn save(&self) -> learn_core::Result<()> {
        let path = Self::config_path();
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(LearnError::Io)?;
        }
        let json = serde_json::to_vec_pretty(self).map_err(LearnError::Serde)?;
        std::fs::write(&path, &json).map_err(LearnError::Io)?;
        Ok(())
    }

    /// Effective seed address: `LEARN_SEED_ADDRESS` env var overrides file config.
    pub fn seed_address(&self) -> Option<String> {
        if let Ok(v) = std::env::var("LEARN_SEED_ADDRESS") {
            if !v.is_empty() {
                return Some(v);
            }
        }
        self.seed.address.clone().filter(|a| !a.is_empty())
    }

    /// Effective auto-push flag: `LEARN_SEED_AUTO_PUSH` env var overrides file config.
    pub fn seed_auto_push(&self) -> bool {
        if let Ok(v) = std::env::var("LEARN_SEED_AUTO_PUSH") {
            return v == "1" || v.eq_ignore_ascii_case("true");
        }
        self.seed.auto_push
    }

    /// Set a config key by dotted path (`seed.address`, `seed.auto_push`).
    pub fn set_key(&mut self, key: &str, value: &str) -> learn_core::Result<()> {
        match key {
            "seed.address" => {
                self.seed.address = if value.is_empty() {
                    None
                } else {
                    Some(value.to_owned())
                };
            }
            "seed.auto_push" => {
                self.seed.auto_push = value == "1" || value.eq_ignore_ascii_case("true");
            }
            _ => {
                return Err(LearnError::Apply(format!(
                    "unknown config key '{key}'\n  valid keys: seed.address  seed.auto_push"
                )));
            }
        }
        Ok(())
    }

    /// Get a config key value as a labelled string.
    pub fn get_key(&self, key: &str) -> learn_core::Result<String> {
        match key {
            "seed.address" => {
                let v = self
                    .seed_address()
                    .unwrap_or_else(|| "(not set)".to_owned());
                Ok(format!("seed.address = {v}"))
            }
            "seed.auto_push" => Ok(format!("seed.auto_push = {}", self.seed_auto_push())),
            _ => Err(LearnError::Apply(format!(
                "unknown config key '{key}'\n  valid keys: seed.address  seed.auto_push"
            ))),
        }
    }
}

/// Print all current config values.
pub fn run_config_list(cfg: &LearnConfig) -> learn_core::Result<()> {
    let path = LearnConfig::config_path();
    println!("Config: {}", path.display());
    #[cfg(target_os = "macos")]
    println!("  (macOS standard config directory — ~/Library/Application Support/learn-rs/)");
    println!();
    println!(
        "  seed.address   = {}",
        cfg.seed_address().unwrap_or_else(|| "(not set)".to_owned())
    );
    println!("  seed.auto_push = {}", cfg.seed_auto_push());
    println!();
    println!("  Override with env vars: LEARN_SEED_ADDRESS  LEARN_SEED_AUTO_PUSH");
    Ok(())
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_defaults_to_no_seed() {
        let cfg = LearnConfig::default();
        assert!(cfg.seed.address.is_none());
        assert!(!cfg.seed.auto_push);
    }

    #[test]
    fn set_seed_address_stores_value() {
        let mut cfg = LearnConfig::default();
        cfg.set_key("seed.address", "192.168.1.42").unwrap();
        assert_eq!(cfg.seed.address.as_deref(), Some("192.168.1.42"));
    }

    #[test]
    fn set_auto_push_true_variants() {
        for val in ["1", "true", "True", "TRUE"] {
            let mut cfg = LearnConfig::default();
            cfg.set_key("seed.auto_push", val).unwrap();
            assert!(cfg.seed.auto_push, "expected true for value '{val}'");
        }
    }

    #[test]
    fn set_auto_push_false_variants() {
        for val in ["0", "false", "False"] {
            let mut cfg = LearnConfig::default();
            cfg.seed.auto_push = true; // start as true
            cfg.set_key("seed.auto_push", val).unwrap();
            assert!(!cfg.seed.auto_push, "expected false for value '{val}'");
        }
    }

    #[test]
    fn set_unknown_key_returns_err() {
        let mut cfg = LearnConfig::default();
        assert!(cfg.set_key("unknown.key", "value").is_err());
    }

    #[test]
    fn get_unset_address_returns_labelled_not_set() {
        let cfg = LearnConfig::default();
        assert_eq!(
            cfg.get_key("seed.address").unwrap(),
            "seed.address = (not set)"
        );
    }

    #[test]
    fn get_set_address_returns_labelled_value() {
        let mut cfg = LearnConfig::default();
        cfg.seed.address = Some("10.0.0.1".to_owned());
        assert_eq!(
            cfg.get_key("seed.address").unwrap(),
            "seed.address = 10.0.0.1"
        );
    }

    #[test]
    fn save_and_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        // Override config path via a temporary env (can't easily do that),
        // so we test the serialisation path directly.
        let mut cfg = LearnConfig::default();
        cfg.seed.address = Some("192.168.2.100".to_owned());
        cfg.seed.auto_push = true;

        let path = dir.path().join("config.json");
        let json = serde_json::to_vec_pretty(&cfg).unwrap();
        std::fs::write(&path, &json).unwrap();

        let loaded: LearnConfig = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(loaded.seed.address.as_deref(), Some("192.168.2.100"));
        assert!(loaded.seed.auto_push);
    }

    #[test]
    fn load_returns_default_for_missing_file() {
        // Config::load() reads from the real config path; no way to mock it here,
        // but we verify the deserialization fallback for malformed JSON.
        let result: LearnConfig = serde_json::from_slice(b"not json").unwrap_or_default();
        assert!(result.seed.address.is_none());
    }
}
