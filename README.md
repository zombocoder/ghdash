# ghdash

A terminal UI dashboard for monitoring GitHub repositories, pull requests, and your review inbox. Built with Rust, ratatui, and the GitHub GraphQL API.

## Features

- Monitor repos across multiple GitHub organizations and personal accounts
- View all open pull requests in one place
- Inbox view for PRs where you're requested for review or assigned
- Expand/collapse organizations in the navigation tree
- Client-side search filtering across PR titles, authors, and repos
- Open any PR or repo in your browser with a single keypress
- Disk caching with configurable TTL to minimize API calls
- Auto-refresh on a configurable interval
- Vim-style keybindings

## Installation

### Homebrew (macOS & Linux)

```sh
brew tap zombocoder/tap https://github.com/zombocoder/ghdash.git
brew install ghdash
```

### pkgsrc (NetBSD & others)

```sh
cargo install --path .
```

### From releases

Download a prebuilt binary from the [Releases](https://github.com/zombocoder/ghdash/releases) page for your platform:

- macOS (Intel & Apple Silicon)
- Linux (x86_64 & aarch64)
- Windows (x86_64)

### From source

```sh
cargo install --path .
```

### Build from source

```sh
git clone https://github.com/zombocoder/ghdash.git
cd ghdash
cargo build --release
# Binary is at ./target/release/ghdash
```

## Authentication

ghdash resolves your GitHub token in this order:

1. `gh auth token` (GitHub CLI — recommended)
2. `GITHUB_TOKEN` environment variable
3. `GH_TOKEN` environment variable

The easiest way is to install the [GitHub CLI](https://cli.github.com/) and run `gh auth login`.

## Configuration

Create a config file at `~/.config/ghdash/config.toml`:

```toml
[github]
# Organizations to monitor
orgs = ["my-org"]
# Personal accounts to monitor
users = ["my-username"]
# Optional: only include repos matching these globs
include_repos = ["important-*"]
# Optional: exclude repos matching these globs
exclude_repos = ["*-archived", "legacy-*"]
# Optional: GitHub Enterprise
# api_url = "https://github.example.com/api/graphql"

[dashboard]
# Auto-refresh interval in seconds (default: 300)
refresh_interval_secs = 300
# Show draft PRs (default: true)
show_draft_prs = true

[cache]
# Cache TTL in seconds (default: 600)
ttl_secs = 600
# Optional: custom cache directory
# dir = "/tmp/ghdash-cache"

[ui]
# Navigation pane width percentage (default: 30)
nav_width_percent = 30
```

On macOS, `~/Library/Application Support/ghdash/config.toml` is also supported.

## Usage

```sh
ghdash                     # Start the dashboard
ghdash --config path.toml  # Use a specific config file
ghdash --refresh           # Force refresh all data on startup
ghdash --no-cache          # Disable disk cache
ghdash --debug             # Enable debug logging to file
ghdash --help              # Show all options
```

## Keybindings

| Key                     | Action                               |
| ----------------------- | ------------------------------------ |
| `j` / `Down`            | Move down                            |
| `k` / `Up`              | Move up                              |
| `Enter` / `l` / `Right` | Select / expand / open PR            |
| `Esc` / `h` / `Left`    | Back / collapse                      |
| `Tab` / `Shift+Tab`     | Switch between nav and content panes |
| `r`                     | Refresh all data                     |
| `o`                     | Open selected item in browser        |
| `/`                     | Toggle search filter                 |
| `q` / `Ctrl+C`          | Quit                                 |

### In search mode

| Key             | Action                               |
| --------------- | ------------------------------------ |
| Type            | Filter PRs by title, author, or repo |
| `Backspace`     | Delete character                     |
| `Esc` / `Enter` | Close search                         |

## Architecture

```
src/
  main.rs           CLI entry point, auth, logging setup
  lib.rs            Public module re-exports
  app/
    state.rs        Core app state (orgs, PRs, nav tree, UI flags)
    actions.rs      Action enum + SideEffect enum
    update.rs       Pure state reducer: update(state, action) -> side effects
    event_loop.rs   Async event loop (crossterm + tokio + mpsc channel)
    view.rs         Layout composition
  github/
    auth.rs         Token resolution (gh CLI / env vars)
    models.rs       Repo, PullRequest, RateLimit types
    queries.rs      GraphQL query strings
    graphql.rs      GithubClient with pagination
  cache/
    store.rs        JSON file cache with TTL
  ui/
    theme.rs        Style constants
    widgets.rs      Rendering functions (nav, PR table, status bar, overlays)
  util/
    config.rs       TOML config with XDG paths
    time.rs         Relative time formatting
    browser.rs      Open URL in browser
```

The app uses an **Action Channel pattern**: crossterm key events and background API results both feed into a single `mpsc` channel of `Action`s. The main loop calls `update()` (a pure state reducer) then `render()`. Side effects (API calls, browser open) are spawned as tokio tasks with bounded concurrency via a semaphore.

## License

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE) for details.
