use crate::memory::{Memory, MemoryCategory, MemoryEntry};
use async_trait::async_trait;
use std::path::Path;

pub struct LucidMemory {}

impl LucidMemory {
    pub fn new(_workspace_dir: &Path, _local_memory: Box<dyn Memory>) -> Self {
        Self {}
    }
}

#[async_trait]
impl Memory for LucidMemory {
    fn name(&self) -> &str { "lucid" }
    async fn store(&self, _: &str, _: &str, _: MemoryCategory) -> anyhow::Result<()> { Ok(()) }
    async fn recall(&self, _: &str, _: usize) -> anyhow::Result<Vec<MemoryEntry>> { Ok(vec![]) }
    async fn get(&self, _: &str) -> anyhow::Result<Option<MemoryEntry>> { Ok(None) }
    async fn list(&self, _: Option<&MemoryCategory>) -> anyhow::Result<Vec<MemoryEntry>> { Ok(vec![]) }
    async fn forget(&self, _: &str) -> anyhow::Result<bool> { Ok(false) }
    async fn count(&self) -> anyhow::Result<usize> { Ok(0) }
    async fn health_check(&self) -> bool { true }
}
