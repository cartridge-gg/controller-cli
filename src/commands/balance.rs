use crate::config::Config;
use crate::error::{CliError, Result};
use crate::output::OutputFormatter;
use account_sdk::storage::{filestorage::FileSystemBackend, StorageBackend};
use serde::{Deserialize, Serialize};
use starknet::core::types::{BlockId, BlockTag, Felt, FunctionCall};
use starknet::providers::{jsonrpc::HttpTransport, JsonRpcClient, Provider};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

const CACHE_TTL_SECS: u64 = 30;

struct TokenInfo {
    address: &'static str,
    decimals: u8,
}

fn builtin_tokens() -> Vec<(&'static str, TokenInfo)> {
    vec![
        (
            "ETH",
            TokenInfo {
                address: "0x049D36570D4e46f48e99674bd3fcc84644DdD6b96F7C741B1562B82f9e004dC7",
                decimals: 18,
            },
        ),
        (
            "STRK",
            TokenInfo {
                address: "0x04718f5a0Fc34cC1AF16A1cdee98fFB20C31f5cD61D6Ab07201858f4287c938D",
                decimals: 18,
            },
        ),
        (
            "USDC",
            TokenInfo {
                address: "0x033068F6539f8e6e6b131e6B2B814e6c34A5224bC66947c47DaB9dFeE93b35fb",
                decimals: 6,
            },
        ),
        (
            "USD.e",
            TokenInfo {
                address: "0x053C91253BC9682c04929cA02ED00b3E423f6710D2ee7e0D5EBB06F3eCF368A8",
                decimals: 6,
            },
        ),
        (
            "LORDS",
            TokenInfo {
                address: "0x0124aeb495b947201f5faC96fD1138E326AD86195B98df6DEc9009158A533B49",
                decimals: 18,
            },
        ),
        (
            "SURVIVOR",
            TokenInfo {
                address: "0x042DD777885AD2C116be96d4D634abC90A26A790ffB5871E037Dd5Ae7d2Ec86B",
                decimals: 18,
            },
        ),
        (
            "WBTC",
            TokenInfo {
                address: "0x03Fe2b97C1Fd336E750087D68B9b867997Fd64a2661fF3ca5A7C771641e8e7AC",
                decimals: 8,
            },
        ),
    ]
}

/// Query a single token's balance and decimals
async fn query_token_balance(
    provider: Arc<JsonRpcClient<HttpTransport>>,
    sym: String,
    contract_address: Felt,
    account_address: Felt,
    known_decimals: Option<u8>,
) -> std::result::Result<BalanceOutput, String> {
    let balance_of_selector = starknet::core::utils::get_selector_from_name("balance_of").unwrap();

    let balance_call = FunctionCall {
        contract_address,
        entry_point_selector: balance_of_selector,
        calldata: vec![account_address],
    };

    let balance_result = provider
        .call(balance_call, BlockId::Tag(BlockTag::Latest))
        .await
        .map_err(|e| format!("Skipping {sym}: balance_of failed: {e}"))?;

    let (raw_low, raw_high) = match balance_result.len() {
        1 => (balance_result[0], Felt::ZERO),
        2.. => (balance_result[0], balance_result[1]),
        _ => return Err(format!("Skipping {sym}: unexpected balance_of response")),
    };

    let decimals = match known_decimals {
        Some(d) => d,
        None => {
            let decimals_selector =
                starknet::core::utils::get_selector_from_name("decimals").unwrap();
            let decimals_call = FunctionCall {
                contract_address,
                entry_point_selector: decimals_selector,
                calldata: vec![],
            };

            match provider
                .call(decimals_call, BlockId::Tag(BlockTag::Latest))
                .await
            {
                Ok(r) if !r.is_empty() => {
                    let val: u64 = r[0].try_into().unwrap_or(18);
                    val as u8
                }
                _ => 18,
            }
        }
    };

    let formatted = format_u256_balance(raw_low, raw_high, decimals);
    let raw_hex = if raw_high == Felt::ZERO {
        format!("0x{raw_low:x}")
    } else {
        format!("0x{raw_high:x}{:032x}", felt_to_u128(raw_low))
    };

    Ok(BalanceOutput {
        token: sym,
        balance: formatted,
        raw: raw_hex,
        contract: format!("0x{contract_address:x}"),
    })
}

