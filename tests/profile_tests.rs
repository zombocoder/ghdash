//! Tests for the profile switcher config layer: the `[[profiles]]` schema and
//! back-compat, the active-profile selector (flag/env/config precedence),
//! per-profile token resolution and host mapping, cache namespacing, and secret
//! hygiene (the token value is never serialized into config).

use std::io::Write;
use tempfile::NamedTempFile;

use ghdash::github::auth::{gh_hostname, resolve_profile_token};
use ghdash::util::config::AppConfig;

fn load(toml: &str) -> AppConfig {
    let mut f = NamedTempFile::new().unwrap();
    f.write_all(toml.as_bytes()).unwrap();
    AppConfig::load(Some(f.path())).unwrap()
}

// --- Back-compat: no [[profiles]] behaves exactly as today ---

#[test]
fn no_profiles_yields_single_default_profile() {
    let config = load(
        r#"
[github]
orgs = ["my-org"]
users = ["me"]
api_url = "https://api.github.com/graphql"
"#,
    );

    assert!(!config.has_explicit_profiles());
    let profiles = config.profiles();
    assert_eq!(profiles.len(), 1, "top-level config is the single profile");
    assert_eq!(profiles[0].name, "default");
    assert_eq!(profiles[0].github.orgs, vec!["my-org"]);
    assert_eq!(profiles[0].github.users, vec!["me"]);
    assert_eq!(config.active_profile_name(None), "default");
    // The top-level config still parses/behaves as before.
    assert_eq!(config.github.orgs, vec!["my-org"]);
}

#[test]
fn existing_config_without_token_env_parses() {
    // token_env is optional; omitting it must not break existing configs.
    let config = load(
        r#"
[github]
orgs = ["my-org"]
"#,
    );
    assert!(config.github.token_env.is_none());
    assert!(config.profiles.is_empty());
    assert!(!config.has_explicit_profiles());
}

// --- [[profiles]] schema, per-profile api_url (Enterprise) + token_env ---

#[test]
fn multiple_profiles_parse_with_per_profile_fields() {
    let config = load(
        r#"
active_profile = "acme-ent"

[[profiles]]
name = "work"
[profiles.github]
orgs = ["AITechCraft", "GKF-InCap"]
api_url = "https://api.github.com/graphql"
token_env = "GHDASH_TOKEN_WORK"

[[profiles]]
name = "acme-ent"
[profiles.github]
orgs = ["acme"]
api_url = "https://ghe.acme.corp/api/v3"
token_env = "GHDASH_TOKEN_ACME"
"#,
    );

    assert!(config.has_explicit_profiles());
    let profiles = config.profiles();
    assert_eq!(profiles.len(), 2);
    assert_eq!(config.active_profile_name(None), "acme-ent");

    let work = profiles.iter().find(|p| p.name == "work").unwrap();
    assert_eq!(work.github.orgs, vec!["AITechCraft", "GKF-InCap"]);
    assert_eq!(work.github.token_env.as_deref(), Some("GHDASH_TOKEN_WORK"));

    let acme = profiles.iter().find(|p| p.name == "acme-ent").unwrap();
    assert_eq!(acme.github.api_url, "https://ghe.acme.corp/api/v3");
    assert_eq!(acme.github.token_env.as_deref(), Some("GHDASH_TOKEN_ACME"));
}

// --- Active-profile selector: flag/env/config precedence + fallback ---

#[test]
fn active_profile_config_field_selects_profile() {
    let config = load(
        r#"
active_profile = "second"

[[profiles]]
name = "first"
[profiles.github]
orgs = ["a"]

[[profiles]]
name = "second"
[profiles.github]
orgs = ["b"]
"#,
    );
    assert_eq!(config.active_profile_name(None), "second");
    assert_eq!(config.active_profile(None).github.orgs, vec!["b"]);
}

