mod human;
mod json;

pub use human::HumanFormatter;
pub use json::JsonFormatter;

use crate::error::CliError;

pub trait OutputFormatter {
    fn success(&self, data: &dyn erased_serde::Serialize);
    fn error(&self, error: &CliError);
    fn info(&self, message: &str);
    fn warning(&self, message: &str);
}

pub fn create_formatter(use_json: bool, use_colors: bool) -> Box<dyn OutputFormatter> {
    if use_json {
        Box::new(JsonFormatter)
    } else {
        Box::new(HumanFormatter::new(use_colors))
    }
}
