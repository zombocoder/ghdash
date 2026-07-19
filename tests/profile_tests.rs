//! Tests for the profile switcher: config schema + back-compat, per-profile token
//! resolution and host mapping, cache namespacing, the modal picker reducer, and
//! secret hygiene (no token leaks in serialized/rendered output).

use std::io::Write;
use tempfile::NamedTempFile;

use ghdash::app::actions::Action;
use ghdash::app::state::{AppState, ProfileSummary};
use ghdash::app::update::update;
use ghdash::github::auth::{gh_hostname, resolve_profile_token};
use ghdash::util::config::AppConfig;

fn load(toml: &str) -> AppConfig {
    let mut f = NamedTempFile::new().unwrap();
    f.write_all(toml.as_bytes()).unwrap();
    AppConfig::load(Some(f.path())).unwrap()
}

fn picker_state() -> AppState {
    let mut state = AppState::new("me".into(), vec!["org-a".into()]);
    state.set_profiles(
        vec![
            ProfileSummary {
                name: "work".into(),
                scope_count: 2,
                host: "github.com".into(),
            },
            ProfileSummary {
                name: "personal".into(),
                scope_count: 1,
                host: "github.com".into(),
            },
            ProfileSummary {
                name: "acme-ent".into(),
                scope_count: 1,
                host: "ghe.acme.corp".into(),
            },
        ],
        "work".into(),
    );
    state
}

// --- AC6: back-compat (no [[profiles]] behaves exactly as today) ---

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

    let profiles = config.profiles();
    assert_eq!(profiles.len(), 1, "top-level config is the single profile");
    assert_eq!(profiles[0].name, "default");
    assert_eq!(profiles[0].github.orgs, vec!["my-org"]);
    assert_eq!(profiles[0].github.users, vec!["me"]);
    assert_eq!(config.active_profile_name(), "default");
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
}

// --- AC1/AC4: [[profiles]] schema, per-profile api_url (Enterprise) ---

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

    let profiles = config.profiles();
    assert_eq!(profiles.len(), 2);
    assert_eq!(config.active_profile_name(), "acme-ent");

    let work = profiles.iter().find(|p| p.name == "work").unwrap();
    assert_eq!(work.github.orgs, vec!["AITechCraft", "GKF-InCap"]);
    assert_eq!(work.github.token_env.as_deref(), Some("GHDASH_TOKEN_WORK"));
    assert_eq!(work.scope_count(), 2);

    let acme = profiles.iter().find(|p| p.name == "acme-ent").unwrap();
    assert_eq!(acme.github.api_url, "https://ghe.acme.corp/api/v3");
    assert_eq!(acme.host(), "ghe.acme.corp");
}

#[test]
fn active_profile_falls_back_to_first_when_unset_or_unknown() {
    let config = load(
        r#"
active_profile = "does-not-exist"

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
    assert_eq!(config.active_profile_name(), "first");
}

// --- AC4: gh --hostname derivation ---

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

// --- AC4: token resolution order (token_env -> GITHUB_TOKEN -> gh) ---

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

// --- AC5: per-profile cache namespace ---

#[test]
fn profiles_have_isolated_cache_dirs() {
    let config = load(
        r#"
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

    let work_dir = work.cache_dir(base);
    let personal_dir = personal.cache_dir(base);

    assert_eq!(work_dir, base.join("work"));
    assert_eq!(personal_dir, base.join("personal"));
    assert_ne!(
        work_dir, personal_dir,
        "profiles must not share a cache namespace"
    );
}

// --- AC1: `p` opens the picker; active profile marked ---

#[test]
fn toggle_opens_picker_on_active_profile() {
    let mut state = picker_state();
    assert!(!state.profile_picker_active);

    update(&mut state, Action::ToggleProfilePicker);
    assert!(state.profile_picker_active);
    // Cursor starts on the active profile ("work" is index 0 here).
    assert_eq!(state.profile_picker_cursor, 0);
    assert_eq!(state.filtered_profiles().len(), 3);

    update(&mut state, Action::ToggleProfilePicker);
    assert!(!state.profile_picker_active, "toggles closed");
}

// --- AC2: type-to-filter, navigate, cancel ---

#[test]
fn typing_filters_profiles_case_insensitive() {
    let mut state = picker_state();
    update(&mut state, Action::ToggleProfilePicker);

    update(&mut state, Action::ProfilePickerInput('A')); // uppercase
    update(&mut state, Action::ProfilePickerInput('c'));
    let filtered = state.filtered_profiles();
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].name, "acme-ent");
    assert_eq!(state.profile_picker_cursor, 0, "cursor resets on input");

    update(&mut state, Action::ProfilePickerBackspace);
    update(&mut state, Action::ProfilePickerBackspace);
    assert_eq!(state.filtered_profiles().len(), 3);
}

