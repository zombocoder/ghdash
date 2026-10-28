use std::cell::Cell;
use std::collections::{HashMap, HashSet};

use crate::github::models::{Issue, PrDetail, PullRequest, RateLimit, Repo, RepoVisibility};

/// State of an on-demand PR detail fetch, keyed by PR url in `AppState::pr_details`.
#[derive(Debug, Clone)]
pub enum PrDetailEntry {
    Loading,
    Loaded(PrDetail),
    Failed(String),
}

/// State of an on-demand PR diff fetch, keyed by PR url in `AppState::pr_diffs`.
#[derive(Debug, Clone)]
pub enum DiffEntry {
    Loading,
    Loaded(String),
    Failed(String),
}

/// Which full-screen overlay (if any) is shown for the highlighted PR.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Overlay {
    None,
    /// Recent commits for the PR ("git log").
    GitLog,
    /// Full unified diff for the PR.
    Diff,
}

/// Merge-state filter applied to PR lists. Cycled with the filter key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeFilter {
    All,
    Conflicting,
    Clean,
}

impl MergeFilter {
    /// Cycle All -> Conflicting -> Clean -> All.
    pub fn next(self) -> Self {
        match self {
            MergeFilter::All => MergeFilter::Conflicting,
            MergeFilter::Conflicting => MergeFilter::Clean,
            MergeFilter::Clean => MergeFilter::All,
        }
    }

    /// Short label for the table title; `None` when inactive (All).
    pub fn label(self) -> Option<&'static str> {
        match self {
            MergeFilter::All => None,
            MergeFilter::Conflicting => Some("conflicting"),
            MergeFilter::Clean => Some("clean"),
        }
    }

    fn matches(self, pr: &PullRequest) -> bool {
        match self {
            MergeFilter::All => true,
            MergeFilter::Conflicting => pr.mergeable.as_deref() == Some("CONFLICTING"),
            MergeFilter::Clean => pr.mergeable.as_deref() == Some("MERGEABLE"),
        }
    }
}

/// What the whole dashboard lists: pull requests (default) or issues.
/// Toggled with `i`; the nav tree, content views and counts follow it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ListMode {
    PullRequests,
    Issues,
}

