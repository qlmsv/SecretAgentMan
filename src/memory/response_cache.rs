use std::path::Path;

pub struct ResponseCache {}

impl ResponseCache {
    pub fn new(
        _workspace_dir: &Path,
        _ttl_minutes: u32,
        _max_entries: usize,
    ) -> anyhow::Result<Self> {
        Ok(Self {})
    }

    pub fn get(&self, _key: &str) -> anyhow::Result<Option<String>> {
        Ok(None)
    }

    pub fn store(&self, _key: &str, _value: &str) -> anyhow::Result<()> {
        Ok(())
    }
}
