mod api;
mod commands;
mod config;
mod error;
mod output;

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
        /// Path to policy file (JSON)
        policy_file: String,

        /// RPC URL to use (overrides config)
        #[arg(long)]
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

    let result = match cli.command {
        Commands::GenerateKeypair => commands::generate::execute(&config, &*formatter).await,
        Commands::RegisterSession {
            policy_file,
            rpc_url,
        } => commands::register::execute(&config, &*formatter, policy_file, rpc_url).await,
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
            )
            .await
        }
        Commands::Status => commands::status::execute(&config, &*formatter).await,
        Commands::Clear { yes } => commands::clear::execute(&config, &*formatter, yes).await,
    };

    if let Err(e) = result {
        formatter.error(&e);
        std::process::exit(1);
    }
}
