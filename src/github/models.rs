use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Which repositories the dashboard is allowed to show. Configured once via
/// `github.visibility`; `All` is the default. Restricting it hides matching
/// repos from the nav tree *and* narrows the PR searches, so nothing from a
/// hidden repo reaches the screen (useful when screen-sharing or streaming).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum RepoVisibility {
    #[default]
    All,
    Public,
    Private,
}

impl RepoVisibility {
    /// Whether a repo with this privacy flag may be shown.
    pub fn allows(self, is_private: bool) -> bool {
        match self {
            RepoVisibility::All => true,
            RepoVisibility::Public => !is_private,
            RepoVisibility::Private => is_private,
        }
    }

    /// GitHub search qualifier that restricts results to this visibility.
    /// `None` for `All` (no qualifier needed).
    pub fn search_qualifier(self) -> Option<&'static str> {
        match self {
            RepoVisibility::All => None,
            RepoVisibility::Public => Some("is:public"),
            RepoVisibility::Private => Some("is:private"),
        }
    }

    /// Status-bar label; `None` when unrestricted.
    pub fn label(self) -> Option<&'static str> {
        match self {
            RepoVisibility::All => None,
            RepoVisibility::Public => Some("public only"),
            RepoVisibility::Private => Some("private only"),
        }
    }

    /// Discriminator for cache keys, so PR lists fetched under one visibility
    /// are never served to a session running under another.
    pub fn cache_suffix(self) -> &'static str {
        match self {
            RepoVisibility::All => "all",
            RepoVisibility::Public => "public",
            RepoVisibility::Private => "private",
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewerInfo {
    pub login: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Repo {
    pub name: String,
    pub owner: String,
    pub url: String,
    pub description: Option<String>,
    pub open_pr_count: u32,
    pub is_archived: bool,
    /// Required (not `#[serde(default)]`) on purpose: cache entries written
    /// before this field existed must fail to parse and be refetched rather
    /// than defaulting private repos to public. See `CacheStore::get`.
    pub is_private: bool,
}

impl Repo {
    pub fn full_name(&self) -> String {
        format!("{}/{}", self.owner, self.name)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullRequest {
    pub number: u32,
    pub title: String,
    pub author: String,
    pub repo_owner: String,
    pub repo_name: String,
    pub url: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub is_draft: bool,
    pub additions: u32,
    pub deletions: u32,
    pub review_decision: Option<String>,
    pub labels: Vec<String>,
    /// GitHub `mergeable` enum: `MERGEABLE` / `CONFLICTING` / `UNKNOWN`.
    /// `None` when absent (e.g. older cache entries). Note: GitHub computes this
    /// lazily, so the search API frequently returns `UNKNOWN`.
    #[serde(default)]
    pub mergeable: Option<String>,
    /// GitHub `mergeStateStatus` enum: `CLEAN` / `DIRTY` / `BLOCKED` / `BEHIND` /
    /// `UNSTABLE` / `HAS_HOOKS` / `DRAFT` / `UNKNOWN`. Richer than `mergeable`;
    /// same lazy-compute caveat.
    #[serde(default)]
    pub merge_state_status: Option<String>,
    /// `statusCheckRollup.state` of the PR's latest commit: `SUCCESS` / `FAILURE` /
    /// `PENDING` / `ERROR` / `EXPECTED`. Unlike `mergeable`, this is not computed
    /// lazily, so the search API returns real values. `None` = no checks / absent.
    #[serde(default)]
    pub checks_status: Option<String>,
}

/// Coarse CI outcome derived from `checks_status`, decoupled from the raw GitHub
/// enum so the UI (and tests) don't hard-code string matching.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CiStatus {
    Passing,
    Failing,
    Pending,
    None,
}

impl PullRequest {
    pub fn repo_full_name(&self) -> String {
        format!("{}/{}", self.repo_owner, self.repo_name)
    }

    /// Classify the CI check rollup into a coarse outcome for display.
    pub fn ci_status(&self) -> CiStatus {
        match self.checks_status.as_deref() {
            Some("SUCCESS") => CiStatus::Passing,
            Some("FAILURE") | Some("ERROR") => CiStatus::Failing,
            Some("PENDING") | Some("EXPECTED") => CiStatus::Pending,
            _ => CiStatus::None,
        }
    }
}

/// A single commit shown in the PR detail pane ("git log").
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitInfo {
    pub oid: String,
    pub headline: String,
    pub committed_date: DateTime<Utc>,
    pub author: String,
}

impl CommitInfo {
    /// Short 7-char SHA for display.
    pub fn short_oid(&self) -> &str {
        let end = self.oid.len().min(7);
        &self.oid[..end]
    }
}

/// On-demand detail for a single PR, fetched when its row is highlighted.
/// Unlike the list, this forces GitHub to compute a fresh `mergeable`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrDetail {
    pub mergeable: Option<String>,
    pub merge_state_status: Option<String>,
    /// `statusCheckRollup.state`: `SUCCESS` / `FAILURE` / `PENDING` / `ERROR` / `EXPECTED`.
    pub checks_status: Option<String>,
    /// Recent commits, oldest-first as returned by GitHub (`commits(last: N)`).
    pub commits: Vec<CommitInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RateLimit {
    pub remaining: u32,
    pub limit: u32,
    pub reset_at: Option<DateTime<Utc>>,
}
