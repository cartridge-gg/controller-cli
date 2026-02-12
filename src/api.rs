use crate::error::{CliError, Result};
use serde::{Deserialize, Serialize};
use starknet::core::types::Felt;

/// Shorten a URL via the Cartridge URL shortener service.
///
/// POSTs to `{api_base}/s` and returns the short URL on success.
/// Returns `Err` on any failure so the caller can fall back to the original URL.
pub async fn shorten_url(api_url: &str, long_url: &str) -> Result<String> {
    // Derive base URL by stripping `/query` from the API URL
    let api_base = api_url.trim_end_matches("/query").trim_end_matches('/');

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| CliError::ApiError(format!("Failed to build HTTP client: {e}")))?;

    #[derive(Serialize)]
    struct ShortenRequest<'a> {
        url: &'a str,
    }

    #[derive(Deserialize)]
    struct ShortenResponse {
        url: String,
    }

    let response = client
        .post(format!("{api_base}/s"))
        .json(&ShortenRequest { url: long_url })
        .send()
        .await
        .map_err(|e| CliError::ApiError(format!("Failed to shorten URL: {e}")))?;

    if !response.status().is_success() {
        return Err(CliError::ApiError(format!(
            "URL shortener returned error status: {}",
            response.status()
        )));
    }

    let shorten_response: ShortenResponse = response
        .json()
        .await
        .map_err(|e| CliError::ApiError(format!("Failed to parse shortener response: {e}")))?;

    Ok(shorten_response.url)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SessionInfo {
    pub authorization: Vec<String>, // Hex-encoded Felt values
    pub controller: ControllerInfo,
    #[serde(rename = "chainID")]
    pub chain_id: String,
    #[serde(rename = "expiresAt")]
    pub expires_at: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ControllerInfo {
    pub address: String,
    #[serde(rename = "accountID")]
    pub account_id: String,
}

/// Query session creation from the Cartridge API (long-polling)
///
/// This uses the `subscribeCreateSession` query which implements long-polling:
/// - Backend holds the HTTP connection open for up to 2 minutes
/// - Checks database periodically for session creation
/// - Returns null if timeout, or SessionInfo if session is created
///
/// Despite the name, this is a **Query** not a Subscription.
pub async fn query_session_info(
    api_url: &str,
    session_key_guid: &str,
) -> Result<Option<SessionInfo>> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(130)) // Slightly longer than backend's 2min timeout
        .build()
        .map_err(|e| CliError::ApiError(format!("Failed to build HTTP client: {e}")))?;

    // This is a QUERY (not subscription) despite the name
    let query = r#"
        query SubscribeCreateSession($sessionKeyGuid: Felt!) {
            subscribeCreateSession(sessionKeyGuid: $sessionKeyGuid) {
                id
                appID
                chainID
                isRevoked
                expiresAt
                createdAt
                updatedAt
                authorization
                controller {
                    address
                    accountID
                }
            }
        }
    "#;

    #[derive(Serialize)]
    struct Variables {
        #[serde(rename = "sessionKeyGuid")]
        session_key_guid: String,
    }

    #[derive(Serialize)]
    struct GraphQLRequest {
        query: String,
        variables: Variables,
    }

    #[derive(Deserialize)]
    struct GraphQLResponse {
        data: Option<GraphQLData>,
        errors: Option<Vec<GraphQLError>>,
    }

    #[derive(Deserialize)]
    struct GraphQLData {
        #[serde(rename = "subscribeCreateSession")]
        subscribe_create_session: Option<SessionInfo>,
    }

    #[derive(Deserialize)]
    struct GraphQLError {
        message: String,
    }

    let request = GraphQLRequest {
        query: query.to_string(),
        variables: Variables {
            session_key_guid: session_key_guid.to_string(),
        },
    };

    let response = client
        .post(api_url)
        .json(&request)
        .send()
        .await
        .map_err(|e| CliError::ApiError(format!("Failed to query session info: {e}")))?;

    if !response.status().is_success() {
        return Err(CliError::ApiError(format!(
            "API returned error status: {}",
            response.status()
        )));
    }

    let graphql_response: GraphQLResponse = response
        .json()
        .await
        .map_err(|e| CliError::ApiError(format!("Failed to parse API response: {e}")))?;

    if let Some(errors) = graphql_response.errors {
        let error_messages: Vec<String> = errors.iter().map(|e| e.message.clone()).collect();
        return Err(CliError::ApiError(format!(
            "GraphQL errors: {}",
            error_messages.join(", ")
        )));
    }

    Ok(graphql_response
        .data
        .and_then(|data| data.subscribe_create_session))
}

impl SessionInfo {
    /// Convert authorization strings to Felt values
    pub fn authorization_as_felts(&self) -> Result<Vec<Felt>> {
        self.authorization
            .iter()
            .map(|hex| {
                Felt::from_hex(hex).map_err(|e| {
                    CliError::InvalidSessionData(format!("Invalid authorization hex: {e}"))
                })
            })
            .collect()
    }

    /// Convert address string to Felt
    pub fn address_as_felt(&self) -> Result<Felt> {
        Felt::from_hex(&self.controller.address)
            .map_err(|e| CliError::InvalidSessionData(format!("Invalid address hex: {e}")))
    }

    /// Convert chain_id string to Felt
    pub fn chain_id_as_felt(&self) -> Result<Felt> {
        // Try hex first
        if let Ok(felt) = Felt::from_hex(&self.chain_id) {
            return Ok(felt);
        }

        // Try as short string (e.g., "SN_SEPOLIA")
        if let Ok(felt) = starknet::core::utils::cairo_short_string_to_felt(&self.chain_id) {
            return Ok(felt);
        }

        // Debug: show what we got
        Err(CliError::InvalidSessionData(format!(
            "Invalid chain_id format: '{}' (expected hex or short string)",
            self.chain_id
        )))
    }
}
