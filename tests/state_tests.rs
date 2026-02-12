use ghdash::app::actions::{Action, DataPayload, SideEffect};
use ghdash::app::state::{AppState, ContentView, FocusedPane, NavNode};
use ghdash::app::update::update;
use ghdash::github::models::{PullRequest, RateLimit, Repo};

fn make_state() -> AppState {
    AppState::new("testuser".into(), vec!["org-a".into(), "org-b".into()])
}

fn make_repo(owner: &str, name: &str, open_prs: u32) -> Repo {
    Repo {
        name: name.into(),
        owner: owner.into(),
        url: format!("https://github.com/{}/{}", owner, name),
        description: None,
        open_pr_count: open_prs,
        is_archived: false,
    }
}

fn make_pr(repo_owner: &str, repo_name: &str, number: u32, title: &str) -> PullRequest {
    PullRequest {
        number,
        title: title.into(),
        author: "author".into(),
        repo_owner: repo_owner.into(),
        repo_name: repo_name.into(),
        url: format!(
            "https://github.com/{}/{}/pull/{}",
            repo_owner, repo_name, number
        ),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        is_draft: false,
        additions: 10,
        deletions: 5,
        review_decision: None,
        labels: vec![],
    }
}

// --- Initial state ---

#[test]
fn test_initial_state_has_nav_nodes() {
    let state = make_state();
    // Should have: MyInbox, AllPrs, Org(org-a), Org(org-b)
    assert_eq!(state.nav_nodes.len(), 4);
    assert!(matches!(&state.nav_nodes[0], NavNode::MyInbox));
    assert!(matches!(&state.nav_nodes[1], NavNode::AllPrs));
}

#[test]
fn test_initial_state_defaults() {
    let state = make_state();
    assert_eq!(state.focused_pane, FocusedPane::Navigation);
    assert_eq!(state.content_view, ContentView::Inbox);
    assert_eq!(state.nav_cursor, 0);
    assert_eq!(state.content_cursor, 0);
    assert!(state.loading);
    assert!(!state.should_quit);
    assert!(!state.search_active);
    assert!(state.search_query.is_empty());
}

// --- Navigation ---

#[test]
fn test_move_down_increments_nav_cursor() {
    let mut state = make_state();
    update(&mut state, Action::MoveDown);
    assert_eq!(state.nav_cursor, 1);
    update(&mut state, Action::MoveDown);
    assert_eq!(state.nav_cursor, 2);
}

#[test]
fn test_move_up_decrements_nav_cursor() {
    let mut state = make_state();
    state.nav_cursor = 2;
    update(&mut state, Action::MoveUp);
    assert_eq!(state.nav_cursor, 1);
}

#[test]
fn test_move_up_at_zero_stays() {
    let mut state = make_state();
    assert_eq!(state.nav_cursor, 0);
    update(&mut state, Action::MoveUp);
    assert_eq!(state.nav_cursor, 0);
}

#[test]
fn test_move_down_at_end_stays() {
    let mut state = make_state();
    let max = state.nav_nodes.len() - 1;
    state.nav_cursor = max;
    update(&mut state, Action::MoveDown);
    assert_eq!(state.nav_cursor, max);
}

// --- Pane switching ---

#[test]
fn test_switch_pane() {
    let mut state = make_state();
    assert_eq!(state.focused_pane, FocusedPane::Navigation);
    update(&mut state, Action::SwitchPane);
    assert_eq!(state.focused_pane, FocusedPane::Content);
    update(&mut state, Action::SwitchPane);
    assert_eq!(state.focused_pane, FocusedPane::Navigation);
}

// --- Select ---

#[test]
fn test_select_inbox() {
    let mut state = make_state();
    // Cursor at 0 = MyInbox
    update(&mut state, Action::Select);
    assert_eq!(state.content_view, ContentView::Inbox);
}

#[test]
fn test_select_all_prs() {
    let mut state = make_state();
    state.nav_cursor = 1; // AllPrs
    update(&mut state, Action::Select);
    assert_eq!(state.content_view, ContentView::AllOpenPrs);
}

