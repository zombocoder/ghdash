use anyhow::Result;
use tracing::debug;

/// Open a URL in the user's default browser.
pub fn open_url(url: &str) -> Result<()> {
    debug!(url = url, "Opening URL in browser");
    open::that(url)?;
    Ok(())
}
