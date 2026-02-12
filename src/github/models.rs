use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

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
}

impl PullRequest {
    pub fn repo_full_name(&self) -> String {
        format!("{}/{}", self.repo_owner, self.repo_name)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RateLimit {
    pub remaining: u32,
    pub limit: u32,
    pub reset_at: Option<DateTime<Utc>>,
}