#[test]
fn navigation_clamps_within_filtered_list() {
    let mut state = picker_state();
    update(&mut state, Action::ToggleProfilePicker);

    update(&mut state, Action::ProfilePickerDown);
    assert_eq!(state.profile_picker_cursor, 1);
    update(&mut state, Action::ProfilePickerDown);
    assert_eq!(state.profile_picker_cursor, 2);
    update(&mut state, Action::ProfilePickerDown);
    assert_eq!(state.profile_picker_cursor, 2, "clamped at end");

    update(&mut state, Action::ProfilePickerUp);
    update(&mut state, Action::ProfilePickerUp);
    update(&mut state, Action::ProfilePickerUp);
    assert_eq!(state.profile_picker_cursor, 0, "clamped at start");
}

#[test]
fn cancel_closes_without_switching() {
    let mut state = picker_state();
    update(&mut state, Action::ToggleProfilePicker);
    update(&mut state, Action::ProfilePickerDown);
    update(&mut state, Action::ProfilePickerCancel);

    assert!(!state.profile_picker_active);
    assert!(state.profile_picker_query.is_empty());
    assert!(state.pending_profile_switch.is_none());
}

// --- AC3: Enter requests a switch to the selected profile ---

#[test]
fn confirm_requests_switch_to_selected_profile() {
    let mut state = picker_state();
    update(&mut state, Action::ToggleProfilePicker);
    update(&mut state, Action::ProfilePickerDown); // -> "personal"
    update(&mut state, Action::ProfilePickerConfirm);

    assert!(!state.profile_picker_active);
    assert_eq!(state.pending_profile_switch.as_deref(), Some("personal"));
}

#[test]
fn confirm_on_active_profile_is_a_noop() {
    let mut state = picker_state();
    update(&mut state, Action::ToggleProfilePicker); // cursor on active "work"
    update(&mut state, Action::ProfilePickerConfirm);

    assert!(!state.profile_picker_active);
    assert!(
        state.pending_profile_switch.is_none(),
        "no switch when selecting the already-active profile"
    );
}

// --- AC7: no token leaks in serialized config or rendered UI ---

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

#[test]
fn rendered_ui_never_shows_a_token() {
    use ghdash::app::view;
    use ratatui::{Terminal, backend::TestBackend};

    let secret = "ghp_rendered_secret_must_be_absent_9999";

    let mut state = picker_state();
    state.loading = false;
    // Open the picker so both the status-bar chip and the modal render.
    update(&mut state, Action::ToggleProfilePicker);

    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| view::render(f, &state)).unwrap();

    let rendered = format!("{}", terminal.backend());
    assert!(
        !rendered.contains(secret),
        "no token value may appear in rendered output"
    );
    // Sanity: the active-profile chip and picker are actually on screen.
    assert!(rendered.contains("work"), "active profile chip is rendered");
    assert!(
        rendered.contains("Switch Profile"),
        "profile picker modal is rendered"
    );
}