#[test]
fn override_beats_config_active_profile() {
    // The `--profile` flag / GHDASH_PROFILE env (passed as `override_name`) wins
    // over the `active_profile` config field.
    let config = load(
        r#"
active_profile = "second"

[[profiles]]
name = "first"
[profiles.github]
orgs = ["a"]

[[profiles]]
name = "second"
[profiles.github]
orgs = ["b"]
"#,
    );
    assert_eq!(config.active_profile_name(Some("first")), "first");
    assert_eq!(config.active_profile(Some("first")).github.orgs, vec!["a"]);
}

#[test]
fn unknown_override_falls_through_to_config_then_first() {
    // An override that names no profile is ignored, falling through to the
    // config's active_profile.
    let config = load(
        r#"
active_profile = "second"

[[profiles]]
name = "first"
[profiles.github]
orgs = ["a"]

[[profiles]]
name = "second"
[profiles.github]
orgs = ["b"]
"#,
    );
    assert_eq!(
        config.active_profile_name(Some("does-not-exist")),
        "second",
        "unknown override falls through to active_profile"
    );

    // With no config active_profile either, both an unknown override and an
    // unset selector fall back to the first profile.
    let config = load(
        r#"
active_profile = "nope"

[[profiles]]
name = "first"
[profiles.github]
orgs = ["a"]

[[profiles]]
name = "second"
[profiles.github]
orgs = ["b"]
"#,
    );
    assert_eq!(config.active_profile_name(Some("also-nope")), "first");
    assert_eq!(config.active_profile_name(None), "first");
}

// --- gh --hostname derivation ---

#[test]
fn gh_hostname_maps_public_and_enterprise_hosts() {
    assert_eq!(
        gh_hostname("https://api.github.com/graphql").as_deref(),
        Some("github.com"),
        "public GitHub maps to github.com for `gh`"
    );
    assert_eq!(
        gh_hostname("https://ghe.acme.corp/api/v3").as_deref(),
        Some("ghe.acme.corp")
    );
    assert_eq!(
        gh_hostname("https://ghe.acme.corp:8443/api/v3").as_deref(),
        Some("ghe.acme.corp"),
        "port is stripped"
    );
    assert_eq!(gh_hostname("not-a-url").as_deref(), None);
}

// --- Token resolution order (token_env -> GITHUB_TOKEN -> gh) ---

#[test]
fn token_resolution_order() {
    // Run sequentially inside one test to control process-global env safely.
    let env_var = "GHDASH_TEST_PROFILE_TOKEN";
    let secret = "ghp_profile_env_secret_ABC123";
    let github_token_val = "ghp_github_token_secret_XYZ789";

    // Save originals so we leave the environment as we found it.
    let orig_env = std::env::var(env_var).ok();
    let orig_github = std::env::var("GITHUB_TOKEN").ok();
    let orig_gh = std::env::var("GH_TOKEN").ok();

    unsafe {
        // 1. token_env wins even when GITHUB_TOKEN is also set.
        std::env::set_var(env_var, secret);
        std::env::set_var("GITHUB_TOKEN", github_token_val);
        std::env::remove_var("GH_TOKEN");
    }
    let t = resolve_profile_token(Some(env_var), "https://api.github.com/graphql").unwrap();
    assert_eq!(t, secret, "token_env takes precedence");

    // 2. With no token_env, GITHUB_TOKEN is used (ahead of gh).
    unsafe {
        std::env::remove_var(env_var);
    }
    let t = resolve_profile_token(None, "https://api.github.com/graphql").unwrap();
    assert_eq!(t, github_token_val, "falls back to GITHUB_TOKEN");

    // 3. A token_env pointing at an unset var also falls through to GITHUB_TOKEN.
    let t = resolve_profile_token(
        Some("GHDASH_UNSET_VAR_NOPE"),
        "https://api.github.com/graphql",
    )
    .unwrap();
    assert_eq!(t, github_token_val);

    // Restore.
    unsafe {
        match orig_env {
            Some(v) => std::env::set_var(env_var, v),
            None => std::env::remove_var(env_var),
        }
        match orig_github {
            Some(v) => std::env::set_var("GITHUB_TOKEN", v),
            None => std::env::remove_var("GITHUB_TOKEN"),
        }
        if let Some(v) = orig_gh {
            std::env::set_var("GH_TOKEN", v);
        }
    }
}

