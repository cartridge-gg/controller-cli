use crate::error::CliError;
use crate::output::OutputFormatter;
use serde_json::json;

pub struct JsonFormatter;

impl OutputFormatter for JsonFormatter {
    fn success(&self, data: &dyn erased_serde::Serialize) {
        let output = json!({
            "status": "success",
            "data": data
        });
        println!("{}", serde_json::to_string_pretty(&output).unwrap());
    }

    fn error(&self, error: &CliError) {
        let mut output = json!({
            "status": "error",
            "error_code": error.error_code(),
            "message": error.to_string(),
        });

        if let Some(hint) = error.recovery_hint() {
            output["recovery_hint"] = json!(hint);
        }

        // Add detailed error info for specific error types
        if let CliError::PolicyViolation { message, details } = error {
            output["details"] = json!({
                "message": message,
                "details": details
            });
        }

        eprintln!("{}", serde_json::to_string_pretty(&output).unwrap());
    }

    fn info(&self, message: &str) {
        let output = json!({
            "status": "info",
            "message": message
        });
        println!("{}", serde_json::to_string_pretty(&output).unwrap());
    }

    fn warning(&self, message: &str) {
        let output = json!({
            "status": "warning",
            "message": message
        });
        println!("{}", serde_json::to_string_pretty(&output).unwrap());
    }
}