/// Query ERC20 token balances for the active session account
pub async fn execute(
    config: &Config,
    formatter: &dyn OutputFormatter,
    symbol: Option<String>,
    chain_id: Option<String>,
    rpc_url: Option<String>,
) -> Result<()> {
    // Load session to get account address
    let storage_path = PathBuf::from(shellexpand::tilde(&config.session.storage_path).to_string());
    let backend = FileSystemBackend::new(storage_path.clone());

    let controller = backend
        .controller()
        .ok()
        .flatten()
        .ok_or(CliError::NoSession)?;

    let account_address = controller.address;

    // Resolve RPC URL
    let rpc_url = resolve_rpc_url(chain_id, rpc_url, config, formatter)?;

    // Check cache
    let cache_key = format!("0x{account_address:x}");
    if let Some(cached) = load_cache(&storage_path, &cache_key) {
        let results = filter_results(cached, &symbol);
        return output_results(config, formatter, &results);
    }

    let url = url::Url::parse(&rpc_url)
        .map_err(|e| CliError::InvalidInput(format!("Invalid RPC URL: {e}")))?;
    let provider = Arc::new(JsonRpcClient::new(HttpTransport::new(url)));

    // Build token list: built-in defaults + config overrides
    let mut tokens: BTreeMap<String, String> = BTreeMap::new();
    for (sym, info) in builtin_tokens() {
        tokens.insert(sym.to_string(), info.address.to_string());
    }
    for (sym, addr) in &config.tokens {
        tokens.insert(sym.clone(), addr.clone());
    }

    // Spawn all balance queries concurrently
    let mut handles = Vec::new();
    let token_order: Vec<String> = tokens.keys().cloned().collect();

    for (sym, addr_str) in &tokens {
        let contract_address = match Felt::from_hex(addr_str) {
            Ok(a) => a,
            Err(e) => {
                formatter.warning(&format!("Skipping {sym}: invalid address: {e}"));
                continue;
            }
        };

        let known_decimals = builtin_tokens()
            .iter()
            .find(|(s, _)| s.to_uppercase() == sym.to_uppercase())
            .map(|(_, info)| info.decimals);

        let provider = Arc::clone(&provider);
        let sym = sym.clone();
        handles.push(tokio::spawn(query_token_balance(
            provider,
            sym,
            contract_address,
            account_address,
            known_decimals,
        )));
    }

    // Collect results, preserving token order
    let query_results = futures::future::join_all(handles).await;
    let mut result_map: BTreeMap<String, BalanceOutput> = BTreeMap::new();
    for res in query_results {
        match res {
            Ok(Ok(output)) => {
                result_map.insert(output.token.clone(), output);
            }
            Ok(Err(warning)) => {
                formatter.warning(&warning);
            }
            Err(e) => {
                formatter.warning(&format!("Task failed: {e}"));
            }
        }
    }

    let all_results: Vec<BalanceOutput> = token_order
        .iter()
        .filter_map(|sym| result_map.remove(sym))
        .collect();

    // Save to cache (all tokens, before filtering)
    save_cache(&storage_path, &cache_key, &all_results);

    let results = filter_results(all_results, &symbol);
    output_results(config, formatter, &results)
}

/// Filter results: by symbol if specified, and skip zero balances when querying all
fn filter_results(results: Vec<BalanceOutput>, symbol: &Option<String>) -> Vec<BalanceOutput> {
    results
        .into_iter()
        .filter(|r| {
            if let Some(ref sym) = symbol {
                r.token.to_uppercase() == sym.to_uppercase()
            } else {
                // Skip zero balances when querying all tokens
                r.raw != "0x0"
            }
        })
        .collect()
}

