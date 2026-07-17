use anyhow::{Result, bail};
use std::process::Command;
use tracing::debug;

/// Derive the `gh` CLI `--hostname` value from a GraphQL/REST `api_url`.
///
/// `https://api.github.com/graphql` maps to `github.com` (the hostname `gh`
/// expects for public GitHub), while an Enterprise host such as
/// `https://ghe.acme.corp/api/v3` maps to `ghe.acme.corp`.
pub fn gh_hostname(api_url: &str) -> Option<String> {
    let rest = api_url
        .strip_prefix("https://")
        .or_else(|| api_url.strip_prefix("http://"))?;
    let host = rest.split('/').next()?.split(':').next()?;
    if host.is_empty() {
        return None;
    }
    let host = if host == "api.github.com" {
        "github.com"
    } else {
        host
    };
    Some(host.to_string())
}

/// Resolve a profile's token WITHOUT ever persisting it. Resolution order:
/// 1. the env var named by `token_env` (never the token itself in config),
/// 2. `GITHUB_TOKEN`,
/// 3. `gh auth token --hostname <host>` (reuses the user's `gh` login, incl.
///    per-host Enterprise credentials).
///
/// The returned token is never logged; only the resolution *source* is traced.
pub fn resolve_profile_token(token_env: Option<&str>, api_url: &str) -> Result<String> {
    // 1. Profile-specific env var (by name).
    if let Some(var) = token_env
        && let Ok(token) = std::env::var(var)
        && !token.is_empty()
    {
        debug!(var = var, "Token resolved via profile token_env");
        return Ok(token);
    }

    // 2. GITHUB_TOKEN.
    if let Ok(token) = std::env::var("GITHUB_TOKEN")
        && !token.is_empty()
    {
        debug!("Token resolved via GITHUB_TOKEN env var");
        return Ok(token);
    }

    // 3. gh auth token --hostname <host>.
    if let Some(host) = gh_hostname(api_url) {
        debug!(host = %host, "Attempting to resolve token via `gh auth token`");
        if let Ok(output) = Command::new("gh")
            .args(["auth", "token", "--hostname", &host])
            .output()
            && output.status.success()
        {
            let token = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !token.is_empty() {
                debug!(host = %host, "Token resolved via gh CLI");
                return Ok(token);
            }
        }
    }

    // 4. GH_TOKEN (kept as a final fallback for existing setups).
    if let Ok(token) = std::env::var("GH_TOKEN")
        && !token.is_empty()
    {
        debug!("Token resolved via GH_TOKEN env var");
        return Ok(token);
    }

    bail!(
        "Could not resolve a GitHub token for this profile. Set the env var named \
         by `token_env`, set GITHUB_TOKEN, or run `gh auth login` for the host."
    )
}
