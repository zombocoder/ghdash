use crate::github::models::{PullRequest, RateLimit, Repo};

#[derive(Debug)]
#[allow(dead_code)]
pub enum Action {
    MoveUp,
    MoveDown,
    Select,
    Back,
    SwitchPane,
    Refresh,
    OpenInBrowser,
    ToggleSearch,
    SearchInput(char),
    SearchBackspace,
    SearchClear,
    DataLoaded(DataPayload),
    LoadError(String),
    DismissError,
    Quit,
    Tick,
}

#[derive(Debug)]
pub enum DataPayload {
    OrgRepos {
        org: String,
        repos: Vec<Repo>,
        rate_limit: RateLimit,
    },
    InboxPrs {
        prs: Vec<PullRequest>,
        rate_limit: RateLimit,
    },
    AllOpenPrs {
        prs: Vec<PullRequest>,
        rate_limit: RateLimit,
    },
}

#[derive(Debug)]
pub enum SideEffect {
    RefreshAll,
    FetchOrgRepos(String),
    FetchUserRepos(String),
    FetchInbox,
    FetchAllOpenPrs,
    OpenUrl(String),
}
