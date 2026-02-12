use crate::error::{CliError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const PRESETS_BASE_URL: &str =
    "https://raw.githubusercontent.com/cartridge-gg/presets/refs/heads/main/configs";

#[derive(Deserialize, Serialize, Debug)]
pub struct PresetConfig {
    pub origin: Vec<String>,
    pub chains: HashMap<String, ChainConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme: Option<serde_json::Value>,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct ChainConfig {
    pub policies: PoliciesConfig,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct PoliciesConfig {
    pub contracts: HashMap<String, ContractConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub messages: Option<Vec<serde_json::Value>>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ContractConfig {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub methods: Vec<MethodConfig>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct MethodConfig {
    pub name: String,
    pub entrypoint: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Fetch preset configuration from GitHub
pub async fn fetch_preset(preset_name: &str) -> Result<PresetConfig> {
    let url = format!("{PRESETS_BASE_URL}/{preset_name}/config.json");

    let response = reqwest::get(&url).await.map_err(|e| {
        CliError::InvalidInput(format!("Failed to fetch preset '{preset_name}': {e}"))
    })?;

    if !response.status().is_success() {
        return Err(CliError::InvalidInput(format!(
            "Preset '{preset_name}' not found. Check available presets at: https://github.com/cartridge-gg/presets/tree/main/configs"
        )));
    }

    let preset: PresetConfig = response.json().await.map_err(|e| {
        CliError::InvalidInput(format!(
            "Failed to parse preset '{preset_name}' configuration: {e}"
        ))
    })?;

    Ok(preset)
}

/// Extract chain-specific policies from preset
pub fn extract_chain_policies(
    preset: &PresetConfig,
    chain_id: &str,
    preset_name: &str,
) -> Result<PoliciesConfig> {
    let chain_config = preset.chains.get(chain_id).ok_or_else(|| {
        let available_chains: Vec<_> = preset.chains.keys().map(|s| s.as_str()).collect();
        CliError::InvalidInput(format!(
            "Preset '{}' does not support chain '{}'. Available chains: {}",
            preset_name,
            chain_id,
            available_chains.join(", ")
        ))
    })?;

    Ok(chain_config.policies.clone())
}
