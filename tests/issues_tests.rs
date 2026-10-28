use std::io::Write;
use tempfile::NamedTempFile;

use ghdash::app::actions::{Action, DataPayload, SideEffect};
use ghdash::app::state::{AppState, ContentView, FocusedPane, ListMode, NavNode, Overlay};
use ghdash::app::update::update;
use ghdash::github::graphql::{
    build_all_open_issues_query, build_issue_inbox_query, parse_search_issue,
};
use ghdash::github::models::{Issue, PullRequest, RateLimit, Repo, RepoVisibility};
use ghdash::util::config::{AppConfig, IssuesConfig};

fn make_state() -> AppState {
    AppState::new("testuser".into(), vec!["org-a".into()])
}

fn make_repo(name: &str, open_prs: u32, open_issues: u32) -> Repo {
    Repo {
        name: name.into(),
        owner: "org-a".into(),
        url: format!("https://github.com/org-a/{}", name),
        description: None,
        open_pr_count: open_prs,
        open_issue_count: open_issues,
        is_archived: false,
        is_private: false,
    }
}

fn make_issue(repo_name: &str, number: u32, title: &str, labels: &[&str]) -> Issue {
    Issue {
        number,
        title: title.into(),
        author: "author".into(),
        repo_owner: "org-a".into(),
        repo_name: repo_name.into(),
        url: format!("https://github.com/org-a/{}/issues/{}", repo_name, number),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        labels: labels.iter().map(|l| l.to_string()).collect(),
        comment_count: 0,
    }
}

fn make_pr(repo_name: &str, number: u32) -> PullRequest {
    PullRequest {
        number,
        title: "pr".into(),
        author: "author".into(),
        repo_owner: "org-a".into(),
        repo_name: repo_name.into(),
        url: format!("https://github.com/org-a/{}/pull/{}", repo_name, number),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        is_draft: false,
        additions: 0,
        deletions: 0,
        review_decision: None,
        mergeable: None,
        merge_state_status: None,
        checks_status: None,
        labels: vec![],
    }
}

fn load_issues(state: &mut AppState, issues: Vec<Issue>) {
    update(
        state,
        Action::DataLoaded(DataPayload::AllOpenIssues {
            issues,
            rate_limit: RateLimit::default(),
        }),
    );
}

// --- Query building ---

#[test]
fn all_open_issues_query_without_filter() {
    let q =
        build_all_open_issues_query(&["acme".into()], &["alice".into()], RepoVisibility::All, "");
    assert_eq!(q, "is:open is:issue archived:false org:acme user:alice");
}

#[test]
fn all_open_issues_query_appends_visibility_and_filter() {
    let q = build_all_open_issues_query(
        &["acme".into()],
        &[],
        RepoVisibility::Public,
        "  label:bug -label:wontfix ",
    );
    assert_eq!(
        q,
        "is:open is:issue archived:false org:acme is:public label:bug -label:wontfix"
    );
}

#[test]
fn issue_inbox_query_is_assigned_to_viewer() {
    let q = build_issue_inbox_query("me", RepoVisibility::All, "");
    assert_eq!(q, "is:open is:issue assignee:me archived:false");
}

#[test]
fn issue_inbox_query_applies_visibility_and_filter() {
    let q = build_issue_inbox_query("me", RepoVisibility::Private, "label:bug");
    assert_eq!(
        q,
        "is:open is:issue assignee:me archived:false is:private label:bug"
    );
}

// --- Parsing ---

#[test]
fn parses_issue_search_node() {
    let node = serde_json::json!({
        "number": 42,
        "title": "Crash on start",
        "author": { "login": "bob" },
        "repository": { "name": "app", "owner": { "login": "acme" } },
        "url": "https://github.com/acme/app/issues/42",
        "createdAt": "2026-01-01T00:00:00Z",
        "updatedAt": "2026-01-02T00:00:00Z",
        "comments": { "totalCount": 3 },
        "labels": { "nodes": [{ "name": "bug" }, { "name": "p1" }] }
    });
    let issue = parse_search_issue(&node);
    assert_eq!(issue.number, 42);
    assert_eq!(issue.title, "Crash on start");
    assert_eq!(issue.author, "bob");
    assert_eq!(issue.repo_full_name(), "acme/app");
    assert_eq!(issue.comment_count, 3);
    assert_eq!(issue.labels, vec!["bug", "p1"]);
}

#[test]
fn parses_issue_with_deleted_author_as_ghost() {
    let node = serde_json::json!({
        "number": 1,
        "title": "t",
        "author": null,
        "repository": { "name": "app", "owner": { "login": "acme" } },
        "url": "u",
        "createdAt": "2026-01-01T00:00:00Z",
        "updatedAt": "2026-01-01T00:00:00Z",
        "comments": { "totalCount": 0 },
        "labels": { "nodes": [] }
    });
    assert_eq!(parse_search_issue(&node).author, "ghost");
}

// --- Config ---

#[test]
fn issues_filter_defaults_to_empty() {
    let mut f = NamedTempFile::new().unwrap();
    f.write_all(b"[github]\norgs = [\"acme\"]\n").unwrap();
    let config = AppConfig::load(Some(f.path())).unwrap();
    assert_eq!(config.issues.filter, "");
}

