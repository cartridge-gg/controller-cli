use crate::{config::Config, error::Result, output::OutputFormatter};

pub async fn execute(
    _config: &Config,
    formatter: &dyn OutputFormatter,
    _account: Option<&str>,
) -> Result<()> {
    formatter.info("Not yet implemented");
    Ok(())
}
