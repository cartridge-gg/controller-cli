use crate::config::Config;
use crate::error::{CliError, Result};
use crate::output::OutputFormatter;
use account_sdk::storage::{filestorage::FileSystemBackend, StorageBackend};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const LOOKUP_URL: &str = "https://api.cartridge.gg/accounts/lookup";

#[derive(Serialize)]
struct LookupRequest {
    addresses: Vec<String>,
}

#[derive(Deserialize)]
struct LookupEntry {
    username: String,
}

#[derive(Deserialize)]
struct LookupResponse {
    results: Vec<LookupEntry>,
}

pub async fn execute(config: &Config, formatter: &dyn OutputFormatter) -> Result<()> {
    let storage_path = PathBuf::from(shellexpand::tilde(&config.session.storage_path).to_string());
    let backend = FileSystemBackend::new(storage_path);

    let controller = backend
        .controller()
        .ok()
        .flatten()
        .ok_or(CliError::NoSession)?;

    let address = format!("0x{:x}", controller.address);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| CliError::ApiError(format!("Failed to build HTTP client: {e}")))?;

    let request = LookupRequest {
        addresses: vec![address],
    };

    let response = client
        .post(LOOKUP_URL)
        .header("Content-Type", "application/json")
        .json(&request)
        .send()
        .await
        .map_err(|e| CliError::ApiError(format!("Lookup request failed: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "unknown".to_string());
        return Err(CliError::ApiError(format!(
            "Lookup API returned {status}: {body}"
        )));
    }

    let lookup_response: LookupResponse = response
        .json()
        .await
        .map_err(|e| CliError::ApiError(format!("Failed to parse lookup response: {e}")))?;

    let username = lookup_response
        .results
        .first()
        .map(|e| e.username.clone())
        .ok_or_else(|| CliError::NotFoundError("No username found for this account".to_string()))?;

    if config.cli.json_output {
        formatter.success(&username);
    } else {
        println!("{username}");
    }

    Ok(())
}
