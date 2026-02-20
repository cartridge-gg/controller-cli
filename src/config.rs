use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub session: SessionConfig,
    #[serde(default)]
    pub cli: CliConfig,
    #[serde(default)]
    pub tokens: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    #[serde(default = "default_storage_path")]
    pub storage_path: String,
    #[serde(default = "default_rpc_url")]
    pub rpc_url: String,
    #[serde(default = "default_keychain_url")]
    pub keychain_url: String,
    #[serde(default = "default_api_url")]
    pub api_url: String,
    /// Whether rpc_url was explicitly set (via config file or env var)
    #[serde(skip)]
    pub rpc_url_explicitly_set: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliConfig {
    #[serde(default)]
    pub json_output: bool,
    #[serde(default = "default_true")]
    pub use_colors: bool,
    #[serde(default = "default_callback_timeout")]
    pub callback_timeout_seconds: u64,
}

fn default_storage_path() -> String {
    dirs::config_dir()
        .map(|p| p.join("controller-cli").to_string_lossy().to_string())
        .unwrap_or_else(|| "~/.config/controller-cli".to_string())
}

fn default_rpc_url() -> String {
    "https://api.cartridge.gg/x/starknet/sepolia".to_string()
}

fn default_keychain_url() -> String {
    "https://x.cartridge.gg".to_string()
}

fn default_api_url() -> String {
    "https://api.cartridge.gg/query".to_string()
}

fn default_true() -> bool {
    true
}

fn default_callback_timeout() -> u64 {
    300
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            storage_path: default_storage_path(),
            rpc_url: default_rpc_url(),
            keychain_url: default_keychain_url(),
            api_url: default_api_url(),
            rpc_url_explicitly_set: false,
        }
    }
}

impl Default for CliConfig {
    fn default() -> Self {
        Self {
            json_output: false,
            use_colors: default_true(),
            callback_timeout_seconds: default_callback_timeout(),
        }
    }
}

impl Config {
    pub fn load() -> anyhow::Result<Self> {
        let config_path = Self::config_path()?;

        if !config_path.exists() {
            return Ok(Self::default());
        }

        let contents = std::fs::read_to_string(&config_path)?;
        let mut config: Config = toml::from_str(&contents)?;

        // Check if rpc-url was explicitly set in the config file
        let raw: toml::Value = toml::from_str(&contents)?;
        if raw.get("session").and_then(|s| s.get("rpc_url")).is_some() {
            config.session.rpc_url_explicitly_set = true;
        }

        Ok(config)
    }

