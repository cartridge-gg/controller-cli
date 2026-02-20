mod api;
mod commands;
mod config;
mod error;
mod output;
mod presets;
mod version;

use clap::{Parser, Subcommand};
use config::Config;
use output::create_formatter;

#[derive(Parser)]
#[command(name = "controller")]
#[command(about = "CLI for Cartridge Controller session management", long_about = None)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Output in JSON format (for LLMs)
    #[arg(long, global = true, env = "CARTRIDGE_JSON_OUTPUT")]
    json: bool,

    /// Disable colored output
    #[arg(long, global = true)]
    no_color: bool,

    /// Account label for multi-account support (e.g., 'player1')
    #[arg(long, global = true)]
    account: Option<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// Manage session lifecycle
    Session {
        #[command(subcommand)]
        command: SessionCommands,
    },

    /// Execute a transaction using the active session
    Execute {
        /// Contract address (positional)
        contract: Option<String>,

        /// Entrypoint/function name (positional)
        entrypoint: Option<String>,

        /// Calldata as comma-separated hex values (positional)
        calldata: Option<String>,

        /// Read calls from JSON file
        #[arg(long)]
        file: Option<String>,

        /// Wait for transaction confirmation
        #[arg(long)]
        wait: bool,

        /// Timeout in seconds when waiting
        #[arg(long, default_value = "300")]
        timeout: u64,

        /// Chain ID (e.g., 'SN_MAIN' or 'SN_SEPOLIA') - auto-selects RPC URL
        #[arg(long, conflicts_with = "rpc_url")]
        chain_id: Option<String>,

        /// RPC URL to use (overrides config and stored session RPC)
        #[arg(long, conflicts_with = "chain_id")]
        rpc_url: Option<String>,

        /// Force self-pay (don't use paymaster)
        #[arg(long)]
        no_paymaster: bool,
    },

    /// Execute a read-only call to a contract
    Call {
        /// Contract address (positional)
        contract: Option<String>,

        /// Entrypoint/function name (positional)
        entrypoint: Option<String>,

        /// Calldata as comma-separated hex values (positional)
        calldata: Option<String>,

        /// Read calls from JSON file
        #[arg(long)]
        file: Option<String>,

        /// Chain ID (e.g., 'SN_MAIN' or 'SN_SEPOLIA') - auto-selects RPC URL
        #[arg(long, conflicts_with = "rpc_url")]
        chain_id: Option<String>,

        /// RPC URL to use (overrides config)
        #[arg(long, conflicts_with = "chain_id")]
        rpc_url: Option<String>,

        /// Block ID to query (latest, pending, block number, or block hash)
        #[arg(long)]
        block_id: Option<String>,
    },

    /// Get transaction status and details
    Transaction {
        /// Transaction hash
        hash: String,

        /// Chain ID (e.g., 'SN_MAIN' or 'SN_SEPOLIA') - auto-selects RPC URL
        #[arg(long, conflicts_with = "rpc_url")]
        chain_id: Option<String>,

        /// RPC URL to use (overrides config)
        #[arg(long, conflicts_with = "chain_id")]
        rpc_url: Option<String>,

        /// Wait for transaction to be confirmed
        #[arg(long)]
        wait: bool,

        /// Timeout in seconds when waiting
        #[arg(long, default_value = "300")]
        timeout: u64,
    },

    /// Get transaction receipt
    Receipt {
        /// Transaction hash
        hash: String,

        /// Chain ID (e.g., 'SN_MAIN' or 'SN_SEPOLIA') - auto-selects RPC URL
        #[arg(long, conflicts_with = "rpc_url")]
        chain_id: Option<String>,

        /// RPC URL to use (overrides config)
        #[arg(long, conflicts_with = "chain_id")]
        rpc_url: Option<String>,

        /// Wait for transaction to be confirmed
        #[arg(long)]
        wait: bool,

        /// Timeout in seconds when waiting
        #[arg(long, default_value = "300")]
        timeout: u64,
    },

    /// Manage CLI configuration
    Config {
        #[command(subcommand)]
        command: ConfigCommands,
    },

    /// Query ERC20 token balances for the active session account
    Balance {
        /// Token symbol (e.g., 'eth', 'strk'). If omitted, queries all known tokens
        symbol: Option<String>,

        /// Chain ID (e.g., 'SN_MAIN' or 'SN_SEPOLIA') - auto-selects RPC URL
        #[arg(long, conflicts_with = "rpc_url")]
        chain_id: Option<String>,

        /// RPC URL to use (overrides config)
        #[arg(long, conflicts_with = "chain_id")]
        rpc_url: Option<String>,
    },

    /// Display the username associated with the active session account
    Username,

    /// Look up controller addresses by usernames or usernames by addresses
    Lookup {
        /// Comma-separated usernames to resolve (e.g., 'shinobi,sensei')
        #[arg(long)]
        usernames: Option<String>,

        /// Comma-separated addresses to resolve (e.g., '0x123...,0x456...')
        #[arg(long)]
        addresses: Option<String>,
    },

    /// Quote and purchase starterpacks
    Starterpack {
        #[command(subcommand)]
        command: StarterpackCommands,
    },
}

