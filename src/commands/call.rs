use crate::config::Config;
use crate::error::{CliError, Result};
use crate::output::OutputFormatter;
use serde::{Deserialize, Serialize};
use starknet::core::{
    types::{BlockId, BlockTag, Felt, FunctionCall},
    utils::cairo_short_string_to_felt,
};
use starknet::providers::{jsonrpc::HttpTransport, JsonRpcClient, Provider};

/// Execute a read-only call to a contract
#[allow(clippy::too_many_arguments)]
pub async fn execute(
    config: &Config,
    formatter: &dyn OutputFormatter,
    contract: Option<String>,
    entrypoint: Option<String>,
    calldata: Option<String>,
    file: Option<String>,
    chain_id: Option<String>,
    rpc_url: Option<String>,
    block_id: Option<String>,
) -> Result<()> {
    // Determine RPC URL
    let rpc_url = resolve_rpc_url(chain_id, rpc_url, config, formatter)?;

    // Build the provider
    let url = url::Url::parse(&rpc_url)
        .map_err(|e| CliError::InvalidInput(format!("Invalid RPC URL: {e}")))?;
    let provider = JsonRpcClient::new(HttpTransport::new(url));

    // Parse block ID (default to latest)
    let block_id = parse_block_id(block_id)?;

    // Handle file input for multiple calls
    if let Some(file_path) = file {
        let calls = parse_calls_file(&file_path)?;
        let mut results = Vec::new();

        for call in calls {
            match execute_single_call(&provider, &call, block_id).await {
                Ok(result) => results.push(CallResult {
                    contract: call.contract_address.clone(),
                    entrypoint: call.entrypoint.clone(),
                    success: true,
                    result: Some(result),
                    error: None,
                }),
                Err(e) => results.push(CallResult {
                    contract: call.contract_address.clone(),
                    entrypoint: call.entrypoint.clone(),
                    success: false,
                    result: None,
                    error: Some(e.to_string()),
                }),
            }
        }

        formatter.success(&CallBatchOutput { calls: results });
        return Ok(());
    }

    // Handle single call
    let contract = contract
        .ok_or_else(|| CliError::InvalidInput("Missing required argument: contract".to_string()))?;
    let entrypoint = entrypoint.ok_or_else(|| {
        CliError::InvalidInput("Missing required argument: entrypoint".to_string())
    })?;

    let call = ContractCall {
        contract_address: contract,
        entrypoint,
        calldata: parse_calldata(calldata)?,
    };

    let result = execute_single_call(&provider, &call, block_id).await?;

    formatter.success(&result);
    Ok(())
}

async fn execute_single_call(
    provider: &JsonRpcClient<HttpTransport>,
    call: &ContractCall,
    block_id: BlockId,
) -> Result<Vec<String>> {
    let contract_address = Felt::from_hex(&call.contract_address)
        .map_err(|e| CliError::InvalidInput(format!("Invalid contract address: {e}")))?;

    let selector = starknet::core::utils::get_selector_from_name(&call.entrypoint)
        .map_err(|e| CliError::InvalidInput(format!("Invalid entrypoint name: {e}")))?;

    let calldata: Vec<Felt> = call
        .calldata
        .iter()
        .map(|s| parse_calldata_value(s))
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .flatten()
        .collect();

    let function_call = FunctionCall {
        contract_address,
        entry_point_selector: selector,
        calldata,
    };

    let result = provider
        .call(function_call, block_id)
        .await
        .map_err(|e| CliError::TransactionFailed(format!("Call failed: {e}")))?;

    Ok(result.iter().map(|f| format!("0x{f:x}")).collect())
}

fn parse_block_id(block_id: Option<String>) -> Result<BlockId> {
    match block_id.as_deref() {
        None | Some("latest") => Ok(BlockId::Tag(BlockTag::Latest)),
        Some(num) if num.starts_with("0x") => {
            let hash = Felt::from_hex(num)
                .map_err(|e| CliError::InvalidInput(format!("Invalid block hash: {e}")))?;
            Ok(BlockId::Hash(hash))
        }
        Some(num) => {
            let number = num
                .parse::<u64>()
                .map_err(|e| CliError::InvalidInput(format!("Invalid block number: {e}")))?;
            Ok(BlockId::Number(number))
        }
    }
}

