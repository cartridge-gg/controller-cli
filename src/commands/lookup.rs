use crate::{
    config::Config,
    error::{CliError, Result},
    output::OutputFormatter,
};
use serde::{Deserialize, Serialize};

const LOOKUP_URL: &str = "https://api.cartridge.gg/accounts/lookup";

#[derive(Serialize)]
struct LookupRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    usernames: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    addresses: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct LookupEntry {
    username: String,
    addresses: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct LookupResponse {
    results: Vec<LookupEntry>,
}

pub async fn execute(
    _config: &Config,
    formatter: &dyn OutputFormatter,
    usernames: Option<String>,
    addresses: Option<String>,
) -> Result<()> {
    let usernames_list = usernames.map(|s| {
        s.split(',')
            .map(|u| u.trim().to_string())
            .filter(|u| !u.is_empty())
            .collect::<Vec<_>>()
    });

    let addresses_list = addresses.map(|s| {
        s.split(',')
            .map(|a| a.trim().to_lowercase())
            .filter(|a| !a.is_empty())
            .collect::<Vec<_>>()
    });

    if usernames_list.is_none() && addresses_list.is_none() {
        return Err(CliError::InvalidInput(
            "Provide at least one of --usernames or --addresses".to_string(),
        ));
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| CliError::ApiError(format!("Failed to build HTTP client: {e}")))?;

    let request = LookupRequest {
        usernames: usernames_list,
        addresses: addresses_list,
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

    // Flatten into "username:address" entries (use first address per result)
    let entries: Vec<String> = lookup_response
        .results
        .iter()
        .filter_map(|entry| {
            entry
                .addresses
                .first()
                .map(|addr| format!("{}:{}", entry.username, addr))
        })
        .collect();

    formatter.success(&entries);

    Ok(())
}