#[derive(Subcommand)]
enum ConfigCommands {
    /// Set a configuration value
    Set {
        /// Config key (e.g., chain-id, rpc-url)
        key: String,
        /// Value to set
        value: String,
    },
    /// Get a configuration value
    Get {
        /// Config key (e.g., chain-id, rpc-url)
        key: String,
    },
    /// List all configuration values
    List,
}

#[derive(Subcommand)]
enum StarterpackCommands {
    /// Get a quote for a starterpack (payment token and amount)
    Quote {
        /// Starterpack ID
        id: String,

        /// Quantity to purchase
        #[arg(long, default_value = "1")]
        quantity: u32,

        /// Chain ID (e.g., 'SN_MAIN' or 'SN_SEPOLIA') - auto-selects RPC URL
        #[arg(long, conflicts_with = "rpc_url")]
        chain_id: Option<String>,

        /// RPC URL to use (overrides config)
        #[arg(long, conflicts_with = "chain_id")]
        rpc_url: Option<String>,
    },

    /// Get info for a starterpack
    Info {
        /// Starterpack ID
        id: String,

        /// Chain ID (e.g., 'SN_MAIN' or 'SN_SEPOLIA') - auto-selects RPC URL
        #[arg(long, conflicts_with = "rpc_url")]
        chain_id: Option<String>,

        /// RPC URL to use (overrides config)
        #[arg(long, conflicts_with = "chain_id")]
        rpc_url: Option<String>,
    },

    /// Purchase a starterpack
    Purchase {
        /// Starterpack ID
        id: String,

        /// Recipient address (defaults to current controller)
        #[arg(long)]
        recipient: Option<String>,

        /// Quantity to purchase
        #[arg(long, default_value = "1")]
        quantity: u32,

        /// Open a UI for purchase (default)
        #[arg(long, group = "mode")]
        ui: bool,

        /// Execute purchase directly from Controller wallet
        #[arg(long, group = "mode")]
        direct: bool,

        /// Chain ID (e.g., 'SN_MAIN' or 'SN_SEPOLIA') - auto-selects RPC URL
        #[arg(long, conflicts_with = "rpc_url")]
        chain_id: Option<String>,

        /// RPC URL to use (overrides config)
        #[arg(long, conflicts_with = "chain_id")]
        rpc_url: Option<String>,

        /// Wait for transaction confirmation (direct mode only)
        #[arg(long)]
        wait: bool,

        /// Timeout in seconds when waiting (direct mode only)
        #[arg(long, default_value = "300")]
        timeout: u64,

        /// Force self-pay, don't use paymaster (direct mode only)
        #[arg(long)]
        no_paymaster: bool,
    },
}

#[derive(Subcommand)]
enum SessionCommands {
    /// Generate keypair and authorize a new session
    Auth {
        /// Preset name (e.g., 'loot-survivor')
        #[arg(long, conflicts_with = "file")]
        preset: Option<String>,

        /// Path to local policy file (JSON)
        #[arg(long, conflicts_with = "preset")]
        file: Option<String>,

        /// Chain ID (e.g., 'SN_MAIN' or 'SN_SEPOLIA') - auto-selects RPC URL
        #[arg(long, conflicts_with = "rpc_url")]
        chain_id: Option<String>,

        /// RPC URL to use (overrides config)
        #[arg(long, conflicts_with = "chain_id")]
        rpc_url: Option<String>,

        /// Overwrite existing session without confirmation
        #[arg(long)]
        overwrite: bool,
    },

    /// Display current session status and information
    Status,

    /// List sessions
    List {
        /// Chain ID (e.g., 'SN_MAIN' or 'SN_SEPOLIA') - defaults to session chain
        #[arg(long)]
        chain_id: Option<String>,

        /// Number of sessions per page
        #[arg(long, default_value = "10")]
        limit: u32,

        /// Page number (starting from 1)
        #[arg(long, default_value = "1")]
        page: u32,
    },

    /// Revoke an active session (onchain)
    Revoke,

