use anyhow::{Result, bail};
use std::process::Command;
use tracing::debug;

/// Resolve GitHub token using multiple strategies:
/// 1. `gh auth token` subprocess
/// 2. `GITHUB_TOKEN` environment variable
/// 3. `GH_TOKEN` environment variable
pub fn resolve_token() -> Result<String> {
    // Try `gh auth token` first
    debug!("Attempting to resolve token via `gh auth token`");
    if let Ok(output) = Command::new("gh").args(["auth", "token"]).output()
        && output.status.success()
    {
        let token = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !token.is_empty() {
            debug!("Token resolved via gh CLI");
            return Ok(token);
        }
    }

    // Try GITHUB_TOKEN env
    if let Ok(token) = std::env::var("GITHUB_TOKEN")
        && !token.is_empty()
    {
        debug!("Token resolved via GITHUB_TOKEN env var");
        return Ok(token);
    }

    // Try GH_TOKEN env
    if let Ok(token) = std::env::var("GH_TOKEN")
        && !token.is_empty()
    {
        debug!("Token resolved via GH_TOKEN env var");
        return Ok(token);
    }

    bail!(
        "Could not resolve GitHub token. Please either:\n\
         - Run `gh auth login` to authenticate with the GitHub CLI\n\
         - Set the GITHUB_TOKEN environment variable\n\
         - Set the GH_TOKEN environment variable"
    )
}
