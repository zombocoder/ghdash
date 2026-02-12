use anyhow::{Context, Result, bail};
use reqwest::Client;
use serde_json::{Value, json};
use tracing::debug;

use super::models::*;
use super::queries;

#[derive(Clone)]
pub struct GithubClient {
    client: Client,
    api_url: String,
    token: String,
}

impl GithubClient {
    pub fn new(token: &str, api_url: &str) -> Result<Self> {
        if !api_url.starts_with("https://") {
            bail!("GitHub API URL must use HTTPS: {}", api_url);
        }

        let client = Client::builder()
            .user_agent("ghdash")
            .build()
            .context("Failed to create HTTP client")?;

        Ok(Self {
            client,
            api_url: api_url.to_string(),
            token: token.to_string(),
        })
    }

    async fn query(&self, query: &str, variables: Value) -> Result<Value> {
        let body = json!({
            "query": query,
            "variables": variables,
        });

        let resp = self
            .client
            .post(&self.api_url)
            .bearer_auth(&self.token)
            .json(&body)
            .send()
            .await
            .context("GitHub API request failed")?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            bail!("GitHub API returned {}: {}", status, text);
        }

        let data: Value = resp
            .json()
            .await
            .context("Failed to parse GitHub response")?;

        if let Some(errors) = data.get("errors") {
            let error_msg = errors
                .as_array()
                .and_then(|arr| arr.first())
                .and_then(|e| e.get("message"))
                .and_then(|m| m.as_str())
                .unwrap_or("Unknown GraphQL error");
            bail!("GraphQL error: {}", error_msg);
        }