    pub fn config_path() -> anyhow::Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine config directory"))?;
        Ok(config_dir.join("controller-cli").join("config.toml"))
    }

    pub const VALID_KEYS: &'static [&'static str] = &[
        "rpc-url",
        "keychain-url",
        "api-url",
        "storage-path",
        "json-output",
        "colors",
        "callback-timeout",
    ];

    pub fn save(&self) -> anyhow::Result<()> {
        let config_path = Self::config_path()?;
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let contents = toml::to_string_pretty(self)?;
        std::fs::write(&config_path, contents)?;
        Ok(())
    }

    pub fn get_by_alias(&self, alias: &str) -> anyhow::Result<String> {
        if let Some(symbol) = alias.strip_prefix("token.") {
            return self
                .tokens
                .get(symbol)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("No custom token configured for '{symbol}'"));
        }

        match alias {
            "rpc-url" => Ok(self.session.rpc_url.clone()),
            "keychain-url" => Ok(self.session.keychain_url.clone()),
            "api-url" => Ok(self.session.api_url.clone()),
            "storage-path" => Ok(self.session.storage_path.clone()),
            "json-output" => Ok(self.cli.json_output.to_string()),
            "colors" => Ok(self.cli.use_colors.to_string()),
            "callback-timeout" => Ok(self.cli.callback_timeout_seconds.to_string()),
            _ => anyhow::bail!(
                "Unknown config key '{}'. Valid keys: {}, token.<symbol>",
                alias,
                Self::VALID_KEYS.join(", ")
            ),
        }
    }

    pub fn set_by_alias(&mut self, alias: &str, value: &str) -> anyhow::Result<()> {
        if let Some(symbol) = alias.strip_prefix("token.") {
            self.tokens.insert(symbol.to_string(), value.to_string());
            return Ok(());
        }

        match alias {
            "rpc-url" => self.session.rpc_url = value.to_string(),
            "keychain-url" => self.session.keychain_url = value.to_string(),
            "api-url" => self.session.api_url = value.to_string(),
            "storage-path" => self.session.storage_path = value.to_string(),
            "json-output" => {
                self.cli.json_output = value.parse::<bool>().map_err(|_| {
                    anyhow::anyhow!("Invalid value for json-output: expected 'true' or 'false'")
                })?;
            }
            "colors" => {
                self.cli.use_colors = value.parse::<bool>().map_err(|_| {
                    anyhow::anyhow!("Invalid value for colors: expected 'true' or 'false'")
                })?;
            }
            "callback-timeout" => {
                self.cli.callback_timeout_seconds = value.parse::<u64>().map_err(|_| {
                    anyhow::anyhow!(
                        "Invalid value for callback-timeout: expected a positive integer"
                    )
                })?;
            }
            _ => anyhow::bail!(
                "Unknown config key '{}'. Valid keys: {}, token.<symbol>",
                alias,
                Self::VALID_KEYS.join(", ")
            ),
        }
        Ok(())
    }

    pub fn merge_from_env(&mut self) {
        if let Ok(path) = std::env::var("CARTRIDGE_STORAGE_PATH") {
            self.session.storage_path = path;
        }
        if let Ok(rpc_url) = std::env::var("CARTRIDGE_RPC_URL") {
            self.session.rpc_url = rpc_url;
            self.session.rpc_url_explicitly_set = true;
        }
        if let Ok(json_output) = std::env::var("CARTRIDGE_JSON_OUTPUT") {
            self.cli.json_output = json_output.eq_ignore_ascii_case("true") || json_output == "1";
        }
    }

    /// Validate an account label: must be non-empty, alphanumeric with hyphens/underscores,
    /// and must not contain path separators or traversal sequences.
    pub fn validate_account_name(name: &str) -> std::result::Result<(), String> {
        if name.is_empty() {
            return Err("Account name cannot be empty".to_string());
        }
        if name.len() > 64 {
            return Err("Account name cannot exceed 64 characters".to_string());
        }
        if !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err(format!(
                "Account name '{name}' contains invalid characters. Only alphanumeric, '-', and '_' are allowed"
            ));
        }
        Ok(())
    }

    /// Resolve the storage path for a given account label.
    /// When `account` is `Some("player1")`, returns `<base>/accounts/player1/`.
    /// When `account` is `None`, returns the default base path (backward compatible).
    ///
    /// Panics if the account name is invalid (callers should validate first or use
    /// `validate_account_name` for user-facing errors).
    pub fn resolve_storage_path(&self, account: Option<&str>) -> PathBuf {
        let base = PathBuf::from(shellexpand::tilde(&self.session.storage_path).to_string());
        match account {
            Some(name) => {
                // Defense-in-depth: reject obviously unsafe names even if caller forgot to validate
                assert!(
                    Self::validate_account_name(name).is_ok(),
                    "invalid account name passed to resolve_storage_path: {name}"
                );
                base.join("accounts").join(name)
            }
            None => base,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_storage_path_default_no_account() {
        let config = Config {
            session: SessionConfig {
                storage_path: "/tmp/controller-test".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };
        let path = config.resolve_storage_path(None);
        assert_eq!(path, PathBuf::from("/tmp/controller-test"));
    }

    #[test]
    fn resolve_storage_path_with_account() {
        let config = Config {
            session: SessionConfig {
                storage_path: "/tmp/controller-test".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };
        let path = config.resolve_storage_path(Some("player1"));
        assert_eq!(
            path,
            PathBuf::from("/tmp/controller-test/accounts/player1")
        );
    }

    #[test]
    fn resolve_storage_path_different_accounts_are_isolated() {
        let config = Config {
            session: SessionConfig {
                storage_path: "/tmp/controller-test".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };
        let path_a = config.resolve_storage_path(Some("player1"));
        let path_b = config.resolve_storage_path(Some("player2"));
        assert_ne!(path_a, path_b);
        assert!(path_a.starts_with("/tmp/controller-test/accounts/"));
        assert!(path_b.starts_with("/tmp/controller-test/accounts/"));
    }

    #[test]
    fn validate_account_name_accepts_valid_names() {
        assert!(Config::validate_account_name("player1").is_ok());
        assert!(Config::validate_account_name("my-agent").is_ok());
        assert!(Config::validate_account_name("bot_42").is_ok());
        assert!(Config::validate_account_name("ABC").is_ok());
    }

    #[test]
    fn validate_account_name_rejects_empty() {
        assert!(Config::validate_account_name("").is_err());
    }

    #[test]
    fn validate_account_name_rejects_path_traversal() {
        assert!(Config::validate_account_name("..").is_err());
        assert!(Config::validate_account_name("../etc").is_err());
        assert!(Config::validate_account_name("foo/bar").is_err());
        assert!(Config::validate_account_name("foo\\bar").is_err());
    }

    #[test]
    fn validate_account_name_rejects_special_chars() {
        assert!(Config::validate_account_name("player 1").is_err());
        assert!(Config::validate_account_name("player@1").is_err());
        assert!(Config::validate_account_name("play!er").is_err());
    }

    #[test]
    fn validate_account_name_rejects_too_long() {
        let long_name = "a".repeat(65);
        assert!(Config::validate_account_name(&long_name).is_err());
        // 64 is ok
        let ok_name = "a".repeat(64);
        assert!(Config::validate_account_name(&ok_name).is_ok());
    }

    #[test]
    #[should_panic(expected = "invalid account name")]
    fn resolve_storage_path_panics_on_invalid_name() {
        let config = Config {
            session: SessionConfig {
                storage_path: "/tmp/controller-test".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };
        let _ = config.resolve_storage_path(Some("../etc"));
    }
}
