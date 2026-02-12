use std::io;
use std::sync::Arc;

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use futures::StreamExt;
use ratatui::{Terminal, backend::CrosstermBackend};
use tokio::sync::{Semaphore, mpsc};
use tracing::{debug, error};

use crate::app::actions::{Action, DataPayload, SideEffect};
use crate::app::state::AppState;
use crate::app::update::update;
use crate::app::view;
use crate::cache::CacheStore;
use crate::github::GithubClient;
use crate::util::config::AppConfig;

pub async fn run(
    config: AppConfig,
    client: GithubClient,
    viewer_login: String,
    cache_store: Option<CacheStore>,
) -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Install panic hook to restore terminal
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        original_hook(panic_info);
    }));

    let result = run_loop(&mut terminal, config, client, viewer_login, cache_store).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    result
}

async fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    config: AppConfig,
    client: GithubClient,
    viewer_login: String,
    cache_store: Option<CacheStore>,
) -> Result<()> {
    let all_owners: Vec<String> = config
        .github
        .orgs
        .iter()
        .chain(config.github.users.iter())
        .cloned()
        .collect();
    let mut state = AppState::new(viewer_login.clone(), all_owners);

    let (action_tx, mut action_rx) = mpsc::unbounded_channel::<Action>();
    let semaphore = Arc::new(Semaphore::new(4));

    // Initial data fetch
    let effects = vec![SideEffect::RefreshAll];
    for effect in effects {
        spawn_side_effect(
            effect,
            &config,
            &client,
            &viewer_login,
            &cache_store,
            &action_tx,
            &semaphore,
        );
    }

    let mut event_stream = crossterm::event::EventStream::new();
    let refresh_interval = config.dashboard.refresh_interval_secs;

    let mut refresh_timer =
        tokio::time::interval(tokio::time::Duration::from_secs(refresh_interval));
    // First tick fires immediately (already handled by initial fetch above)
    refresh_timer.tick().await;

    loop {
        // Render
        terminal.draw(|f| view::render(f, &state))?;

        if state.should_quit {
            break;
        }

        // Wait for events
        tokio::select! {
            // Terminal events
            maybe_event = event_stream.next() => {
                if let Some(Ok(event)) = maybe_event
                    && let Some(action) = map_event_to_action(&event, &state) {
                        let effects = update(&mut state, action);
                        for effect in effects {
                            spawn_side_effect(
                                effect,
                                &config,
                                &client,
                                &viewer_login,
                                &cache_store,
                                &action_tx,
                                &semaphore,
                            );
                        }
                    }
            }
            // Actions from background tasks
            Some(action) = action_rx.recv() => {
                let effects = update(&mut state, action);
                for effect in effects {
                    spawn_side_effect(
                        effect,
                        &config,
                        &client,
                        &viewer_login,
                        &cache_store,
                        &action_tx,
                        &semaphore,
                    );
                }
            }
            // Auto-refresh timer
            _ = refresh_timer.tick() => {
                if !state.loading {
                    let effects = update(&mut state, Action::Refresh);
                    for effect in effects {
                        spawn_side_effect(
                            effect,
                            &config,
                            &client,
                            &viewer_login,
                            &cache_store,
                            &action_tx,
                            &semaphore,
                        );
                    }
                }
            }
        }
    }

    Ok(())
}

fn map_event_to_action(event: &Event, state: &AppState) -> Option<Action> {
    let Event::Key(KeyEvent {
        code,
        modifiers,
        kind: event::KeyEventKind::Press,
        ..
    }) = event
    else {
        return None;
    };

    // Handle error modal first
    if state.error_message.is_some() {
        return match code {
            KeyCode::Esc => Some(Action::DismissError),
            _ => None,
        };
    }

    // Handle search mode
    if state.search_active {
        return match code {
            KeyCode::Esc => Some(Action::ToggleSearch),
            KeyCode::Backspace => Some(Action::SearchBackspace),
            KeyCode::Char(c) => Some(Action::SearchInput(*c)),
            KeyCode::Enter => Some(Action::ToggleSearch),
            _ => None,
        };
    }

    // Normal mode
    match code {
        KeyCode::Char('q') => Some(Action::Quit),
        KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => Some(Action::Quit),
        KeyCode::Char('j') | KeyCode::Down => Some(Action::MoveDown),
        KeyCode::Char('k') | KeyCode::Up => Some(Action::MoveUp),
        KeyCode::Enter | KeyCode::Char('l') | KeyCode::Right => Some(Action::Select),
        KeyCode::Esc | KeyCode::Char('h') | KeyCode::Left => Some(Action::Back),
        KeyCode::Tab => Some(Action::SwitchPane),
        KeyCode::BackTab => Some(Action::SwitchPane),
        KeyCode::Char('r') => Some(Action::Refresh),
        KeyCode::Char('o') => Some(Action::OpenInBrowser),
        KeyCode::Char('/') => Some(Action::ToggleSearch),
        _ => None,
    }
}

