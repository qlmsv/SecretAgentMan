use crate::config::Config;
use crate::MigrateCommands;
use anyhow::Result;

pub async fn handle_command(_cmd: MigrateCommands, _config: &Config) -> Result<()> {
    anyhow::bail!("Migration disabled for SaaS")
}
