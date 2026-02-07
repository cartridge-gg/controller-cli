use crate::{config::Config, error::Result, output::OutputFormatter};
use account_sdk::storage::{filestorage::FileSystemBackend, StorageBackend};
use serde::Serialize;
use std::path::PathBuf;

#[derive(Serialize)]
pub struct ClearOutput {
    pub message: String,
    pub cleared_path: String,
}

pub async fn execute(
    config: &Config,
    formatter: &dyn OutputFormatter,
    skip_confirm: bool,
) -> Result<()> {
    let storage_path = PathBuf::from(shellexpand::tilde(&config.session.storage_path).to_string());
    let mut backend = FileSystemBackend::new(storage_path.clone());

    if !skip_confirm && !config.cli.json_output {
        // In human mode, ask for confirmation
        println!(
            "This will delete all stored session data at: {}",
            storage_path.display()
        );
        println!("Are you sure? (y/N): ");

        let mut input = String::new();
        std::io::stdin().read_line(&mut input).ok();

        if !input.trim().eq_ignore_ascii_case("y") && !input.trim().eq_ignore_ascii_case("yes") {
            formatter.info("Cancelled.");
            return Ok(());
        }
    }

    backend
        .clear()
        .map_err(|e| crate::error::CliError::Storage(e.to_string()))?;

    let output = ClearOutput {
        message: "All session data cleared successfully.".to_string(),
        cleared_path: storage_path.display().to_string(),
    };

    formatter.success(&output);

    Ok(())
}
