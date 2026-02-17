pub mod backend;
pub mod chunker;
pub mod embeddings;
pub mod hygiene;
pub mod lucid;
pub mod markdown;
pub mod none;
pub mod response_cache;
pub mod snapshot;
pub mod postgres;
pub mod traits;
pub mod vector;

pub use backend::{
    classify_memory_backend, default_memory_backend_key, memory_backend_profile,
    selectable_memory_backends, MemoryBackendKind, MemoryBackendProfile,
};
pub use lucid::LucidMemory;
pub use markdown::MarkdownMemory;
pub use none::NoneMemory;
pub use response_cache::ResponseCache;
pub use postgres::PostgresMemory;
pub use traits::Memory;
pub use traits::{MemoryCategory, MemoryEntry};

use crate::config::MemoryConfig;
use std::path::Path;

pub fn create_memory(
    config: &MemoryConfig,
    workspace_dir: &Path,
    _api_key: Option<&str>,
) -> anyhow::Result<Box<dyn Memory>> {
    // For SaaS, we mostly use Postgres injected via API handler.
    // CLI commands might use Markdown or None.
    // If backend is "postgres", we can't easily create it here without connection pool.
    // So we fallback to Markdown for CLI usage if configured as postgres?
    // Or we return error?
    // Let's fallback to Markdown.

    match classify_memory_backend(&config.backend) {
        MemoryBackendKind::Markdown | MemoryBackendKind::Unknown => Ok(Box::new(MarkdownMemory::new(workspace_dir))),
        MemoryBackendKind::None => Ok(Box::new(NoneMemory::new())),
        _ => Ok(Box::new(MarkdownMemory::new(workspace_dir))),
    }
}

pub fn create_memory_for_migration(
    _backend: &str,
    workspace_dir: &Path,
) -> anyhow::Result<Box<dyn Memory>> {
    Ok(Box::new(MarkdownMemory::new(workspace_dir)))
}

pub fn create_response_cache(config: &MemoryConfig, workspace_dir: &Path) -> Option<ResponseCache> {
    if !config.response_cache_enabled {
        return None;
    }
    match ResponseCache::new(
        workspace_dir,
        config.response_cache_ttl_minutes,
        config.response_cache_max_entries,
    ) {
        Ok(c) => Some(c),
        Err(_) => None,
    }
}