    /// Clear all stored session data
    Clear {
        /// Skip confirmation prompt
        #[arg(long)]
        yes: bool,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    // Load config and merge with environment
    let mut config = Config::load().unwrap_or_default();
    config.merge_from_env();

    // Override config with CLI flags
    if cli.json {
        config.cli.json_output = true;
    }
    if cli.no_color {
        config.cli.use_colors = false;
    }

    let formatter = create_formatter(config.cli.json_output, config.cli.use_colors);

    // Start version check in background (non-blocking)
    let update_check = tokio::spawn(version::check_for_update());

    let account = cli.account;

    // Validate account name early, before any command uses it
    if let Some(ref name) = account {
        if let Err(e) = Config::validate_account_name(name) {
            formatter.error(&crate::error::CliError::InvalidInput(e));
            std::process::exit(1);
        }
    }

    let result = match cli.command {
        Commands::Session { command } => match command {
            SessionCommands::Auth {
                preset,
                file,
                chain_id,
                rpc_url,
                overwrite,
            } => {
                commands::session::authorize::execute(
                    &config,
                    &*formatter,
                    preset,
                    file,
                    chain_id,
                    rpc_url,
                    overwrite,
                    account.as_deref(),
                )
                .await
            }
            SessionCommands::Status => {
                commands::status::execute(&config, &*formatter, account.as_deref()).await
            }
            SessionCommands::List {
                chain_id,
                limit,
                page,
            } => {
                commands::session::list::execute(
                    &config,
                    &*formatter,
                    chain_id,
                    limit,
                    page,
                    account.as_deref(),
                )
                .await
            }
            SessionCommands::Revoke => {
                commands::session::revoke::execute(&config, &*formatter, account.as_deref())
                    .await
            }
            SessionCommands::Clear { yes } => {
                commands::clear::execute(&config, &*formatter, yes, account.as_deref()).await
            }
        },
        Commands::Config { command } => match command {
            ConfigCommands::Set { key, value } => {
                commands::config_cmd::execute_set(&*formatter, key, value).await
            }
            ConfigCommands::Get { key } => {
                commands::config_cmd::execute_get(&*formatter, config.cli.json_output, key).await
            }
            ConfigCommands::List => {
                commands::config_cmd::execute_list(&*formatter, config.cli.json_output).await
            }
        },
        Commands::Execute {
            contract,
            entrypoint,
            calldata,
            file,
            wait,
            timeout,
            chain_id,
            rpc_url,
            no_paymaster,
        } => {
            commands::execute::execute(
                &config,
                &*formatter,
                contract,
                entrypoint,
                calldata,
                file,
                wait,
                timeout,
                chain_id,
                rpc_url,
                no_paymaster,
                account.as_deref(),
            )
            .await
        }
        Commands::Balance {
            symbol,
            chain_id,
            rpc_url,
        } => {
            commands::balance::execute(
                &config,
                &*formatter,
                symbol,
                chain_id,
                rpc_url,
                account.as_deref(),
            )
            .await
        }
        Commands::Username => {
            commands::username::execute(&config, &*formatter, account.as_deref()).await
        }
        Commands::Lookup {
            usernames,
            addresses,
        } => commands::lookup::execute(&config, &*formatter, usernames, addresses).await,
        Commands::Call {
            contract,
            entrypoint,
            calldata,
            file,
            chain_id,
            rpc_url,
            block_id,
        } => {
            commands::call::execute(
                &config,
                &*formatter,
                contract,
                entrypoint,
                calldata,
                file,
                chain_id,
                rpc_url,
                block_id,
            )
            .await
        }
        Commands::Transaction {
            hash,
            chain_id,
            rpc_url,
            wait,
            timeout,
        } => {
            commands::transaction::execute(
                &config,
                &*formatter,
                hash,
                chain_id,
                rpc_url,
                wait,
                timeout,
            )
            .await
        }
        Commands::Receipt {
            hash,
            chain_id,
            rpc_url,
            wait,
            timeout,
        } => {
            commands::receipt::execute(&config, &*formatter, hash, chain_id, rpc_url, wait, timeout)
                .await
        }
        Commands::Starterpack { command } => match command {
            StarterpackCommands::Quote {
                id,
                quantity,
                chain_id,
                rpc_url,
            } => {
                commands::starterpack::quote::execute(
                    &config,
                    &*formatter,
                    id,
                    quantity,
                    chain_id,
                    rpc_url,
                )
                .await
            }
            StarterpackCommands::Info {
                id,
                chain_id,
                rpc_url,
            } => {
                commands::starterpack::info::execute(&config, &*formatter, id, chain_id, rpc_url)
                    .await
            }
            StarterpackCommands::Purchase {
                id,
                recipient,
                quantity,
                ui,
                direct,
                chain_id,
                rpc_url,
                wait,
                timeout,
                no_paymaster,
            } => {
                commands::starterpack::purchase::execute(
                    &config,
                    &*formatter,
                    id,
                    recipient,
                    quantity,
                    ui,
                    direct,
                    chain_id,
                    rpc_url,
                    wait,
                    timeout,
                    no_paymaster,
                    account.as_deref(),
                )
                .await
            }
        },
    };

    if let Err(e) = result {
        formatter.error(&e);
        // Still show update warning on error
        if let Ok(Some(msg)) = update_check.await {
            formatter.warning(&msg);
        }
        std::process::exit(1);
    }

    // Show update warning after successful command output
    if let Ok(Some(msg)) = update_check.await {
        formatter.warning(&msg);
    }
}
