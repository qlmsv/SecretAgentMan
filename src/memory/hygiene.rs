use crate::config::MemoryConfig;
use std::path::Path;
use anyhow::Result;

pub fn run_if_due(_config: &MemoryConfig, _workspace_dir: &Path) -> Result<()> { Ok(()) }
