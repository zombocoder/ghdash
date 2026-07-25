mod app;
mod cache;
mod github;
mod ui;
mod util;

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;
use tracing::info;

use util::config::PROFILE_ENV;

#[derive(Parser, Debug)]
#[command(name = "ghdash", version, about = "TUI GitHub Dashboard")]
struct Cli {
    /// Path to config file
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// Active profile to start with (overrides `active_profile` in config and the
    /// GHDASH_PROFILE env var). Falls back to the first profile if unknown.
    #[arg(short, long)]
    profile: Option<String>,

    /// Disable disk cache
    #[arg(long)]
    no_cache: bool,

    /// Force refresh all data on startup
    #[arg(short, long)]
    refresh: bool,

    /// Enable debug logging to file
    #[arg(short, long)]
    debug: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let config = util::config::AppConfig::load(cli.config.as_deref())?;

    // Setup logging
    let _guard = setup_logging(&config, cli.debug)?;

    info!("ghdash starting");

    // Select the active profile: `--profile` flag, else GHDASH_PROFILE env, else
    // the `active_profile` config field, else the first profile. A requested name
    // that matches no profile is ignored (falls through), so a stale value
    // degrades gracefully to the first profile rather than failing.
    let requested = cli
        .profile
        .clone()
        .or_else(|| std::env::var(PROFILE_ENV).ok().filter(|s| !s.is_empty()));
    let active_profile = config.active_profile(requested.as_deref());
    if let Some(name) = &requested
        && name != &active_profile.name
    {
        eprintln!(
            "Profile '{name}' not found; using '{}' instead.",
            active_profile.name
        );
    }

    // Effective single-context config for the active profile.
    let effective = active_profile.to_app_config();

    // Resolve this profile's token (by env-var name / GITHUB_TOKEN / gh CLI) and
    // its api_url. The token value is never stored or logged.
    let token = match github::auth::resolve_profile_token(
        effective.github.token_env.as_deref(),
        &effective.github.api_url,
    ) {
        Ok(t) => t,
        Err(e) => {
            eprintln!(
                "Authentication error for profile '{}': {e}",
                active_profile.name
            );
            std::process::exit(1);
        }
    };

    let client = github::GithubClient::new(&token, &effective.github.api_url)?;

    // Verify auth by fetching viewer
    let viewer = match client.fetch_viewer().await {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Failed to authenticate with GitHub: {e}");
            eprintln!("Please check your token and try again.");
            std::process::exit(1);
        }
    };

    info!(login = %viewer, profile = %active_profile.name, "Authenticated as {}", viewer);

    if effective.github.orgs.is_empty() && effective.github.users.is_empty() {
        eprintln!(
            "No organizations or users configured for profile '{}'. Please add orgs or users to \
             your config file.\n\
             Example config (~/.config/ghdash/config.toml):\n\n\
             [github]\n\
             orgs = [\"my-org\"]\n\
             users = [\"my-username\"]",
            active_profile.name
        );
        std::process::exit(1);
    }

    // Build cache store from the profile's namespaced cache directory, so profiles
    // never read each other's cached data. With no [[profiles]] configured, this
    // is the plain top-level cache dir (unchanged for existing installs).
    let cache_store = if cli.no_cache {
        None
    } else {
        let store = cache::CacheStore::new(
            config.profile_cache_dir(&active_profile),
            effective.cache.ttl_secs,
        );
        if cli.refresh {
            store.invalidate_all()?;
        }
        Some(store)
    };

    // Run the TUI event loop
    app::event_loop::run(effective, client, viewer, cache_store).await
}

fn setup_logging(
    config: &util::config::AppConfig,
    debug: bool,
) -> Result<Option<tracing_appender::non_blocking::WorkerGuard>> {
    if !debug {
        return Ok(None);
    }

    let log_dir = config.log_dir();
    std::fs::create_dir_all(&log_dir)?;

    let file_appender = tracing_appender::rolling::daily(&log_dir, "ghdash.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::fmt()
        .with_writer(non_blocking)
        .with_env_filter("ghdash=debug")
        .with_ansi(false)
        .init();

    Ok(Some(guard))
}
