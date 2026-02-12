use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    #[serde(default)]
    pub github: GithubConfig,
    #[serde(default)]
    pub dashboard: DashboardConfig,
    #[serde(default)]
    pub cache: CacheConfig,
    #[serde(default)]
    pub ui: UiConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GithubConfig {
    #[serde(default)]
    pub orgs: Vec<String>,
    #[serde(default)]
    pub users: Vec<String>,
    #[serde(default)]
    pub include_repos: Vec<String>,
    #[serde(default)]
    pub exclude_repos: Vec<String>,
    #[serde(default = "default_api_url")]
    pub api_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardConfig {
    #[serde(default = "default_refresh_interval")]
    pub refresh_interval_secs: u64,
    #[serde(default = "default_true")]
    pub show_draft_prs: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    #[serde(default = "default_cache_ttl")]
    pub ttl_secs: u64,
    #[serde(default)]
    pub dir: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    #[serde(default = "default_nav_width")]
    pub nav_width_percent: u16,
}

fn default_api_url() -> String {
    "https://api.github.com/graphql".to_string()
}
fn default_refresh_interval() -> u64 {
    300
}
fn default_true() -> bool {
    true
}
fn default_cache_ttl() -> u64 {
    600
}
fn default_nav_width() -> u16 {
    30
}

impl Default for GithubConfig {
    fn default() -> Self {
        Self {
            orgs: Vec::new(),
            users: Vec::new(),
            include_repos: Vec::new(),
            exclude_repos: Vec::new(),
            api_url: default_api_url(),
        }
    }
}

impl Default for DashboardConfig {
    fn default() -> Self {
        Self {
            refresh_interval_secs: default_refresh_interval(),
            show_draft_prs: true,
        }
    }
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            ttl_secs: default_cache_ttl(),
            dir: None,
        }
    }
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            nav_width_percent: default_nav_width(),
        }
    }
}

impl AppConfig {
    pub fn load(path: Option<&Path>) -> Result<Self> {
        if let Some(path) = path {
            let content = std::fs::read_to_string(path)
                .with_context(|| format!("Failed to read config file: {}", path.display()))?;
            let config: AppConfig =
                toml::from_str(&content).with_context(|| "Failed to parse config file")?;
            return Ok(config);
        }

        // Search candidate paths in order
        let mut candidates = Vec::new();

        // 1. ~/.config/ghdash/config.toml (standard XDG on all platforms)
        if let Some(home) = std::env::var_os("HOME") {
            candidates.push(PathBuf::from(home).join(".config/ghdash/config.toml"));
        }

        // 2. Platform-specific path from `directories` crate
        //    (macOS: ~/Library/Application Support/ghdash/)
        if let Some(proj_dirs) = ProjectDirs::from("", "", "ghdash") {
            candidates.push(proj_dirs.config_dir().join("config.toml"));
        }

        for config_path in &candidates {
            if config_path.exists() {
                let content = std::fs::read_to_string(config_path).with_context(|| {
                    format!("Failed to read config file: {}", config_path.display())
                })?;
                let config: AppConfig =
                    toml::from_str(&content).with_context(|| "Failed to parse config file")?;
                return Ok(config);
            }
        }

        // Fallback to default
        Ok(AppConfig::default())
    }

    pub fn cache_dir(&self) -> PathBuf {
        if let Some(ref dir) = self.cache.dir {
            return dir.clone();
        }
        if let Some(proj_dirs) = ProjectDirs::from("", "", "ghdash") {
            return proj_dirs.cache_dir().to_path_buf();
        }
        PathBuf::from(".cache/ghdash")
    }

    pub fn log_dir(&self) -> PathBuf {
        if let Some(proj_dirs) = ProjectDirs::from("", "", "ghdash") {
            return proj_dirs.data_dir().join("logs");
        }
        PathBuf::from(".local/share/ghdash/logs")
    }
}