fn spawn_side_effect(
    effect: SideEffect,
    config: &AppConfig,
    client: &GithubClient,
    viewer_login: &str,
    cache_store: &Option<CacheStore>,
    action_tx: &mpsc::UnboundedSender<Action>,
    semaphore: &Arc<Semaphore>,
) {
    match effect {
        SideEffect::RefreshAll => {
            // Spawn org fetches
            for org in &config.github.orgs {
                spawn_side_effect(
                    SideEffect::FetchOrgRepos(org.clone()),
                    config,
                    client,
                    viewer_login,
                    cache_store,
                    action_tx,
                    semaphore,
                );
            }
            // Spawn user fetches
            for user in &config.github.users {
                spawn_side_effect(
                    SideEffect::FetchUserRepos(user.clone()),
                    config,
                    client,
                    viewer_login,
                    cache_store,
                    action_tx,
                    semaphore,
                );
            }
            // Fetch inbox
            spawn_side_effect(
                SideEffect::FetchInbox,
                config,
                client,
                viewer_login,
                cache_store,
                action_tx,
                semaphore,
            );
            // Fetch all open PRs
            spawn_side_effect(
                SideEffect::FetchAllOpenPrs,
                config,
                client,
                viewer_login,
                cache_store,
                action_tx,
                semaphore,
            );
        }
        SideEffect::FetchOrgRepos(org) => {
            let client = client.clone();
            let tx = action_tx.clone();
            let sem = semaphore.clone();
            let cache = cache_store.clone();
            let include_repos = config.github.include_repos.clone();
            let exclude_repos = config.github.exclude_repos.clone();
            let org_clone = org.clone();

            // Mark org as loading via action
            let _ = tx.send(Action::DataLoaded(DataPayload::OrgRepos {
                org: org.clone(),
                repos: Vec::new(),
                rate_limit: crate::github::RateLimit::default(),
            }));

            tokio::spawn(async move {
                let _permit = sem.acquire().await;
                debug!(org = %org_clone, "Fetching org repos");

                // Check cache
                let cache_key = format!("org_repos_{}", org_clone);
                if let Some(ref cache) = cache
                    && let Some(repos) = cache.get::<Vec<crate::github::Repo>>(&cache_key)
                {
                    let filtered = filter_repos(repos, &include_repos, &exclude_repos);
                    let _ = tx.send(Action::DataLoaded(DataPayload::OrgRepos {
                        org: org_clone,
                        repos: filtered,
                        rate_limit: crate::github::RateLimit::default(),
                    }));
                    return;
                }

                match client.fetch_org_repos(&org_clone).await {
                    Ok((repos, rate_limit)) => {
                        // Cache the raw repos
                        if let Some(ref cache) = cache
                            && let Err(e) = cache.set(&cache_key, &repos)
                        {
                            error!(error = %e, "Failed to cache org repos");
                        }

                        let filtered = filter_repos(repos, &include_repos, &exclude_repos);
                        let _ = tx.send(Action::DataLoaded(DataPayload::OrgRepos {
                            org: org_clone,
                            repos: filtered,
                            rate_limit,
                        }));
                    }
                    Err(e) => {
                        error!(org = %org_clone, error = %e, "Failed to fetch org repos");
                        let _ = tx.send(Action::LoadError(format!(
                            "Failed to fetch repos for {}: {}",
                            org_clone, e
                        )));
                    }
                }
            });
        }
        SideEffect::FetchUserRepos(user) => {
            let client = client.clone();
            let tx = action_tx.clone();
            let sem = semaphore.clone();
            let cache = cache_store.clone();
            let include_repos = config.github.include_repos.clone();
            let exclude_repos = config.github.exclude_repos.clone();
            let user_clone = user.clone();

            // Mark user as loading via action (reuse OrgRepos payload)
            let _ = tx.send(Action::DataLoaded(DataPayload::OrgRepos {
                org: user.clone(),
                repos: Vec::new(),
                rate_limit: crate::github::RateLimit::default(),
            }));

            tokio::spawn(async move {
                let _permit = sem.acquire().await;
                debug!(user = %user_clone, "Fetching user repos");

                let cache_key = format!("user_repos_{}", user_clone);
                if let Some(ref cache) = cache
                    && let Some(repos) = cache.get::<Vec<crate::github::Repo>>(&cache_key)
                {
                    let filtered = filter_repos(repos, &include_repos, &exclude_repos);
                    let _ = tx.send(Action::DataLoaded(DataPayload::OrgRepos {
                        org: user_clone,
                        repos: filtered,
                        rate_limit: crate::github::RateLimit::default(),
                    }));
                    return;
                }

                match client.fetch_user_repos(&user_clone).await {
                    Ok((repos, rate_limit)) => {
                        if let Some(ref cache) = cache
                            && let Err(e) = cache.set(&cache_key, &repos)
                        {
                            error!(error = %e, "Failed to cache user repos");
                        }

                        let filtered = filter_repos(repos, &include_repos, &exclude_repos);
                        let _ = tx.send(Action::DataLoaded(DataPayload::OrgRepos {
                            org: user_clone,
                            repos: filtered,
                            rate_limit,
                        }));
                    }
                    Err(e) => {
                        error!(user = %user_clone, error = %e, "Failed to fetch user repos");
                        let _ = tx.send(Action::LoadError(format!(
                            "Failed to fetch repos for {}: {}",
                            user_clone, e
                        )));
                    }
                }
            });
        }
        SideEffect::FetchInbox => {
            let client = client.clone();
            let tx = action_tx.clone();
            let sem = semaphore.clone();
            let cache = cache_store.clone();
            let login = viewer_login.to_string();

            tokio::spawn(async move {
                let _permit = sem.acquire().await;
                debug!("Fetching inbox");

                let cache_key = format!("inbox_{}", login);
                if let Some(ref cache) = cache
                    && let Some(prs) = cache.get::<Vec<crate::github::PullRequest>>(&cache_key)
                {
                    let _ = tx.send(Action::DataLoaded(DataPayload::InboxPrs {
                        prs,
                        rate_limit: crate::github::RateLimit::default(),
                    }));
                    return;
                }

                match client.fetch_inbox(&login).await {
                    Ok((prs, rate_limit)) => {
                        if let Some(ref cache) = cache
                            && let Err(e) = cache.set(&cache_key, &prs)
                        {
                            error!(error = %e, "Failed to cache inbox");
                        }
                        let _ = tx.send(Action::DataLoaded(DataPayload::InboxPrs {
                            prs,
                            rate_limit,
                        }));
                    }
                    Err(e) => {
                        error!(error = %e, "Failed to fetch inbox");
                        let _ = tx.send(Action::LoadError(format!("Failed to fetch inbox: {}", e)));
                    }
                }
            });
        }
        SideEffect::FetchAllOpenPrs => {
            let client = client.clone();
            let tx = action_tx.clone();
            let sem = semaphore.clone();
            let cache = cache_store.clone();
            let orgs = config.github.orgs.clone();
            let users = config.github.users.clone();

            tokio::spawn(async move {
                let _permit = sem.acquire().await;
                debug!("Fetching all open PRs");

                let cache_key = "all_open_prs".to_string();
                if let Some(ref cache) = cache
                    && let Some(prs) = cache.get::<Vec<crate::github::PullRequest>>(&cache_key)
                {
                    let _ = tx.send(Action::DataLoaded(DataPayload::AllOpenPrs {
                        prs,
                        rate_limit: crate::github::RateLimit::default(),
                    }));
                    return;
                }

                match client.fetch_all_open_prs(&orgs, &users).await {
                    Ok((prs, rate_limit)) => {
                        if let Some(ref cache) = cache
                            && let Err(e) = cache.set(&cache_key, &prs)
                        {
                            error!(error = %e, "Failed to cache all open PRs");
                        }
                        let _ = tx.send(Action::DataLoaded(DataPayload::AllOpenPrs {
                            prs,
                            rate_limit,
                        }));
                    }
                    Err(e) => {
                        error!(error = %e, "Failed to fetch all open PRs");
                        let _ = tx.send(Action::LoadError(format!(
                            "Failed to fetch all open PRs: {}",
                            e
                        )));
                    }
                }
            });
        }
        SideEffect::OpenUrl(url) => {
            tokio::task::spawn_blocking(move || {
                if let Err(e) = crate::util::browser::open_url(&url) {
                    error!(error = %e, "Failed to open URL");
                }
            });
        }
    }
}