fn output_results(
    config: &Config,
    formatter: &dyn OutputFormatter,
    results: &[BalanceOutput],
) -> Result<()> {
    if config.cli.json_output {
        formatter.success(&results);
    } else {
        for r in results {
            println!("{} {}", r.balance, r.token);
        }
    }
    Ok(())
}

// --- Cache ---

#[derive(Serialize, Deserialize)]
struct BalanceCache {
    timestamp: u64,
    balances: Vec<BalanceOutput>,
}

fn cache_path(storage_path: &std::path::Path, account: &str) -> PathBuf {
    storage_path.join(format!("balance_cache_{account}.json"))
}

fn load_cache(storage_path: &std::path::Path, account: &str) -> Option<Vec<BalanceOutput>> {
    let path = cache_path(storage_path, account);
    let content = std::fs::read_to_string(&path).ok()?;
    let cache: BalanceCache = serde_json::from_str(&content).ok()?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();

    if now - cache.timestamp <= CACHE_TTL_SECS {
        Some(cache.balances)
    } else {
        // Expired — clean up
        let _ = std::fs::remove_file(&path);
        None
    }
}

fn save_cache(storage_path: &std::path::Path, account: &str, balances: &[BalanceOutput]) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let cache = BalanceCache {
        timestamp: now,
        balances: balances.to_vec(),
    };

    if let Ok(json) = serde_json::to_string(&cache) {
        let _ = std::fs::write(cache_path(storage_path, account), json);
    }
}

// --- Formatting ---

fn felt_to_u128(f: Felt) -> u128 {
    let bytes = f.to_bytes_be();
    u128::from_be_bytes(bytes[16..32].try_into().unwrap())
}

/// Format a u256 balance (given as low/high felt pair) with decimal places.
/// Shows up to 6 decimal places.
fn format_u256_balance(low: Felt, high: Felt, decimals: u8) -> String {
    let low_val = felt_to_u128(low);
    let high_val = felt_to_u128(high);

    if decimals == 0 {
        if high_val == 0 {
            return low_val.to_string();
        }
        return format!("0x{high_val:x}{low_val:032x}");
    }

    if high_val == 0 {
        return format_u128_balance(low_val, decimals);
    }

    let combined = format!("{high_val:032x}{low_val:032x}");
    format!("0x{combined}")
}

/// Format a u128 balance with the given number of decimals (up to 6 visible decimal places)
fn format_u128_balance(value: u128, decimals: u8) -> String {
    if decimals == 0 {
        return value.to_string();
    }

    let display_decimals = std::cmp::min(decimals as usize, 6);
    let divisor = 10u128.pow(decimals as u32);
    let whole = value / divisor;
    let remainder = value % divisor;

    let padded = format!("{:0>width$}", remainder, width = decimals as usize);
    let truncated = &padded[..display_decimals];

    format!("{whole}.{truncated}")
}

/// Resolve RPC URL from chain_id, explicit rpc_url, or config
fn resolve_rpc_url(
    chain_id: Option<String>,
    rpc_url: Option<String>,
    config: &Config,
    formatter: &dyn OutputFormatter,
) -> Result<String> {
    if let Some(url) = rpc_url {
        return Ok(url);
    }

    if let Some(chain) = chain_id {
        match chain.as_str() {
            "SN_MAIN" => Ok("https://api.cartridge.gg/x/starknet/mainnet".to_string()),
            "SN_SEPOLIA" => Ok("https://api.cartridge.gg/x/starknet/sepolia".to_string()),
            _ => Err(CliError::InvalidInput(format!(
                "Unsupported chain ID '{chain}'. Supported chains: SN_MAIN, SN_SEPOLIA"
            ))),
        }
    } else if !config.session.rpc_url.is_empty() {
        Ok(config.session.rpc_url.clone())
    } else {
        formatter.warning("No --chain-id or --rpc-url specified, using SN_SEPOLIA by default");
        Ok("https://api.cartridge.gg/x/starknet/sepolia".to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BalanceOutput {
    token: String,
    balance: String,
    raw: String,
    contract: String,
}
