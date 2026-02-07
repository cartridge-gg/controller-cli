use crate::error::CliError;
use crate::output::OutputFormatter;
use colored::*;

pub struct HumanFormatter;

impl HumanFormatter {
    pub fn new(use_colors: bool) -> Self {
        if !use_colors {
            colored::control::set_override(false);
        }
        Self
    }

    fn format_success_symbol(&self) -> String {
        "✓".green().to_string()
    }

    fn format_error_symbol(&self) -> String {
        "✗".red().to_string()
    }

    fn format_info_symbol(&self) -> String {
        "ℹ".blue().to_string()
    }

    fn format_warning_symbol(&self) -> String {
        "⚠".yellow().to_string()
    }
}

impl OutputFormatter for HumanFormatter {
    fn success(&self, data: &dyn erased_serde::Serialize) {
        println!(
            "{} {}",
            self.format_success_symbol(),
            "Success".green().bold()
        );

        // Try to format the data as pretty JSON
        if let Ok(json) = serde_json::to_value(data) {
            if let Ok(pretty) = serde_json::to_string_pretty(&json) {
                println!("\n{}", pretty);
            }
        }
    }

    fn error(&self, error: &CliError) {
        eprintln!("{} {}", self.format_error_symbol(), "Error".red().bold());
        eprintln!("{}", error.to_string().red());

        if let Some(hint) = error.recovery_hint() {
            eprintln!("\n{} {}", self.format_info_symbol(), hint.cyan());
        }
    }

    fn info(&self, message: &str) {
        println!("{} {}", self.format_info_symbol(), message);
    }

    fn warning(&self, message: &str) {
        println!("{} {}", self.format_warning_symbol(), message.yellow());
    }
}
