use std::io::Write;
use tempfile::NamedTempFile;

use ghdash::util::config::AppConfig;

#[test]
fn test_load_full_config() {
    let toml = r#"
[github]
orgs = ["my-org", "other-org"]
users = ["my-user"]
include_repos = ["important-*"]
exclude_repos = ["*-archived"]
api_url = "https://github.example.com/api/graphql"

[dashboard]
refresh_interval_secs = 120
show_draft_prs = false

[cache]
ttl_secs = 300

[ui]
nav_width_percent = 40
"#;
    let mut f = NamedTempFile::new().unwrap();
    f.write_all(toml.as_bytes()).unwrap();

    let config = AppConfig::load(Some(f.path())).unwrap();
    assert_eq!(config.github.orgs, vec!["my-org", "other-org"]);
    assert_eq!(config.github.users, vec!["my-user"]);
    assert_eq!(config.github.include_repos, vec!["important-*"]);
    assert_eq!(config.github.exclude_repos, vec!["*-archived"]);
    assert_eq!(
        config.github.api_url,
        "https://github.example.com/api/graphql"
    );
    assert_eq!(config.dashboard.refresh_interval_secs, 120);
    assert!(!config.dashboard.show_draft_prs);
    assert_eq!(config.cache.ttl_secs, 300);
    assert_eq!(config.ui.nav_width_percent, 40);
}

#[test]
fn test_load_partial_config_uses_defaults() {
    let toml = r#"
[github]
orgs = ["my-org"]
"#;
    let mut f = NamedTempFile::new().unwrap();
    f.write_all(toml.as_bytes()).unwrap();

    let config = AppConfig::load(Some(f.path())).unwrap();
    assert_eq!(config.github.orgs, vec!["my-org"]);
    assert!(config.github.users.is_empty());
    assert_eq!(config.github.api_url, "https://api.github.com/graphql");
    assert_eq!(config.dashboard.refresh_interval_secs, 300);
    assert!(config.dashboard.show_draft_prs);
    assert_eq!(config.cache.ttl_secs, 600);
    assert_eq!(config.ui.nav_width_percent, 30);
}

#[test]
fn test_load_empty_config_uses_all_defaults() {
    let toml = "";
    let mut f = NamedTempFile::new().unwrap();
    f.write_all(toml.as_bytes()).unwrap();

    let config = AppConfig::load(Some(f.path())).unwrap();
    assert!(config.github.orgs.is_empty());
    assert!(config.github.users.is_empty());
    assert_eq!(config.dashboard.refresh_interval_secs, 300);
}

#[test]
fn test_load_users_only_config() {
    let toml = r#"
[github]
users = ["alice", "bob"]
"#;
    let mut f = NamedTempFile::new().unwrap();
    f.write_all(toml.as_bytes()).unwrap();

    let config = AppConfig::load(Some(f.path())).unwrap();
    assert!(config.github.orgs.is_empty());
    assert_eq!(config.github.users, vec!["alice", "bob"]);
}

#[test]
fn test_load_nonexistent_file_fails() {
    let result = AppConfig::load(Some(std::path::Path::new("/nonexistent/path/config.toml")));
    assert!(result.is_err());
}

#[test]
fn test_load_invalid_toml_fails() {
    let mut f = NamedTempFile::new().unwrap();
    f.write_all(b"this is not [valid toml {{").unwrap();

    let result = AppConfig::load(Some(f.path()));
    assert!(result.is_err());
}

#[test]
fn test_default_config() {
    let config = AppConfig::default();
    assert!(config.github.orgs.is_empty());
    assert!(config.github.users.is_empty());
    assert!(config.github.include_repos.is_empty());
    assert!(config.github.exclude_repos.is_empty());
    assert_eq!(config.github.api_url, "https://api.github.com/graphql");
    assert_eq!(config.dashboard.refresh_interval_secs, 300);
    assert!(config.dashboard.show_draft_prs);
    assert_eq!(config.cache.ttl_secs, 600);
    assert!(config.cache.dir.is_none());
    assert_eq!(config.ui.nav_width_percent, 30);
}
