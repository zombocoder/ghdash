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

    // Resolve auth token before starting TUI
    let token = match github::auth::resolve_token() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Authentication error: {e}");
            std::process::exit(1);
        }
    };

    let client = github::GithubClient::new(&token, &config.github.api_url)?;

    // Verify auth by fetching viewer
    let viewer = match client.fetch_viewer().await {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Failed to authenticate with GitHub: {e}");
            eprintln!("Please check your token and try again.");
            std::process::exit(1);
        }
    };

    info!(login = %viewer, "Authenticated as {}", viewer);

    if config.github.orgs.is_empty() && config.github.users.is_empty() {
        eprintln!(
            "No organizations or users configured. Please add orgs or users to your config file.\n\
             Example config (~/.config/ghdash/config.toml):\n\n\
             [github]\n\
             orgs = [\"my-org\"]\n\
             users = [\"my-username\"]"
        );
        std::process::exit(1);
    }

    // Build cache store
    let cache_store = if cli.no_cache {
        None
    } else {
        let store = cache::CacheStore::new(config.cache_dir(), config.cache.ttl_secs);
        if cli.refresh {
            store.invalidate_all()?;
        }
        Some(store)
    };

    // Run the TUI event loop
    app::event_loop::run(config, client, viewer, cache_store).await
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
