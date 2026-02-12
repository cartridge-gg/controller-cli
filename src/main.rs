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
}

#[derive(Subcommand)]
enum Commands {
    /// Generate and store a new session keypair
    Generate,

    /// Generate authorization URL for session registration
    Register {
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
    },

    /// Manually store session credentials from authorization
    Store {
        /// Base64-encoded session data
        session_data: Option<String>,

        /// Read session data from file
        #[arg(long)]
        from_file: Option<String>,
    },

    /// Display current session status and information
    Status,

    /// Clear all stored session data
    Clear {
        /// Skip confirmation prompt
        #[arg(long)]
        yes: bool,
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

        /// RPC URL to use (overrides config and stored session RPC)
        #[arg(long)]
        rpc_url: Option<String>,

        /// Force self-pay (don't use paymaster)
        #[arg(long)]
        no_paymaster: bool,
    },

    /// Look up controller addresses by usernames or usernames by addresses
    Lookup {
        /// Comma-separated usernames to resolve (e.g., 'shinobi,sensei')
        #[arg(long)]
        usernames: Option<String>,

        /// Comma-separated addresses to resolve (e.g., '0x123...,0x456...')
        #[arg(long)]
        addresses: Option<String>,
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

    let result = match cli.command {
        Commands::Generate => commands::generate::execute(&config, &*formatter).await,
        Commands::Register {
            preset,
            file,
            chain_id,
            rpc_url,
        } => {
            commands::register::execute(&config, &*formatter, preset, file, chain_id, rpc_url).await
        }
        Commands::Store {
            session_data,
            from_file,
        } => commands::store::execute(&config, &*formatter, session_data, from_file).await,
        Commands::Status => commands::status::execute(&config, &*formatter).await,
        Commands::Clear { yes } => commands::clear::execute(&config, &*formatter, yes).await,
        Commands::Execute {
            contract,
            entrypoint,
            calldata,
            file,
            wait,
            timeout,
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
                rpc_url,
                no_paymaster,
            )
            .await
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
