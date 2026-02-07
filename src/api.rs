use crate::error::{CliError, Result};
use serde::{Deserialize, Serialize};
use starknet::core::types::Felt;

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

/// Query session information from the Cartridge API
///
/// This is a placeholder implementation. The actual API endpoint needs to be implemented
/// on the backend to return:
/// - authorization: Vec<Felt> signature that proves owner approved this session
/// - address: Account address
/// - chain_id: StarkNet chain ID
/// - expires_at: Unix timestamp when session expires
/// - owner_signer: Full signer details (Starknet or Webauthn)
pub async fn query_session_info(
    api_url: &str,
    session_key_guid: &str,
) -> Result<Option<SessionInfo>> {
    // Uses existing subscribeCreateSession query from controller-rs
    // See: account_sdk/src/graphql/session/subscribe-create-session.graphql

    let client = reqwest::Client::new();

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
        .map_err(|e| CliError::ApiError(format!("Failed to query session info: {}", e)))?;

    if !response.status().is_success() {
        return Err(CliError::ApiError(format!(
            "API returned error status: {}",
            response.status()
        )));
    }

    let graphql_response: GraphQLResponse = response
        .json()
        .await
        .map_err(|e| CliError::ApiError(format!("Failed to parse API response: {}", e)))?;

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
                    CliError::InvalidSessionData(format!("Invalid authorization hex: {}", e))
                })
            })
            .collect()
    }

    /// Convert address string to Felt
    pub fn address_as_felt(&self) -> Result<Felt> {
        Felt::from_hex(&self.controller.address).map_err(|e| {
            CliError::InvalidSessionData(format!("Invalid address hex: {}", e))
        })
    }

    /// Convert chain_id string to Felt
    pub fn chain_id_as_felt(&self) -> Result<Felt> {
        Felt::from_hex(&self.chain_id)
            .map_err(|e| CliError::InvalidSessionData(format!("Invalid chain_id hex: {}", e)))
    }
}