#[test]
fn loads_issues_filter() {
    let mut f = NamedTempFile::new().unwrap();
    f.write_all(b"[issues]\nfilter = \"label:bug -label:wontfix\"\n")
        .unwrap();
    let config = AppConfig::load(Some(f.path())).unwrap();
    assert_eq!(config.issues.filter, "label:bug -label:wontfix");
}

#[test]
fn issues_cache_suffix_tracks_filter() {
    let none = IssuesConfig::default();
    let blank = IssuesConfig {
        filter: "   ".into(),
    };
    let bug = IssuesConfig {
        filter: "label:bug".into(),
    };
    let feature = IssuesConfig {
        filter: "label:feature".into(),
    };
    assert_eq!(none.cache_suffix(), blank.cache_suffix());
    assert_ne!(none.cache_suffix(), bug.cache_suffix());
    assert_ne!(bug.cache_suffix(), feature.cache_suffix());
    assert_eq!(bug.cache_suffix(), bug.clone().cache_suffix());
    assert!(
        bug.cache_suffix()
            .chars()
            .all(|c| c.is_ascii_alphanumeric())
    );
}

// --- Mode toggling ---

#[test]
fn starts_in_pull_request_mode() {
    let state = make_state();
    assert_eq!(state.mode, ListMode::PullRequests);
}

#[test]
fn first_toggle_to_issues_fetches_issue_lists() {
    let mut state = make_state();
    let effects = update(&mut state, Action::ToggleListMode);
    assert_eq!(state.mode, ListMode::Issues);
    assert!(state.loading);
    assert_eq!(effects.len(), 2);
    assert!(matches!(effects[0], SideEffect::FetchIssueInbox));
    assert!(matches!(effects[1], SideEffect::FetchAllOpenIssues));
}

#[test]
fn toggling_back_and_forth_does_not_refetch() {
    let mut state = make_state();
    update(&mut state, Action::ToggleListMode);
    let back = update(&mut state, Action::ToggleListMode);
    assert_eq!(state.mode, ListMode::PullRequests);
    assert!(back.is_empty());
    let again = update(&mut state, Action::ToggleListMode);
    assert!(again.is_empty());
}

#[test]
fn refresh_only_targets_current_mode_and_marks_other_stale() {
    let mut state = make_state();
    update(&mut state, Action::ToggleListMode);
    let effects = update(&mut state, Action::Refresh);
    assert!(matches!(
        effects[0],
        SideEffect::RefreshAll(ListMode::Issues)
    ));

    // PR lists were dropped from the cache by the refresh, so going back
    // to PR mode refetches them.
    let back = update(&mut state, Action::ToggleListMode);
    assert_eq!(back.len(), 2);
    assert!(matches!(back[0], SideEffect::FetchInbox));
    assert!(matches!(back[1], SideEffect::FetchAllOpenPrs));
}

#[test]
fn toggle_resets_cursor_and_closes_overlay() {
    let mut state = make_state();
    state.content_cursor = 3;
    state.overlay = Overlay::GitLog;
    update(&mut state, Action::ToggleListMode);
    assert_eq!(state.content_cursor, 0);
    assert_eq!(state.overlay, Overlay::None);
}

#[test]
fn nav_counts_and_order_follow_mode() {
    let mut state = make_state();
    update(
        &mut state,
        Action::DataLoaded(DataPayload::OrgRepos {
            org: "org-a".into(),
            repos: vec![make_repo("few-prs", 1, 9), make_repo("many-prs", 5, 2)],
            rate_limit: RateLimit::default(),
        }),
    );

    let repos = |state: &AppState| -> Vec<(String, u32)> {
        state
            .nav_nodes
            .iter()
            .filter_map(|n| match n {
                NavNode::Repo {
                    name, open_count, ..
                } => Some((name.clone(), *open_count)),
                _ => None,
            })
            .collect()
    };

    assert_eq!(
        repos(&state),
        vec![("many-prs".into(), 5), ("few-prs".into(), 1)]
    );
    update(&mut state, Action::ToggleListMode);
    assert_eq!(
        repos(&state),
        vec![("few-prs".into(), 9), ("many-prs".into(), 2)]
    );
}

// --- Issue lists ---

#[test]
fn issue_lists_follow_content_view() {
    let mut state = make_state();
    update(&mut state, Action::ToggleListMode);
    load_issues(
        &mut state,
        vec![
            make_issue("a", 1, "one", &[]),
            make_issue("b", 2, "two", &[]),
        ],
    );
    update(
        &mut state,
        Action::DataLoaded(DataPayload::IssueInbox {
            issues: vec![make_issue("a", 1, "one", &[])],
            rate_limit: RateLimit::default(),
        }),
    );

    state.content_view = ContentView::Inbox;
    assert_eq!(state.current_issue_list().len(), 1);

    state.content_view = ContentView::AllOpenPrs;
    assert_eq!(state.current_issue_list().len(), 2);

    state.content_view = ContentView::RepoPrList {
        owner: "org-a".into(),
        name: "b".into(),
    };
    let issues = state.current_issue_list();
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].number, 2);
}