// --- Per-profile cache namespace ---

#[test]
fn profiles_have_isolated_cache_dirs() {
    let config = load(
        r#"
[cache]
dir = "/tmp/ghdash-cache"

[[profiles]]
name = "work"
[profiles.github]
orgs = ["a"]

[[profiles]]
name = "personal"
[profiles.github]
orgs = ["b"]
"#,
    );
    let base = std::path::Path::new("/tmp/ghdash-cache");
    let profiles = config.profiles();
    let work = profiles.iter().find(|p| p.name == "work").unwrap();
    let personal = profiles.iter().find(|p| p.name == "personal").unwrap();

    // Low-level namespacing helper.
    assert_eq!(work.cache_dir(base), base.join("work"));
    assert_eq!(personal.cache_dir(base), base.join("personal"));

    // Config-level resolution namespaces under the shared cache.dir base.
    let work_dir = config.profile_cache_dir(work);
    let personal_dir = config.profile_cache_dir(personal);
    assert_eq!(work_dir, base.join("work"));
    assert_eq!(personal_dir, base.join("personal"));
    assert_ne!(
        work_dir, personal_dir,
        "profiles must not share a cache namespace"
    );
}

#[test]
fn sanitizes_profile_name_for_cache_dir() {
    let config = load(
        r#"
[[profiles]]
name = "ent/team:one"
[profiles.github]
orgs = ["a"]
"#,
    );
    let profile = &config.profiles()[0];
    let base = std::path::Path::new("/tmp/ghdash-cache");
    // Path separators / colon are replaced so the name is a single safe segment.
    assert_eq!(profile.cache_dir(base), base.join("ent_team_one"));
}

#[test]
fn back_compat_cache_dir_has_no_default_subdir() {
    // With no [[profiles]], the single implicit `default` profile keeps the exact
    // top-level cache location — no `/default` namespacing — so existing installs
    // are unaffected.
    let config = load(
        r#"
[cache]
dir = "/tmp/ghdash-cache"

[github]
orgs = ["a"]
"#,
    );
    let profile = config.active_profile(None);
    assert_eq!(
        config.profile_cache_dir(&profile),
        std::path::PathBuf::from("/tmp/ghdash-cache"),
        "back-compat: default profile uses the base cache dir directly"
    );
}

#[test]
fn profile_can_override_its_own_cache_dir() {
    let config = load(
        r#"
[cache]
dir = "/tmp/shared-base"

[[profiles]]
name = "work"
[profiles.github]
orgs = ["a"]
[profiles.cache]
dir = "/var/ghdash/work-cache"
"#,
    );
    let profile = &config.profiles()[0];
    // The profile's own cache.dir wins over the shared base, still namespaced by name.
    assert_eq!(
        config.profile_cache_dir(profile),
        std::path::PathBuf::from("/var/ghdash/work-cache").join("work")
    );
}

// --- Secret hygiene: no token value ever lands in serialized config ---

#[test]
fn token_never_serialized_into_config() {
    // Only the env var NAME is ever stored; the token value lives in the env.
    let toml = r#"
[[profiles]]
name = "work"
[profiles.github]
orgs = ["a"]
token_env = "GHDASH_TOKEN_WORK"
"#;
    let config = load(toml);
    let secret = "ghp_this_must_never_appear_0000";

    let serialized = toml::to_string(&config).unwrap();
    assert!(
        !serialized.contains(secret),
        "serialized config must not contain any token value"
    );
    // The env var NAME is fine to persist; the secret is not.
    assert!(serialized.contains("GHDASH_TOKEN_WORK"));

    // Debug formatting of the whole config must not leak a token either.
    let dbg = format!("{config:?}");
    assert!(!dbg.contains(secret));
}
