mod json;
mod human;

pub use json::JsonFormatter;
pub use human::HumanFormatter;

use crate::error::CliError;
use serde::Serialize;

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

// Helper trait for making types work with erased_serde
pub trait IntoErased {
    fn into_erased(&self) -> &dyn erased_serde::Serialize;
}

impl<T: Serialize> IntoErased for T {
    fn into_erased(&self) -> &dyn erased_serde::Serialize {
        self
    }
}
