use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing::{debug, warn};

#[derive(Debug, Clone)]
pub struct CacheStore {
    dir: PathBuf,
    ttl_secs: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct CacheEntry<T> {
    timestamp: chrono::DateTime<chrono::Utc>,
    data: T,
}

impl CacheStore {
    pub fn new(dir: PathBuf, ttl_secs: u64) -> Self {
        Self { dir, ttl_secs }
    }

    fn path_for_key(&self, key: &str) -> PathBuf {
        // Sanitize key for filesystem
        let safe_key = key.replace(['/', '\\'], "_");
        self.dir.join(format!("{safe_key}.json"))
    }

    pub fn get<T: for<'de> Deserialize<'de>>(&self, key: &str) -> Option<T> {
        let path = self.path_for_key(key);
        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => return None,
        };

        let entry: CacheEntry<T> = match serde_json::from_str(&content) {
            Ok(e) => e,
            Err(e) => {
                warn!(key = key, error = %e, "Failed to parse cache entry");
                return None;
            }
        };

        let age = chrono::Utc::now()
            .signed_duration_since(entry.timestamp)
            .num_seconds();

        if age < 0 || age as u64 > self.ttl_secs {
            debug!(key = key, age = age, "Cache entry expired");
            return None;
        }

        debug!(key = key, age = age, "Cache hit");
        Some(entry.data)
    }

    pub fn set<T: Serialize>(&self, key: &str, data: &T) -> Result<()> {
        std::fs::create_dir_all(&self.dir)
            .with_context(|| format!("Failed to create cache directory: {}", self.dir.display()))?;

        let entry = CacheEntry {
            timestamp: chrono::Utc::now(),
            data,
        };

        let content = serde_json::to_string(&entry).context("Failed to serialize cache entry")?;
        let path = self.path_for_key(key);
        std::fs::write(&path, content)
            .with_context(|| format!("Failed to write cache file: {}", path.display()))?;

        debug!(key = key, "Cache set");
        Ok(())
    }

    #[allow(dead_code)]
    pub fn invalidate(&self, key: &str) -> Result<()> {
        let path = self.path_for_key(key);
        if path.exists() {
            std::fs::remove_file(&path)
                .with_context(|| format!("Failed to remove cache file: {}", path.display()))?;
            debug!(key = key, "Cache invalidated");
        }
        Ok(())
    }

    pub fn invalidate_all(&self) -> Result<()> {
        if self.dir.exists() {
            for entry in std::fs::read_dir(&self.dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.extension().is_some_and(|ext| ext == "json") {
                    std::fs::remove_file(&path)?;
                }
            }
            debug!("All cache entries invalidated");
        }
        Ok(())
    }
}
