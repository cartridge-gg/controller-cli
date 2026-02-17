use crate::config::Config;
use crate::error::{CliError, Result};
use crate::output::OutputFormatter;
use cainome_cairo_serde::{ByteArray, CairoSerde};
use serde::{Deserialize, Serialize};
use starknet::core::types::{BlockId, BlockTag, FunctionCall};
use starknet::providers::{jsonrpc::HttpTransport, JsonRpcClient, Provider};

use super::{parse_starterpack_id, resolve_rpc_url, STARTERPACK_CONTRACT};

#[derive(Serialize, Deserialize)]
struct StarterpackMetadata {
    name: String,
    description: String,
    image_uri: String,
    #[serde(default)]
    items: Vec<StarterpackItem>,
}

#[derive(Serialize, Deserialize)]
struct StarterpackItem {
    name: String,
    description: String,
    image_uri: String,
}

pub async fn execute(
    config: &Config,
    formatter: &dyn OutputFormatter,
    id: String,
    chain_id: Option<String>,
    rpc_url: Option<String>,
) -> Result<()> {
    let rpc_url = resolve_rpc_url(chain_id, rpc_url, config, formatter)?;

    let url = url::Url::parse(&rpc_url)
        .map_err(|e| CliError::InvalidInput(format!("Invalid RPC URL: {e}")))?;
    let provider = JsonRpcClient::new(HttpTransport::new(url));

    let id_felt = parse_starterpack_id(&id)?;

    let selector = starknet::core::utils::get_selector_from_name("metadata")
        .map_err(|e| CliError::InvalidInput(format!("Invalid entrypoint: {e}")))?;

    formatter.info("Fetching info...");

    let result = provider
        .call(
            FunctionCall {
                contract_address: STARTERPACK_CONTRACT,
                entry_point_selector: selector,
                calldata: vec![id_felt],
            },
            BlockId::Tag(BlockTag::Latest),
        )
        .await
        .map_err(|e| CliError::TransactionFailed(format!("Info call failed: {e}")))?;

    // Decode ByteArray from felt array
    let byte_array = ByteArray::cairo_deserialize(&result, 0)
        .map_err(|e| CliError::InvalidInput(format!("Failed to decode ByteArray: {e}")))?;

    let json_str = byte_array
        .to_string()
        .map_err(|e| CliError::InvalidInput(format!("Invalid UTF-8 in metadata: {e}")))?;

    let metadata: StarterpackMetadata = serde_json::from_str(&json_str)
        .map_err(|e| CliError::InvalidInput(format!("Invalid JSON in metadata: {e}")))?;

    if config.cli.json_output {
        formatter.success(&metadata);
    } else {
        formatter.info(&format!("Starterpack #{id}:"));
        println!("  Name:        {}", metadata.name);
        println!("  Description: {}", metadata.description);
        println!("  Image:       {}", metadata.image_uri);

        if !metadata.items.is_empty() {
            println!("  Items:");
            for item in &metadata.items {
                println!("    - {}: {}", item.name, item.description);
            }
        }
    }

    Ok(())
}