fn parse_calldata(calldata: Option<String>) -> Result<Vec<String>> {
    match calldata {
        None => Ok(Vec::new()),
        Some(data) => Ok(data.split(',').map(|s| s.trim().to_string()).collect()),
    }
}

/// Parse a calldata value, handling special prefixes (u256:, str:) and default felt parsing.
fn parse_calldata_value(value: &str) -> Result<Vec<Felt>> {
    if let Some(u256_str) = value.strip_prefix("u256:") {
        // Parse u256 value and split into low/high felts
        let normalized = if u256_str.starts_with("0X") {
            u256_str.to_lowercase()
        } else {
            u256_str.to_string()
        };

        let felt = normalized
            .parse::<Felt>()
            .map_err(|e| CliError::InvalidInput(format!("Invalid u256 value '{value}': {e}")))?;

        // Split into low and high 128-bit parts
        let felt_bytes = felt.to_bytes_be();
        let high_bytes = &felt_bytes[0..16];
        let low_bytes = &felt_bytes[16..32];

        let low = Felt::from_bytes_be_slice(low_bytes);
        let high = Felt::from_bytes_be_slice(high_bytes);

        Ok(vec![low, high])
    } else if let Some(str_value) = value.strip_prefix("str:") {
        // Parse Cairo short string
        let felt = cairo_short_string_to_felt(str_value)
            .map_err(|e| CliError::InvalidInput(format!("Invalid short string '{value}': {e}")))?;
        Ok(vec![felt])
    } else {
        // Default: parse as felt
        let normalized = if value.starts_with("0X") {
            value.to_lowercase()
        } else {
            value.to_string()
        };
        let felt = normalized
            .parse::<Felt>()
            .map_err(|e| CliError::InvalidInput(format!("Invalid felt value '{value}': {e}")))?;
        Ok(vec![felt])
    }
}

fn parse_calls_file(file_path: &str) -> Result<Vec<ContractCall>> {
    let content = std::fs::read_to_string(file_path).map_err(|e| CliError::FileError {
        path: file_path.to_string(),
        message: e.to_string(),
    })?;

    let file: CallsFile = serde_json::from_str(&content)
        .map_err(|e| CliError::InvalidInput(format!("Invalid JSON in calls file: {e}")))?;

    Ok(file.calls)
}

#[derive(Debug, Deserialize)]
struct CallsFile {
    calls: Vec<ContractCall>,
}

#[derive(Debug, Deserialize)]
struct ContractCall {
    #[serde(rename = "contractAddress")]
    contract_address: String,
    entrypoint: String,
    calldata: Vec<String>,
}

#[derive(Debug, Serialize)]
struct CallResult {
    contract: String,
    entrypoint: String,
    success: bool,
    result: Option<Vec<String>>,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct CallBatchOutput {
    calls: Vec<CallResult>,
}

/// Resolve RPC URL from chain_id, explicit rpc_url, or config
fn resolve_rpc_url(
    chain_id: Option<String>,
    rpc_url: Option<String>,
    config: &Config,
    formatter: &dyn OutputFormatter,
) -> Result<String> {
    // If explicit RPC URL provided, use it
    if let Some(url) = rpc_url {
        return Ok(url);
    }

    // If chain_id provided, map to known RPC URL
    if let Some(chain) = chain_id {
        match chain.as_str() {
            "SN_MAIN" => Ok("https://api.cartridge.gg/x/starknet/mainnet".to_string()),
            "SN_SEPOLIA" => Ok("https://api.cartridge.gg/x/starknet/sepolia".to_string()),
            _ => Err(CliError::InvalidInput(format!(
                "Unsupported chain ID '{chain}'. Supported chains: SN_MAIN, SN_SEPOLIA"
            ))),
        }
    } else if !config.session.default_rpc_url.is_empty() {
        // Fall back to config default
        Ok(config.session.default_rpc_url.clone())
    } else {
        // No chain_id, no rpc_url, no config default - use Sepolia with warning
        formatter.warning("No --chain-id or --rpc-url specified, using SN_SEPOLIA by default");
        Ok("https://api.cartridge.gg/x/starknet/sepolia".to_string())
    }
}
