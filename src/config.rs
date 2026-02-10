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
    #[serde(default = "default_chain_id")]
    pub default_chain_id: String,
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

fn default_chain_id() -> String {
    "SN_SEPOLIA".to_string()
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
            default_chain_id: default_chain_id(),
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

    pub fn merge_from_env(&mut self) {
        if let Ok(path) = std::env::var("CARTRIDGE_STORAGE_PATH") {
            self.session.storage_path = path;
        }
        if let Ok(chain_id) = std::env::var("CARTRIDGE_CHAIN_ID") {
            self.session.default_chain_id = chain_id;
        }
        if let Ok(rpc_url) = std::env::var("CARTRIDGE_RPC_URL") {
            self.session.default_rpc_url = rpc_url;
        }
        if let Ok(json_output) = std::env::var("CARTRIDGE_JSON_OUTPUT") {
            self.cli.json_output = json_output.eq_ignore_ascii_case("true") || json_output == "1";
        }
    }
}