#[test]
fn modes_do_not_leak_into_each_other() {
    let mut state = make_state();
    state.content_view = ContentView::AllOpenPrs;
    update(
        &mut state,
        Action::DataLoaded(DataPayload::AllOpenPrs {
            prs: vec![make_pr("a", 7)],
            rate_limit: RateLimit::default(),
        }),
    );
    load_issues(&mut state, vec![make_issue("a", 1, "one", &[])]);

    assert_eq!(state.current_pr_list().len(), 1);
    assert!(state.current_issue_list().is_empty());

    update(&mut state, Action::ToggleListMode);
    assert!(state.current_pr_list().is_empty());
    assert!(state.selected_pr().is_none());
    assert_eq!(state.current_issue_list().len(), 1);
}

#[test]
fn search_matches_issue_labels() {
    let mut state = make_state();
    update(&mut state, Action::ToggleListMode);
    state.content_view = ContentView::AllOpenPrs;
    load_issues(
        &mut state,
        vec![
            make_issue("a", 1, "one", &["Bug"]),
            make_issue("a", 2, "two", &["feature"]),
        ],
    );
    update(&mut state, Action::ToggleSearch);
    for ch in "bug".chars() {
        update(&mut state, Action::SearchInput(ch));
    }
    let issues = state.current_issue_list();
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].number, 1);
}

#[test]
fn enter_and_open_use_selected_issue_url() {
    let mut state = make_state();
    update(&mut state, Action::ToggleListMode);
    state.content_view = ContentView::AllOpenPrs;
    state.focused_pane = FocusedPane::Content;
    load_issues(
        &mut state,
        vec![
            make_issue("a", 1, "one", &[]),
            make_issue("a", 2, "two", &[]),
        ],
    );

    update(&mut state, Action::MoveDown);
    assert_eq!(state.content_cursor, 1);
    update(&mut state, Action::MoveDown);
    assert_eq!(state.content_cursor, 1, "cursor stays on the last issue");

    let effects = update(&mut state, Action::OpenInBrowser);
    assert!(matches!(
        &effects[0],
        SideEffect::OpenUrl(url) if url == "https://github.com/org-a/a/issues/2"
    ));
}

#[test]
fn pr_only_keys_do_nothing_in_issues_mode() {
    let mut state = make_state();
    update(&mut state, Action::ToggleListMode);
    state.focused_pane = FocusedPane::Content;

    update(&mut state, Action::ToggleGitLog);
    assert_eq!(state.overlay, Overlay::None);
    update(&mut state, Action::ToggleDiff);
    assert_eq!(state.overlay, Overlay::None);
    let before = state.merge_filter;
    update(&mut state, Action::CycleMergeFilter);
    assert_eq!(state.merge_filter, before);
}

// --- Scrolling ---

fn render_content(state: &AppState, height: u16) -> String {
    use ratatui::{Terminal, backend::TestBackend};
    let mut terminal = Terminal::new(TestBackend::new(120, height)).unwrap();
    terminal
        .draw(|f| ghdash::ui::widgets::render_content_pane(f, f.area(), state))
        .unwrap();
    let buffer = terminal.backend().buffer();
    buffer
        .content()
        .chunks(buffer.area.width as usize)
        .map(|row| row.iter().map(|c| c.symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn issue_table_scrolls_to_keep_cursor_visible() {
    let mut state = make_state();
    update(&mut state, Action::ToggleListMode);
    state.content_view = ContentView::AllOpenPrs;
    state.focused_pane = FocusedPane::Content;
    load_issues(
        &mut state,
        (1..=60)
            .map(|n| make_issue("a", n, &format!("issue-{n}"), &[]))
            .collect(),
    );
    for _ in 0..50 {
        update(&mut state, Action::MoveDown);
    }

    let screen = render_content(&state, 15);
    assert!(
        screen.contains("issue-51"),
        "cursor row must be on screen:\n{screen}"
    );
    assert!(
        !screen.contains("issue-1 "),
        "top rows scrolled away:\n{screen}"
    );

    // Moving back up a little keeps the viewport where it was instead of
    // jumping, until the cursor reaches its top edge.
    update(&mut state, Action::MoveUp);
    let screen = render_content(&state, 15);
    assert!(screen.contains("issue-50"));
    assert!(screen.contains("issue-51"));
}

#[test]
fn pr_table_scrolls_to_keep_cursor_visible() {
    let mut state = make_state();
    state.content_view = ContentView::AllOpenPrs;
    state.focused_pane = FocusedPane::Content;
    update(
        &mut state,
        Action::DataLoaded(DataPayload::AllOpenPrs {
            prs: (1..=60).map(|n| make_pr("a", n)).collect(),
            rate_limit: RateLimit::default(),
        }),
    );
    for _ in 0..50 {
        update(&mut state, Action::MoveDown);
    }

    let screen = render_content(&state, 15);
    assert!(
        screen.contains("#51 "),
        "cursor row must be on screen:\n{screen}"
    );
}