fn filter_repos(
    repos: Vec<crate::github::Repo>,
    include_patterns: &[String],
    exclude_patterns: &[String],
) -> Vec<crate::github::Repo> {
    repos
        .into_iter()
        .filter(|repo| {
            let full_name = repo.full_name();
            let name = &repo.name;

            // If include patterns specified, repo must match at least one
            if !include_patterns.is_empty() {
                let matches = include_patterns
                    .iter()
                    .any(|pattern| glob_match(pattern, &full_name) || glob_match(pattern, name));
                if !matches {
                    return false;
                }
            }

            // If exclude patterns specified, repo must not match any
            if !exclude_patterns.is_empty() {
                let excluded = exclude_patterns
                    .iter()
                    .any(|pattern| glob_match(pattern, &full_name) || glob_match(pattern, name));
                if excluded {
                    return false;
                }
            }

            true
        })
        .collect()
}

fn glob_match(pattern: &str, text: &str) -> bool {
    // Simple glob matching: * matches any sequence
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() == 1 {
        return pattern == text;
    }

    let mut pos = 0;
    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        match text[pos..].find(part) {
            Some(idx) => {
                if i == 0 && idx != 0 {
                    return false;
                }
                pos += idx + part.len();
            }
            None => return false,
        }
    }

    // If the pattern doesn't end with *, the text must end at pos
    if !pattern.ends_with('*') {
        return pos == text.len();
    }

    true
}
