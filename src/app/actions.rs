use crate::github::models::{PrDetail, PullRequest, RateLimit, Repo};

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
    ToggleGitLog,
    ToggleDiff,
    CloseOverlay,
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
    PrDetailLoaded {
        /// PR url — the key into `AppState::pr_details`.
        key: String,
        detail: PrDetail,
        rate_limit: RateLimit,
    },
    PrDetailFailed {
        key: String,
        msg: String,
    },
    PrDiffLoaded {
        /// PR url — the key into `AppState::pr_diffs`.
        key: String,
        diff: String,
    },
    PrDiffFailed {
        key: String,
        msg: String,
    },
}

#[derive(Debug)]
pub enum SideEffect {
    RefreshAll,
    FetchOrgRepos(String),
    FetchUserRepos(String),
    FetchInbox,
    FetchAllOpenPrs,
    FetchPrDetail {
        owner: String,
        name: String,
        number: u32,
        /// PR url — echoed back so the result can be stored under the right key.
        key: String,
    },
    FetchPrDiff {
        owner: String,
        name: String,
        number: u32,
        /// PR url — echoed back so the result can be stored under the right key.
        key: String,
    },
    OpenUrl(String),
}
