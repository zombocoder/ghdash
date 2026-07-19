mod app;
mod cache;
mod github;
mod ui;
mod util;

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;
use tracing::info;

#[derive(Parser, Debug)]
#[command(name = "ghdash", version, about = "TUI GitHub Dashboard")]
struct Cli {
    /// Path to config file
    #[arg(short, long)]
    config: Option<PathBuf>,

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

    // Resolve the profile list (single "default" when no [[profiles]] configured)
    // and pick the active one.
    let profiles = config.profiles();
    let active_name = config.active_profile_name();
    let base_cache_dir = config.cache_dir();

    let active_profile = profiles
        .iter()
        .find(|p| p.name == active_name)
        .cloned()
        .expect("active profile always present in profile list");

    // Build the runtime for the active profile (resolves token, authenticates,
    // opens its cache namespace). Exits on auth failure, as before.
    let runtime = match app::event_loop::build_runtime(
        &active_profile,
        &base_cache_dir,
        cli.no_cache,
        cli.refresh,
    )
    .await
    {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("Failed to start profile '{active_name}': {e}");
            eprintln!("Check the profile's token (token_env / GITHUB_TOKEN / `gh auth login`).");
            std::process::exit(1);
        }
    };

    info!(login = %runtime.viewer_login, profile = %active_name, "Authenticated");

    if runtime.config.github.orgs.is_empty() && runtime.config.github.users.is_empty() {
        eprintln!(
            "No organizations or users configured for profile '{active_name}'. Add orgs or users \
             to your config file.\n\
             Example config (~/.config/ghdash/config.toml):\n\n\
             [github]\n\
             orgs = [\"my-org\"]\n\
             users = [\"my-username\"]"
        );
        std::process::exit(1);
    }

    // Run the TUI event loop
    app::event_loop::run(profiles, active_name, base_cache_dir, cli.no_cache, runtime).await
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
