use crate::config::Config;
use crate::output::OutputFormatter;
use serde::Serialize;

#[derive(Serialize)]
struct ConfigEntry {
    key: String,
    value: String,
}

#[derive(Serialize)]
struct ConfigList {
    entries: Vec<ConfigEntry>,
}

pub async fn execute_set(
    formatter: &dyn OutputFormatter,
    key: String,
    value: String,
) -> Result<(), crate::error::CliError> {
    // Load config from file only (no env merge) so we persist file-level values
    let mut config = Config::load().map_err(|e| crate::error::CliError::Config(e.to_string()))?;

    config
        .set_by_alias(&key, &value)
        .map_err(|e| crate::error::CliError::Config(e.to_string()))?;

    config
        .save()
        .map_err(|e| crate::error::CliError::Config(e.to_string()))?;

    formatter.info(&format!("Set {key} = {value}"));

    Ok(())
}

pub async fn execute_get(
    formatter: &dyn OutputFormatter,
    json_output: bool,
    key: String,
) -> Result<(), crate::error::CliError> {
    // Show effective value (file + env merged)
    let mut config = Config::load().map_err(|e| crate::error::CliError::Config(e.to_string()))?;
    config.merge_from_env();

    let value = config
        .get_by_alias(&key)
        .map_err(|e| crate::error::CliError::Config(e.to_string()))?;

    if json_output {
        let entry = ConfigEntry {
            key: key.clone(),
            value,
        };
        formatter.success(&entry);
    } else {
        println!("{value}");
    }

    Ok(())
}

pub async fn execute_list(
    formatter: &dyn OutputFormatter,
    json_output: bool,
) -> Result<(), crate::error::CliError> {
    // Show effective values (file + env merged)
    let mut config = Config::load().map_err(|e| crate::error::CliError::Config(e.to_string()))?;
    config.merge_from_env();

    let entries: Vec<ConfigEntry> = Config::VALID_KEYS
        .iter()
        .map(|&key| {
            let value = config
                .get_by_alias(key)
                .unwrap_or_else(|_| "<error>".to_string());
            ConfigEntry {
                key: key.to_string(),
                value,
            }
        })
        .collect();

    if json_output {
        let list = ConfigList { entries };
        formatter.success(&list);
    } else {
        let max_key_len = entries.iter().map(|e| e.key.len()).max().unwrap_or(0);
        for entry in &entries {
            println!(
                "{:<width$}  {}",
                entry.key,
                entry.value,
                width = max_key_len
            );
        }
    }

    Ok(())
}