#[test]
fn test_select_org_toggles_expand() {
    let mut state = make_state();
    state.nav_cursor = 2; // First org

    let org_name = match &state.nav_nodes[2] {
        NavNode::Org(name) => name.clone(),
        _ => panic!("Expected Org node"),
    };

    // Initially expanded, selecting should collapse
    assert!(state.nav_expanded.contains(&org_name));
    update(&mut state, Action::Select);
    assert!(!state.nav_expanded.contains(&org_name));

    // Select again to expand
    // After collapse, nav tree changed; find the org again
    let org_idx = state
        .nav_nodes
        .iter()
        .position(|n| matches!(n, NavNode::Org(name) if *name == org_name))
        .unwrap();
    state.nav_cursor = org_idx;
    update(&mut state, Action::Select);
    assert!(state.nav_expanded.contains(&org_name));
}

// --- DataLoaded ---

#[test]
fn test_data_loaded_org_repos() {
    let mut state = make_state();
    let repos = vec![
        make_repo("org-a", "repo1", 3),
        make_repo("org-a", "repo2", 0),
    ];

    update(
        &mut state,
        Action::DataLoaded(DataPayload::OrgRepos {
            org: "org-a".into(),
            repos,
            rate_limit: RateLimit {
                remaining: 4999,
                limit: 5000,
                reset_at: None,
            },
        }),
    );

    let org_data = state.orgs.get("org-a").unwrap();
    assert_eq!(org_data.repos.len(), 2);
    assert_eq!(state.rate_limit.remaining, 4999);

    // Nav tree should now include repos under org-a
    let repo_nodes: Vec<_> = state
        .nav_nodes
        .iter()
        .filter(|n| matches!(n, NavNode::Repo { owner, .. } if owner == "org-a"))
        .collect();
    assert_eq!(repo_nodes.len(), 2);
}

#[test]
fn test_data_loaded_inbox() {
    let mut state = make_state();
    let prs = vec![make_pr("org-a", "repo1", 1, "Fix bug")];

    update(
        &mut state,
        Action::DataLoaded(DataPayload::InboxPrs {
            prs: prs.clone(),
            rate_limit: RateLimit::default(),
        }),
    );

    assert_eq!(state.inbox.len(), 1);
    assert_eq!(state.inbox[0].title, "Fix bug");
}

#[test]
fn test_data_loaded_all_open_prs() {
    let mut state = make_state();
    let prs = vec![
        make_pr("org-a", "repo1", 1, "PR 1"),
        make_pr("org-a", "repo1", 2, "PR 2"),
    ];

    update(
        &mut state,
        Action::DataLoaded(DataPayload::AllOpenPrs {
            prs,
            rate_limit: RateLimit::default(),
        }),
    );

    assert_eq!(state.all_open_prs.len(), 2);
}

// --- Loading state ---

#[test]
fn test_loading_completes_when_no_orgs_loading() {
    let mut state = make_state();
    assert!(state.loading);

    // Simulate data loaded (loading_orgs is empty by default)
    update(
        &mut state,
        Action::DataLoaded(DataPayload::InboxPrs {
            prs: vec![],
            rate_limit: RateLimit::default(),
        }),
    );

    assert!(!state.loading);
    assert!(state.last_refresh.is_some());
}

// --- Error handling ---

#[test]
fn test_load_error_sets_message() {
    let mut state = make_state();
    update(&mut state, Action::LoadError("Network error".into()));
    assert_eq!(state.error_message, Some("Network error".into()));
    assert!(!state.loading);
}

#[test]
fn test_dismiss_error() {
    let mut state = make_state();
    state.error_message = Some("err".into());
    update(&mut state, Action::DismissError);
    assert!(state.error_message.is_none());
}

// --- Refresh ---

#[test]
fn test_refresh_returns_side_effect() {
    let mut state = make_state();
    let effects = update(&mut state, Action::Refresh);
    assert!(state.loading);
    assert!(state.error_message.is_none());
    assert_eq!(effects.len(), 1);
    assert!(matches!(effects[0], SideEffect::RefreshAll));
}

// --- Search ---

#[test]
fn test_toggle_search() {
    let mut state = make_state();
    assert!(!state.search_active);
    update(&mut state, Action::ToggleSearch);
    assert!(state.search_active);
    update(&mut state, Action::ToggleSearch);
    assert!(!state.search_active);
}

#[test]
fn test_search_input() {
    let mut state = make_state();
    update(&mut state, Action::ToggleSearch);
    update(&mut state, Action::SearchInput('h'));
    update(&mut state, Action::SearchInput('i'));
    assert_eq!(state.search_query, "hi");
}

#[test]
fn test_search_backspace() {
    let mut state = make_state();
    update(&mut state, Action::ToggleSearch);
    update(&mut state, Action::SearchInput('a'));
    update(&mut state, Action::SearchInput('b'));
    update(&mut state, Action::SearchBackspace);
    assert_eq!(state.search_query, "a");
}

