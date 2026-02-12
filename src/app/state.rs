use std::collections::{HashMap, HashSet};

use crate::github::models::{PullRequest, RateLimit, Repo};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FocusedPane {
    Navigation,
    Content,
}

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
        open_prs: u32,
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
    pub viewer_login: String,
    pub rate_limit: RateLimit,
    pub last_refresh: Option<chrono::DateTime<chrono::Utc>>,

    // Navigation
    pub nav_nodes: Vec<NavNode>,
    pub nav_cursor: usize,
    pub nav_expanded: HashSet<String>,
    pub focused_pane: FocusedPane,
    pub content_view: ContentView,
    pub content_cursor: usize,

    // Search
    pub search_active: bool,
    pub search_query: String,

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
            viewer_login,
            rate_limit: RateLimit::default(),
            last_refresh: None,
            nav_nodes: Vec::new(),
            nav_cursor: 0,
            nav_expanded,
            focused_pane: FocusedPane::Navigation,
            content_view: ContentView::Inbox,
            content_cursor: 0,
            search_active: false,
            search_query: String::new(),
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
                    b.open_pr_count
                        .cmp(&a.open_pr_count)
                        .then(a.name.cmp(&b.name))
                });

                for repo in repos {
                    nodes.push(NavNode::Repo {
                        owner: repo.owner.clone(),
                        name: repo.name.clone(),
                        open_prs: repo.open_pr_count,
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

    pub fn filtered_prs(&self, prs: &[PullRequest]) -> Vec<PullRequest> {
        if self.search_query.is_empty() {
            return prs.to_vec();
        }
        let query = self.search_query.to_lowercase();
        prs.iter()
            .filter(|pr| {
                pr.title.to_lowercase().contains(&query)
                    || pr.author.to_lowercase().contains(&query)
                    || pr.repo_name.to_lowercase().contains(&query)
                    || pr.repo_full_name().to_lowercase().contains(&query)
            })
            .cloned()
            .collect()
    }

    pub fn current_pr_list(&self) -> Vec<PullRequest> {
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

    pub fn selected_pr_url(&self) -> Option<String> {
        let prs = self.current_pr_list();
        prs.get(self.content_cursor).map(|pr| pr.url.clone())
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
