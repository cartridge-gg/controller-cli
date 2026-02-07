use crate::error::{CliError, Result};
use serde::{Deserialize, Serialize};
use starknet::core::types::Felt;

#[derive(Debug, Serialize, Deserialize)]
pub struct SessionInfo {
    pub authorization: Vec<String>, // Hex-encoded Felt values
    pub address: String,
    pub chain_id: String,
    pub expires_at: u64,
    pub username: String,
    pub class_hash: String,
    pub rpc_url: String,
    pub salt: String,
    pub owner_signer: SignerInfo,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum SignerInfo {
    Starknet {
        private_key: String, // Hex-encoded Felt for storage (not for signing!)
    },
    Webauthn {
        // TODO: Define webauthn storage fields based on account_sdk requirements
        data: String,
    },
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
    // TODO: Replace with actual GraphQL query once backend implements SessionInfo endpoint
    //
    // Expected GraphQL query:
    // query SessionInfo($sessionKeyGuid: String!) {
    //   session(sessionKeyGuid: $sessionKeyGuid) {
    //     authorization
    //     address
    //     chainId
    //     expiresAt
    //     username
    //     classHash
    //     rpcUrl
    //     salt
    //     ownerSigner {
    //       type
    //       ... on StarknetSigner {
    //         privateKey  # For storage, not signing
    //       }
    //       ... on WebauthnSigner {
    //         # TBD: webauthn storage fields
    //       }
    //     }
    //   }
    // }

    let client = reqwest::Client::new();

    let query = r#"
        query SessionInfo($sessionKeyGuid: String!) {
            session(sessionKeyGuid: $sessionKeyGuid) {
                authorization
                address
                chainId
                expiresAt
                ownerSigner {
                    type
                    ... on StarknetSigner {
                        publicKey
                    }
                    ... on WebauthnSigner {
                        origin
                        rpId
                        publicKey
                    }
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
        session: Option<SessionInfo>,
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
        .and_then(|data| data.session))
}

impl SessionInfo {
    /// Convert authorization strings to Felt values
    pub fn authorization_as_felts(&self) -> Result<Vec<Felt>> {
        self.authorization
            .iter()
            .map(|hex| {
                Felt::from_hex(hex)
                    .map_err(|e| CliError::InvalidSessionData(format!("Invalid authorization hex: {}", e)))
            })
            .collect()
    }

    /// Convert address string to Felt
    pub fn address_as_felt(&self) -> Result<Felt> {
        Felt::from_hex(&self.address)
            .map_err(|e| CliError::InvalidSessionData(format!("Invalid address hex: {}", e)))
    }

    /// Convert chain_id string to Felt
    pub fn chain_id_as_felt(&self) -> Result<Felt> {
        Felt::from_hex(&self.chain_id)
            .map_err(|e| CliError::InvalidSessionData(format!("Invalid chain_id hex: {}", e)))
    }

    /// Convert class_hash string to Felt
    pub fn class_hash_as_felt(&self) -> Result<Felt> {
        Felt::from_hex(&self.class_hash)
            .map_err(|e| CliError::InvalidSessionData(format!("Invalid class_hash hex: {}", e)))
    }

    /// Convert salt string to Felt
    pub fn salt_as_felt(&self) -> Result<Felt> {
        Felt::from_hex(&self.salt)
            .map_err(|e| CliError::InvalidSessionData(format!("Invalid salt hex: {}", e)))
    }
}
