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
#[command(name = "controller-cli")]
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
    GenerateKeypair,

    /// Generate authorization URL for session registration
    RegisterSession {
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
    StoreSession {
        /// Base64-encoded session data
        session_data: Option<String>,

        /// Read session data from file
        #[arg(long)]
        from_file: Option<String>,
    },

    /// Execute a transaction using the active session
    Execute {
        /// Contract address
        #[arg(long)]
        contract: Option<String>,

        /// Entrypoint/function name
        #[arg(long)]
        entrypoint: Option<String>,

        /// Calldata as comma-separated hex values
        #[arg(long)]
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

    /// Display current session status and information
    Status,

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

    let result = match cli.command {
        Commands::GenerateKeypair => commands::generate::execute(&config, &*formatter).await,
        Commands::RegisterSession {
            preset,
            file,
            chain_id,
            rpc_url,
        } => {
            commands::register::execute(&config, &*formatter, preset, file, chain_id, rpc_url).await
        }
        Commands::StoreSession {
            session_data,
            from_file,
        } => commands::store::execute(&config, &*formatter, session_data, from_file).await,
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
        Commands::Status => commands::status::execute(&config, &*formatter).await,
        Commands::Clear { yes } => commands::clear::execute(&config, &*formatter, yes).await,
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