        Ok(data)
    }

    fn extract_rate_limit(data: &Value) -> RateLimit {
        let rl = &data["data"]["rateLimit"];
        RateLimit {
            remaining: rl["remaining"].as_u64().unwrap_or(0) as u32,
            limit: rl["limit"].as_u64().unwrap_or(0) as u32,
            reset_at: rl["resetAt"].as_str().and_then(|s| s.parse().ok()),
        }
    }

    pub async fn fetch_viewer(&self) -> Result<String> {
        let data = self.query(queries::VIEWER_QUERY, json!({})).await?;
        let login = data["data"]["viewer"]["login"]
            .as_str()
            .context("Missing viewer login")?
            .to_string();
        debug!(login = %login, "Fetched viewer");
        Ok(login)
    }

    pub async fn fetch_org_repos(&self, org: &str) -> Result<(Vec<Repo>, RateLimit)> {
        let mut all_repos = Vec::new();
        let mut cursor: Option<String> = None;
        let mut rate_limit;

        loop {
            let variables = json!({
                "org": org,
                "cursor": cursor,
            });

            let data = self.query(queries::ORG_REPOS_QUERY, variables).await?;
            rate_limit = Self::extract_rate_limit(&data);

            let repos_data = &data["data"]["organization"]["repositories"];
            let nodes = repos_data["nodes"]
                .as_array()
                .context("Missing repository nodes")?;

            for node in nodes {
                let repo = Repo {
                    name: node["name"].as_str().unwrap_or("").to_string(),
                    owner: node["owner"]["login"].as_str().unwrap_or("").to_string(),
                    url: node["url"].as_str().unwrap_or("").to_string(),
                    description: node["description"].as_str().map(|s| s.to_string()),
                    open_pr_count: node["pullRequests"]["totalCount"].as_u64().unwrap_or(0) as u32,
                    is_archived: node["isArchived"].as_bool().unwrap_or(false),
                };
                all_repos.push(repo);
            }

            let page_info = &repos_data["pageInfo"];
            if page_info["hasNextPage"].as_bool().unwrap_or(false) {
                cursor = page_info["endCursor"].as_str().map(|s| s.to_string());
            } else {
                break;
            }
        }

        debug!(org = org, count = all_repos.len(), "Fetched org repos");
        Ok((all_repos, rate_limit))
    }

    pub async fn fetch_user_repos(&self, user: &str) -> Result<(Vec<Repo>, RateLimit)> {
        let mut all_repos = Vec::new();
        let mut cursor: Option<String> = None;
        let mut rate_limit;

        loop {
            let variables = json!({
                "user": user,
                "cursor": cursor,
            });

            let data = self.query(queries::USER_REPOS_QUERY, variables).await?;
            rate_limit = Self::extract_rate_limit(&data);

            let repos_data = &data["data"]["user"]["repositories"];
            let nodes = repos_data["nodes"]
                .as_array()
                .context("Missing repository nodes")?;

            for node in nodes {
                let repo = Repo {
                    name: node["name"].as_str().unwrap_or("").to_string(),
                    owner: node["owner"]["login"].as_str().unwrap_or("").to_string(),
                    url: node["url"].as_str().unwrap_or("").to_string(),
                    description: node["description"].as_str().map(|s| s.to_string()),
                    open_pr_count: node["pullRequests"]["totalCount"].as_u64().unwrap_or(0) as u32,
                    is_archived: node["isArchived"].as_bool().unwrap_or(false),
                };
                all_repos.push(repo);
            }

            let page_info = &repos_data["pageInfo"];
            if page_info["hasNextPage"].as_bool().unwrap_or(false) {
                cursor = page_info["endCursor"].as_str().map(|s| s.to_string());
            } else {
                break;
            }
        }

        debug!(user = user, count = all_repos.len(), "Fetched user repos");
        Ok((all_repos, rate_limit))
    }

    pub async fn search_prs(&self, query_string: &str) -> Result<(Vec<PullRequest>, RateLimit)> {
        let mut all_prs = Vec::new();
        let mut cursor: Option<String> = None;
        let mut rate_limit;

        loop {
            let variables = json!({
                "query": query_string,
                "cursor": cursor,
            });

            let data = self.query(queries::SEARCH_PRS_QUERY, variables).await?;
            rate_limit = Self::extract_rate_limit(&data);

            let search_data = &data["data"]["search"];
            let nodes = search_data["nodes"]
                .as_array()
                .context("Missing search nodes")?;

            for node in nodes {
                if node.get("number").is_none() {
                    continue;
                }
                let pr = parse_search_pr(node);
                all_prs.push(pr);
            }

            let page_info = &search_data["pageInfo"];
            if page_info["hasNextPage"].as_bool().unwrap_or(false) {
                cursor = page_info["endCursor"].as_str().map(|s| s.to_string());
            } else {
                break;
            }
        }

        debug!(
            query = query_string,
            count = all_prs.len(),
            "Search PRs complete"
        );
        Ok((all_prs, rate_limit))
    }

    pub async fn fetch_inbox(&self, viewer_login: &str) -> Result<(Vec<PullRequest>, RateLimit)> {
        let review_query = format!(
            "is:open is:pr review-requested:{} archived:false",
            viewer_login
        );
        let assigned_query = format!("is:open is:pr assignee:{} archived:false", viewer_login);

        let (review_result, assigned_result) = tokio::join!(
            self.search_prs(&review_query),
            self.search_prs(&assigned_query),
        );

        let (review_prs, _) = review_result.context("Failed to fetch review-requested PRs")?;
        let (assigned_prs, rate_limit) = assigned_result.context("Failed to fetch assigned PRs")?;

        // Deduplicate by (repo, number)
        let mut seen = std::collections::HashSet::new();
        let mut inbox = Vec::new();

        for pr in review_prs.into_iter().chain(assigned_prs) {
            let key = (pr.repo_full_name(), pr.number);
            if seen.insert(key) {
                inbox.push(pr);
            }
        }

        // Sort by updated_at descending
        inbox.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));

        debug!(count = inbox.len(), "Fetched inbox");
        Ok((inbox, rate_limit))
    }

    pub async fn fetch_all_open_prs(
        &self,
        orgs: &[String],
        users: &[String],
    ) -> Result<(Vec<PullRequest>, RateLimit)> {
        let mut owner_filters: Vec<String> = Vec::new();
        for o in orgs {
            owner_filters.push(format!("org:{}", o));
        }
        for u in users {
            owner_filters.push(format!("user:{}", u));
        }
        let filter = owner_filters.join(" ");
        let query_string = format!("is:open is:pr archived:false {}", filter);
        self.search_prs(&query_string).await
    }
}

fn parse_search_pr(node: &Value) -> PullRequest {
    let labels = node["labels"]["nodes"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|l| l["name"].as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    PullRequest {
        number: node["number"].as_u64().unwrap_or(0) as u32,
        title: node["title"].as_str().unwrap_or("").to_string(),
        author: node["author"]["login"]
            .as_str()
            .unwrap_or("ghost")
            .to_string(),
        repo_owner: node["repository"]["owner"]["login"]
            .as_str()
            .unwrap_or("")
            .to_string(),
        repo_name: node["repository"]["name"]
            .as_str()
            .unwrap_or("")
            .to_string(),
        url: node["url"].as_str().unwrap_or("").to_string(),
        created_at: node["createdAt"]
            .as_str()
            .and_then(|s| s.parse().ok())
            .unwrap_or_default(),
        updated_at: node["updatedAt"]
            .as_str()
            .and_then(|s| s.parse().ok())
            .unwrap_or_default(),
        is_draft: node["isDraft"].as_bool().unwrap_or(false),
        additions: node["additions"].as_u64().unwrap_or(0) as u32,
        deletions: node["deletions"].as_u64().unwrap_or(0) as u32,
        review_decision: node["reviewDecision"].as_str().map(|s| s.to_string()),
        labels,
    }
}
