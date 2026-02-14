use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub session: SessionConfig,
    #[serde(default)]
    pub cli: CliConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    #[serde(default = "default_storage_path")]
    pub storage_path: String,
    #[serde(default = "default_rpc_url")]
    pub default_rpc_url: String,
    #[serde(default = "default_keychain_url")]
    pub keychain_url: String,
    #[serde(default = "default_api_url")]
    pub api_url: String,
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
            default_rpc_url: default_rpc_url(),
            keychain_url: default_keychain_url(),
            api_url: default_api_url(),
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
        let config: Config = toml::from_str(&contents)?;
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
        match alias {
            "rpc-url" => Ok(self.session.default_rpc_url.clone()),
            "keychain-url" => Ok(self.session.keychain_url.clone()),
            "api-url" => Ok(self.session.api_url.clone()),
            "storage-path" => Ok(self.session.storage_path.clone()),
            "json-output" => Ok(self.cli.json_output.to_string()),
            "colors" => Ok(self.cli.use_colors.to_string()),
            "callback-timeout" => Ok(self.cli.callback_timeout_seconds.to_string()),
            _ => anyhow::bail!(
                "Unknown config key '{}'. Valid keys: {}",
                alias,
                Self::VALID_KEYS.join(", ")
            ),
        }
    }

    pub fn set_by_alias(&mut self, alias: &str, value: &str) -> anyhow::Result<()> {
        match alias {
            "rpc-url" => self.session.default_rpc_url = value.to_string(),
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
                "Unknown config key '{}'. Valid keys: {}",
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
            self.session.default_rpc_url = rpc_url;
        }
        if let Ok(json_output) = std::env::var("CARTRIDGE_JSON_OUTPUT") {
            self.cli.json_output = json_output.eq_ignore_ascii_case("true") || json_output == "1";
        }
    }
}
