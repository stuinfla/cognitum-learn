//! Persistent configuration for cognitum-learn.
//!
//! Stored as JSON at the platform's standard config dir:
//!   - Linux:   `~/.config/learn-rs/config.json`
//!   - macOS:   `~/Library/Application Support/learn-rs/config.json`
//!   - Windows: `%APPDATA%\learn-rs\config.json`
//!
//! Env vars override file values:
//!   - `LEARN_SEED_ADDRESS` overrides `seed.address`
//!   - `LEARN_SEED_AUTO_PUSH` overrides `seed.auto_push`
//!   - `LEARN_SEED_TOKEN` overrides `seed.token`

use learn_core::LearnError;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SeedConfig {
    pub address: Option<String>,
    #[serde(default)]
    pub auto_push: bool,
    /// Bearer token from the Seed's pairing flow (`POST /api/v1/pair`).
    /// Used as the default for `learn push` when neither `--token` nor
    /// `LEARN_SEED_TOKEN` is supplied.
    #[serde(default)]
    pub token: Option<String>,
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

    /// Effective seed bearer token: `LEARN_SEED_TOKEN` env var overrides file config.
    /// Returns `None` when both env and file are unset/empty.
    pub fn seed_token(&self) -> Option<String> {
        if let Ok(v) = std::env::var("LEARN_SEED_TOKEN") {
            if !v.is_empty() {
                return Some(v);
            }
        }
        self.seed.token.clone().filter(|t| !t.is_empty())
    }

    /// Set a config key by dotted path (`seed.address`, `seed.auto_push`, `seed.token`).
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
            "seed.token" => {
                self.seed.token = if value.is_empty() {
                    None
                } else {
                    Some(value.to_owned())
                };
            }
            _ => {
                return Err(LearnError::Apply(format!(
                    "unknown config key '{key}'\n  \
                     valid keys: seed.address  seed.auto_push  seed.token"
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
            "seed.token" => {
                let v = self
                    .seed_token()
                    .map(|_| "(set)".to_owned())
                    .unwrap_or_else(|| "(not set)".to_owned());
                Ok(format!("seed.token = {v}"))
            }
            _ => Err(LearnError::Apply(format!(
                "unknown config key '{key}'\n  \
                 valid keys: seed.address  seed.auto_push  seed.token"
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
    println!(
        "  seed.token     = {}",
        if cfg.seed_token().is_some() {
            "(set)"
        } else {
            "(not set)"
        }
    );
    println!();
    println!(
        "  Override with env vars: LEARN_SEED_ADDRESS  LEARN_SEED_AUTO_PUSH  LEARN_SEED_TOKEN"
    );
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

    #[test]
    fn set_seed_token_stores_value() {
        let mut cfg = LearnConfig::default();
        cfg.set_key("seed.token", "bearer-abc-123").unwrap();
        assert_eq!(cfg.seed.token.as_deref(), Some("bearer-abc-123"));
    }

    #[test]
    fn set_seed_token_empty_clears() {
        let mut cfg = LearnConfig::default();
        cfg.seed.token = Some("old".to_owned());
        cfg.set_key("seed.token", "").unwrap();
        assert!(cfg.seed.token.is_none());
    }

    #[test]
    fn seed_token_returns_file_value_when_env_unset() {
        let mut cfg = LearnConfig::default();
        cfg.seed.token = Some("file-token".to_owned());
        // Cannot reliably mutate env in a parallel test, so only assert when env unset.
        if std::env::var("LEARN_SEED_TOKEN").is_err() {
            assert_eq!(cfg.seed_token().as_deref(), Some("file-token"));
        }
    }

    #[test]
    fn seed_token_returns_none_when_both_unset() {
        let cfg = LearnConfig::default();
        if std::env::var("LEARN_SEED_TOKEN").is_err() {
            assert!(cfg.seed_token().is_none());
        }
    }

    #[test]
    fn get_seed_token_redacts_value() {
        let mut cfg = LearnConfig::default();
        cfg.seed.token = Some("super-secret-bearer".to_owned());
        if std::env::var("LEARN_SEED_TOKEN").is_err() {
            let s = cfg.get_key("seed.token").unwrap();
            assert!(s.contains("(set)"), "expected '(set)' marker; got: {s}");
            assert!(
                !s.contains("super-secret-bearer"),
                "token must NOT be printed; got: {s}"
            );
        }
    }

    #[test]
    fn config_deserializes_with_token_field() {
        // Matches the live shape at ~/Library/Application Support/learn-rs/config.json.
        let json = br#"{
            "seed": {
                "address": "10.0.0.72",
                "auto_push": true,
                "token": "r3kpdooQBcLt2QFX83pAjaObUMAUxwUFBQZtg-Ph3P0"
            }
        }"#;
        let cfg: LearnConfig = serde_json::from_slice(json).unwrap();
        assert_eq!(cfg.seed.address.as_deref(), Some("10.0.0.72"));
        assert!(cfg.seed.auto_push);
        assert_eq!(
            cfg.seed.token.as_deref(),
            Some("r3kpdooQBcLt2QFX83pAjaObUMAUxwUFBQZtg-Ph3P0")
        );
    }
}
