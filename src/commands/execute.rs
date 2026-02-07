use crate::{
    config::Config,
    error::{CliError, Result},
    output::OutputFormatter,
};
use account_sdk::{
    account::session::account::SessionAccount,
    provider::CartridgeJsonRpcProvider,
    signers::Signer,
    storage::{filestorage::FileSystemBackend, StorageBackend},
};
use serde::{Deserialize, Serialize};
use starknet::{
    accounts::{Account, ConnectedAccount},
    core::types::{Call, Felt},
    providers::Provider,
};
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
struct CallFile {
    calls: Vec<CallSpec>,
}

#[derive(Debug, Deserialize, Serialize)]
struct CallSpec {
    #[serde(rename = "contractAddress")]
    contract_address: String,
    entrypoint: String,
    calldata: Vec<String>,
}

#[derive(Serialize)]
pub struct ExecuteOutput {
    pub transaction_hash: String,
    pub message: String,
}

#[allow(clippy::too_many_arguments)]
pub async fn execute(
    config: &Config,
    formatter: &dyn OutputFormatter,
    contract: Option<String>,
    entrypoint: Option<String>,
    calldata: Option<String>,
    call_file: Option<String>,
    wait: bool,
    timeout: u64,
) -> Result<()> {
    // Parse calls from arguments or file
    let calls = if let Some(call_file_path) = call_file {
        // Load calls from JSON file
        let file_content = std::fs::read_to_string(&call_file_path)
            .map_err(|e| CliError::InvalidInput(format!("Failed to read call file: {}", e)))?;

        let call_file: CallFile = serde_json::from_str(&file_content)
            .map_err(|e| CliError::InvalidInput(format!("Invalid call file format: {}", e)))?;

        call_file.calls
    } else if let (Some(contract_addr), Some(entry), Some(data)) = (contract, entrypoint, calldata)
    {
        // Single call from CLI arguments
        vec![CallSpec {
            contract_address: contract_addr,
            entrypoint: entry,
            calldata: data.split(',').map(|s| s.trim().to_string()).collect(),
        }]
    } else {
        return Err(CliError::InvalidInput(
            "Either --call-file or all of --contract, --entrypoint, --calldata must be provided"
                .to_string(),
        ));
    };

    formatter.info(&format!("Preparing to execute {} call(s)...", calls.len()));

    // Load session and credentials from storage
    let storage_path = PathBuf::from(shellexpand::tilde(&config.session.storage_path).to_string());
    let backend = FileSystemBackend::new(storage_path);

    let session_metadata = backend
        .session("session")
        .map_err(|e| CliError::Storage(e.to_string()))?
        .ok_or(CliError::NoSession)?;

    // Check if session is expired
    if session_metadata.session.is_expired() {
        let expires_at =
            chrono::DateTime::from_timestamp(session_metadata.session.inner.expires_at as i64, 0)
                .unwrap_or_else(|| chrono::Utc::now());

        return Err(CliError::SessionExpired(
            expires_at.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
        ));
    }

    let credentials = session_metadata
        .credentials
        .ok_or_else(|| CliError::InvalidSessionData("No credentials found".to_string()))?;

    // Load controller metadata to get address
    let controller_metadata = backend
        .controller()
        .map_err(|e| CliError::Storage(e.to_string()))?
        .ok_or_else(|| CliError::InvalidSessionData("No controller metadata found".to_string()))?;

    // Create signer from stored private key
    let signing_key = starknet::signers::SigningKey::from_secret_scalar(credentials.private_key);
    let signer = Signer::Starknet(signing_key);

    // Get authorization from credentials
    let authorization = credentials.authorization.clone();

    // Create provider
    let provider =
        CartridgeJsonRpcProvider::new(url::Url::parse(&config.session.default_rpc_url).unwrap());

    // Create session account using authorization
    let session_account = SessionAccount::new(
        provider,
        signer,
        controller_metadata.address,
        controller_metadata.chain_id,
        authorization,
        session_metadata.session.clone(),
    );

    // Convert CallSpec to starknet Call
    let starknet_calls: Vec<Call> = calls
        .iter()
        .map(|call| {
            let contract_address = Felt::from_hex(&call.contract_address)
                .map_err(|e| CliError::InvalidInput(format!("Invalid contract address: {}", e)))?;

            let selector = starknet::core::utils::get_selector_from_name(&call.entrypoint)
                .map_err(|e| CliError::InvalidInput(format!("Invalid entrypoint: {}", e)))?;

            let calldata: Result<Vec<Felt>> = call
                .calldata
                .iter()
                .map(|data| {
                    Felt::from_hex(data.trim())
                        .map_err(|e| CliError::InvalidInput(format!("Invalid calldata: {}", e)))
                })
                .collect();

            Ok(Call {
                to: contract_address,
                selector,
                calldata: calldata?,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    formatter.info("Executing transaction...");

    // Execute the calls
    let execution = session_account.execute_v3(starknet_calls);

    let result = execution
        .send()
        .await
        .map_err(|e| CliError::TransactionFailed(format!("Transaction failed: {}", e)))?;

    let transaction_hash = format!("0x{:x}", result.transaction_hash);

    let output = ExecuteOutput {
        transaction_hash: transaction_hash.clone(),
        message: if wait {
            "Transaction submitted. Waiting for confirmation...".to_string()
        } else {
            "Transaction submitted successfully".to_string()
        },
    };

    formatter.success(&output);

    // Wait for transaction confirmation if requested
    if wait {
        formatter.info("Waiting for transaction confirmation...");

        let start = std::time::Instant::now();
        let timeout_duration = std::time::Duration::from_secs(timeout);

        loop {
            if start.elapsed() > timeout_duration {
                return Err(CliError::TransactionFailed(format!(
                    "Transaction confirmation timeout after {} seconds",
                    timeout
                )));
            }

            // Check transaction status
            match session_account
                .provider()
                .get_transaction_receipt(result.transaction_hash)
                .await
            {
                Ok(_receipt) => {
                    formatter.info("Transaction confirmed!");
                    break;
                }
                Err(_) => {
                    // Transaction not yet confirmed, wait and retry
                    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                }
            }
        }
    }

    Ok(())
}
