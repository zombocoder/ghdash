use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Environment variable that selects the active profile at startup, overriding
/// the `active_profile` config field (but below an explicit `--profile` flag).
pub const PROFILE_ENV: &str = "GHDASH_PROFILE";

/// Name used for the implicit single profile when no `[[profiles]]` are declared.
pub const DEFAULT_PROFILE_NAME: &str = "default";

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
    /// Name of the profile to activate on startup. Overridden by the `--profile`
    /// flag and the `GHDASH_PROFILE` env var. If unset (or it names no known
    /// profile), the first profile is used.
    #[serde(default)]
    pub active_profile: Option<String>,
    /// Named profiles. When empty, the top-level `[github]`/`[dashboard]`/
    /// `[cache]`/`[ui]` config is treated as a single default profile named
    /// `default` — so existing single-context configs keep working unchanged.
    #[serde(default)]
    pub profiles: Vec<Profile>,
}

/// A named account/instance context: a full `AppConfig`-shaped body plus a
/// `name`. Each profile resolves its own token, `api_url`, and cache namespace,
/// so switching profiles never mixes credentials or cached data across accounts
/// or GitHub Enterprise instances.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
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
    /// Name of the environment variable holding this profile's token. Only the
    /// variable *name* is ever stored in config — never the token value itself.
    #[serde(default)]
    pub token_env: Option<String>,
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
            token_env: None,
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

    /// Base cache directory for this config: the explicit `cache.dir` override, or
    /// the platform cache dir. Per-profile namespacing is applied on top of this by
    /// [`AppConfig::profile_cache_dir`].
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

    /// Whether `[[profiles]]` are explicitly configured. When `false`, the
    /// top-level config is the single implicit `default` profile.
    pub fn has_explicit_profiles(&self) -> bool {
        !self.profiles.is_empty()
    }

    /// Resolve the profile list. When no `[[profiles]]` are configured, the
    /// top-level config is returned as a single profile named `default`, so
    /// existing single-context configs keep working unchanged (back-compat).
    pub fn profiles(&self) -> Vec<Profile> {
        if self.profiles.is_empty() {
            vec![Profile {
                name: DEFAULT_PROFILE_NAME.to_string(),
                github: self.github.clone(),
                dashboard: self.dashboard.clone(),
                cache: self.cache.clone(),
                ui: self.ui.clone(),
            }]
        } else {
            self.profiles.clone()
        }
    }

    /// Name of the profile to activate, applying the full selector precedence:
    ///
    /// 1. `override_name` — the `--profile` flag or `GHDASH_PROFILE` env var,
    /// 2. the `active_profile` config field,
    /// 3. the first profile in the list.
    ///
    /// A selector that names no known profile is ignored (falls through to the
    /// next source), so a stale flag/env/config value degrades to the first
    /// profile rather than failing.
    pub fn active_profile_name(&self, override_name: Option<&str>) -> String {
        let profiles = self.profiles();
        let known = |name: &str| profiles.iter().any(|p| p.name == name);

        if let Some(name) = override_name
            && known(name)
        {
            return name.to_string();
        }
        if let Some(name) = &self.active_profile
            && known(name)
        {
            return name.clone();
        }
        profiles
            .first()
            .map(|p| p.name.clone())
            .unwrap_or_else(|| DEFAULT_PROFILE_NAME.to_string())
    }

    /// Resolve the active [`Profile`] value using [`AppConfig::active_profile_name`].
    pub fn active_profile(&self, override_name: Option<&str>) -> Profile {
        let name = self.active_profile_name(override_name);
        self.profiles()
            .into_iter()
            .find(|p| p.name == name)
            .expect("active profile name always names a profile in the list")
    }

    /// Effective cache directory for `profile`, namespaced so profiles never read
    /// each other's cached data.
    ///
    /// Back-compat: when no `[[profiles]]` are configured, the single implicit
    /// `default` profile uses the base cache dir directly (no `/default` subdir),
    /// so existing installs keep their current cache location unchanged.
    pub fn profile_cache_dir(&self, profile: &Profile) -> PathBuf {
        let base = self.cache_dir();
        if self.has_explicit_profiles() {
            profile.cache_dir(&base)
        } else {
            base
        }
    }
}

/// Replace filesystem-hostile characters in a profile name so it is safe to use
/// as a cache subdirectory.
fn sanitize_name(name: &str) -> String {
    name.replace(['/', '\\', ':'], "_")
}

impl Profile {
    /// The profile body as an `AppConfig` (with no nested profiles), used by the
    /// data-fetch path which reads `github`/`dashboard`/`cache`/`ui`.
    pub fn to_app_config(&self) -> AppConfig {
        AppConfig {
            github: self.github.clone(),
            dashboard: self.dashboard.clone(),
            cache: self.cache.clone(),
            ui: self.ui.clone(),
            active_profile: None,
            profiles: Vec::new(),
        }
    }

    /// Per-profile cache directory: the profile's own `cache.dir` (or the shared
    /// `default_base`) namespaced under the sanitized profile name.
    pub fn cache_dir(&self, default_base: &Path) -> PathBuf {
        let base = self
            .cache
            .dir
            .clone()
            .unwrap_or_else(|| default_base.to_path_buf());
        base.join(sanitize_name(&self.name))
    }
}