impl ListMode {
    pub fn toggle(self) -> Self {
        match self {
            ListMode::PullRequests => ListMode::Issues,
            ListMode::Issues => ListMode::PullRequests,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FocusedPane {
    Navigation,
    Content,
}

/// The PR-named variants are shared by both modes: in `ListMode::Issues`,
/// `RepoPrList`/`AllOpenPrs`/`Inbox` show the matching issue lists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentView {
    OrgOverview(String),
    RepoPrList { owner: String, name: String },
    AllOpenPrs,
    Inbox,
}

#[derive(Debug, Clone)]
pub enum NavNode {
    Org(String),
    Repo {
        owner: String,
        name: String,
        /// Open PRs or open issues, depending on the current `ListMode`.
        open_count: u32,
    },
    AllPrs,
    MyInbox,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct OrgData {
    pub name: String,
    pub repos: Vec<Repo>,
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct AppState {
    // Data
    pub orgs: HashMap<String, OrgData>,
    pub all_open_prs: Vec<PullRequest>,
    pub inbox: Vec<PullRequest>,
    pub all_open_issues: Vec<Issue>,
    pub issue_inbox: Vec<Issue>,
    pub viewer_login: String,
    pub rate_limit: RateLimit,
    pub last_refresh: Option<chrono::DateTime<chrono::Utc>>,

    // List mode. `loaded_modes` tracks which modes' lists have been fetched
    // since the last refresh, so toggling only fetches what's missing.
    pub mode: ListMode,
    pub loaded_modes: HashSet<ListMode>,

    // Navigation
    pub nav_nodes: Vec<NavNode>,
    pub nav_cursor: usize,
    pub nav_expanded: HashSet<String>,
    pub focused_pane: FocusedPane,
    pub content_view: ContentView,
    pub content_cursor: usize,
    /// First visible row of the content table. Owned by the renderer, which
    /// is the only place that knows the viewport height; kept across frames
    /// so the view only scrolls when the cursor leaves it.
    pub content_offset: Cell<usize>,

    // Search
    pub search_active: bool,
    pub search_query: String,

    // PR overlays (git log / diff), fetched on-highlight while open
    pub overlay: Overlay,
    pub pr_details: HashMap<String, PrDetailEntry>,
    pub pr_diffs: HashMap<String, DiffEntry>,
    /// Vertical scroll offset (in lines) for the diff overlay.
    pub diff_scroll: u16,

    // Merge-state filter + help overlay
    pub merge_filter: MergeFilter,
    pub help_open: bool,

    /// Repo visibility the session is restricted to (`github.visibility`).
    /// Read-only here; the fetches are already scoped, this only drives the
    /// status-bar indicator so a restricted session is visibly restricted.
    pub visibility: RepoVisibility,

    // UI flags
    pub loading: bool,
    pub loading_orgs: HashSet<String>,
    pub error_message: Option<String>,
    pub should_quit: bool,
}

impl AppState {
    pub fn new(viewer_login: String, org_names: Vec<String>) -> Self {
        let mut orgs = HashMap::new();
        let mut nav_expanded = HashSet::new();

        for name in &org_names {
            orgs.insert(
                name.clone(),
                OrgData {
                    name: name.clone(),
                    repos: Vec::new(),
                },
            );
            nav_expanded.insert(name.clone());
        }

        let mut state = Self {
            orgs,
            all_open_prs: Vec::new(),
            inbox: Vec::new(),
            all_open_issues: Vec::new(),
            issue_inbox: Vec::new(),
            viewer_login,
            rate_limit: RateLimit::default(),
            last_refresh: None,
            mode: ListMode::PullRequests,
            loaded_modes: HashSet::from([ListMode::PullRequests]),
            nav_nodes: Vec::new(),
            nav_cursor: 0,
            nav_expanded,
            focused_pane: FocusedPane::Navigation,
            content_view: ContentView::Inbox,
            content_cursor: 0,
            content_offset: Cell::new(0),
            search_active: false,
            search_query: String::new(),
            overlay: Overlay::None,
            pr_details: HashMap::new(),
            pr_diffs: HashMap::new(),
            diff_scroll: 0,
            merge_filter: MergeFilter::All,
            help_open: false,
            visibility: RepoVisibility::All,
            loading: true,
            loading_orgs: HashSet::new(),
            error_message: None,
            should_quit: false,
        };

        state.rebuild_nav_tree();
        state
    }

    pub fn rebuild_nav_tree(&mut self) {
        let mut nodes = Vec::new();

        // Virtual entries at top
        nodes.push(NavNode::MyInbox);
        nodes.push(NavNode::AllPrs);

        // Org entries sorted by name
        let mut org_names: Vec<_> = self.orgs.keys().cloned().collect();
        org_names.sort();

        for org_name in &org_names {
            nodes.push(NavNode::Org(org_name.clone()));

            if self.nav_expanded.contains(org_name)
                && let Some(org_data) = self.orgs.get(org_name)
            {
                let mut repos: Vec<_> = org_data.repos.iter().filter(|r| !r.is_archived).collect();
                repos.sort_by(|a, b| {
                    self.repo_open_count(b)
                        .cmp(&self.repo_open_count(a))
                        .then(a.name.cmp(&b.name))
                });

                for repo in repos {
                    nodes.push(NavNode::Repo {
                        owner: repo.owner.clone(),
                        name: repo.name.clone(),
                        open_count: self.repo_open_count(repo),
                    });
                }
            }
        }

        self.nav_nodes = nodes;

        // Clamp cursor
        if !self.nav_nodes.is_empty() && self.nav_cursor >= self.nav_nodes.len() {
            self.nav_cursor = self.nav_nodes.len() - 1;
        }
    }

    /// Open PRs or open issues for `repo`, depending on the current mode.
    pub fn repo_open_count(&self, repo: &Repo) -> u32 {
        match self.mode {
            ListMode::PullRequests => repo.open_pr_count,
            ListMode::Issues => repo.open_issue_count,
        }
    }

    pub fn filtered_prs(&self, prs: &[PullRequest]) -> Vec<PullRequest> {
        let query = self.search_query.to_lowercase();
        prs.iter()
            .filter(|pr| self.merge_filter.matches(pr))
            .filter(|pr| {
                query.is_empty()
                    || pr.title.to_lowercase().contains(&query)
                    || pr.author.to_lowercase().contains(&query)
                    || pr.repo_name.to_lowercase().contains(&query)
                    || pr.repo_full_name().to_lowercase().contains(&query)
            })
            .cloned()
            .collect()
    }

    /// The PRs listed in the content pane. Empty in issues mode, which keeps
    /// the PR-only overlays (git log / diff) from ever targeting a row.
    pub fn current_pr_list(&self) -> Vec<PullRequest> {
        if self.mode == ListMode::Issues {
            return Vec::new();
        }
        let prs = match &self.content_view {
            ContentView::Inbox => &self.inbox,
            ContentView::AllOpenPrs => &self.all_open_prs,
            ContentView::RepoPrList { owner, name } => {
                let full_name = format!("{}/{}", owner, name);
                let filtered: Vec<PullRequest> = self
                    .all_open_prs
                    .iter()
                    .filter(|pr| pr.repo_full_name() == full_name)
                    .cloned()
                    .collect();
                return self.filtered_prs(&filtered);
            }
            ContentView::OrgOverview(_) => return Vec::new(),
        };
        self.filtered_prs(prs)
    }

    pub fn filtered_issues(&self, issues: &[Issue]) -> Vec<Issue> {
        let query = self.search_query.to_lowercase();
        issues
            .iter()
            .filter(|issue| {
                query.is_empty()
                    || issue.title.to_lowercase().contains(&query)
                    || issue.author.to_lowercase().contains(&query)
                    || issue.repo_full_name().to_lowercase().contains(&query)
                    || issue
                        .labels
                        .iter()
                        .any(|l| l.to_lowercase().contains(&query))
            })
            .cloned()
            .collect()
    }

    /// The issues listed in the content pane. Empty in PR mode.
    pub fn current_issue_list(&self) -> Vec<Issue> {
        if self.mode == ListMode::PullRequests {
            return Vec::new();
        }
        let issues = match &self.content_view {
            ContentView::Inbox => &self.issue_inbox,
            ContentView::AllOpenPrs => &self.all_open_issues,
            ContentView::RepoPrList { owner, name } => {
                let full_name = format!("{}/{}", owner, name);
                let filtered: Vec<Issue> = self
                    .all_open_issues
                    .iter()
                    .filter(|issue| issue.repo_full_name() == full_name)
                    .cloned()
                    .collect();
                return self.filtered_issues(&filtered);
            }
            ContentView::OrgOverview(_) => return Vec::new(),
        };
        self.filtered_issues(issues)
    }

    /// Number of rows in the content pane for the current mode.
    pub fn current_item_count(&self) -> usize {
        match self.mode {
            ListMode::PullRequests => self.current_pr_list().len(),
            ListMode::Issues => self.current_issue_list().len(),
        }
    }

    /// URL of the highlighted row (PR or issue) in the content pane.
    pub fn selected_pr_url(&self) -> Option<String> {
        match self.mode {
            ListMode::PullRequests => self
                .current_pr_list()
                .get(self.content_cursor)
                .map(|pr| pr.url.clone()),
            ListMode::Issues => self
                .current_issue_list()
                .get(self.content_cursor)
                .map(|issue| issue.url.clone()),
        }
    }

    /// The currently highlighted PR (in the content pane), cloned.
    pub fn selected_pr(&self) -> Option<PullRequest> {
        self.current_pr_list().into_iter().nth(self.content_cursor)
    }

    /// Apply a freshly fetched merge state to the matching PR in every list, so
    /// the list column reflects the authoritative value once detail resolves.
    pub fn apply_fresh_merge_state(
        &mut self,
        url: &str,
        mergeable: Option<String>,
        merge_state_status: Option<String>,
    ) {
        for pr in self.all_open_prs.iter_mut().chain(self.inbox.iter_mut()) {
            if pr.url == url {
                pr.mergeable = mergeable.clone();
                pr.merge_state_status = merge_state_status.clone();
            }
        }
    }

    pub fn selected_nav_url(&self) -> Option<String> {
        self.nav_nodes
            .get(self.nav_cursor)
            .and_then(|node| match node {
                NavNode::Repo { owner, name, .. } => {
                    Some(format!("https://github.com/{}/{}", owner, name))
                }
                NavNode::Org(org) => Some(format!("https://github.com/{}", org)),
                _ => None,
            })
    }
}