#[test]
fn test_search_filters_prs() {
    let mut state = make_state();
    state.content_view = ContentView::Inbox;
    state.inbox = vec![
        make_pr("org", "repo", 1, "Fix login bug"),
        make_pr("org", "repo", 2, "Add dashboard feature"),
        make_pr("org", "repo", 3, "Login page redesign"),
    ];

    state.search_active = true;
    state.search_query = "login".into();

    let filtered = state.current_pr_list();
    assert_eq!(filtered.len(), 2);
    assert!(
        filtered
            .iter()
            .all(|pr| pr.title.to_lowercase().contains("login"))
    );
}

// --- Quit ---

#[test]
fn test_quit() {
    let mut state = make_state();
    update(&mut state, Action::Quit);
    assert!(state.should_quit);
}

// --- Back ---

#[test]
fn test_back_closes_search() {
    let mut state = make_state();
    state.search_active = true;
    state.search_query = "test".into();
    update(&mut state, Action::Back);
    assert!(!state.search_active);
    assert!(state.search_query.is_empty());
}

#[test]
fn test_back_dismisses_error() {
    let mut state = make_state();
    state.error_message = Some("err".into());
    update(&mut state, Action::Back);
    assert!(state.error_message.is_none());
}

#[test]
fn test_back_switches_to_nav_pane() {
    let mut state = make_state();
    state.focused_pane = FocusedPane::Content;
    update(&mut state, Action::Back);
    assert_eq!(state.focused_pane, FocusedPane::Navigation);
}

// --- Open in browser ---

#[test]
fn test_open_in_browser_from_content_with_pr() {
    let mut state = make_state();
    state.focused_pane = FocusedPane::Content;
    state.content_view = ContentView::Inbox;
    state.inbox = vec![make_pr("org", "repo", 42, "My PR")];
    state.content_cursor = 0;

    let effects = update(&mut state, Action::OpenInBrowser);
    assert_eq!(effects.len(), 1);
    match &effects[0] {
        SideEffect::OpenUrl(url) => {
            assert!(url.contains("42"));
        }
        _ => panic!("Expected OpenUrl side effect"),
    }
}

#[test]
fn test_open_in_browser_from_nav_on_org() {
    let mut state = make_state();
    state.focused_pane = FocusedPane::Navigation;
    // Find an Org node
    let org_idx = state
        .nav_nodes
        .iter()
        .position(|n| matches!(n, NavNode::Org(_)))
        .unwrap();
    state.nav_cursor = org_idx;

    let effects = update(&mut state, Action::OpenInBrowser);
    assert_eq!(effects.len(), 1);
    assert!(
        matches!(&effects[0], SideEffect::OpenUrl(url) if url.starts_with("https://github.com/"))
    );
}

// --- Nav tree rebuild with repos ---

#[test]
fn test_nav_tree_sorts_repos_by_pr_count() {
    let mut state = make_state();

    let repos = vec![
        make_repo("org-a", "low-prs", 1),
        make_repo("org-a", "high-prs", 10),
        make_repo("org-a", "mid-prs", 5),
    ];

    update(
        &mut state,
        Action::DataLoaded(DataPayload::OrgRepos {
            org: "org-a".into(),
            repos,
            rate_limit: RateLimit::default(),
        }),
    );

    let repo_names: Vec<String> = state
        .nav_nodes
        .iter()
        .filter_map(|n| match n {
            NavNode::Repo { owner, name, .. } if owner == "org-a" => Some(name.clone()),
            _ => None,
        })
        .collect();

    assert_eq!(repo_names, vec!["high-prs", "mid-prs", "low-prs"]);
}

#[test]
fn test_archived_repos_excluded_from_nav() {
    let mut state = make_state();

    let mut archived = make_repo("org-a", "old-repo", 0);
    archived.is_archived = true;
    let repos = vec![make_repo("org-a", "active-repo", 2), archived];

    update(
        &mut state,
        Action::DataLoaded(DataPayload::OrgRepos {
            org: "org-a".into(),
            repos,
            rate_limit: RateLimit::default(),
        }),
    );

    let repo_names: Vec<String> = state
        .nav_nodes
        .iter()
        .filter_map(|n| match n {
            NavNode::Repo { name, .. } => Some(name.clone()),
            _ => None,
        })
        .collect();

    assert_eq!(repo_names, vec!["active-repo"]);
}
