use std::path::Path;
use anyhow::Result;

pub fn export_snapshot(_workspace_dir: &Path) -> Result<()> { Ok(()) }
pub fn should_hydrate(_workspace_dir: &Path) -> bool { false }
pub fn hydrate_from_snapshot(_workspace_dir: &Path) -> Result<usize> { Ok(0) }
