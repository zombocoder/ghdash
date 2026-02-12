pub const VIEWER_QUERY: &str = r#"
query {
  viewer {
    login
  }
  rateLimit {
    remaining
    limit
    resetAt
  }
}
"#;

pub const ORG_REPOS_QUERY: &str = r#"
query($org: String!, $cursor: String) {
  organization(login: $org) {
    repositories(first: 100, after: $cursor, orderBy: {field: UPDATED_AT, direction: DESC}) {
      pageInfo {
        hasNextPage
        endCursor
      }
      nodes {
        name
        owner { login }
        url
        description
        isArchived
        pullRequests(states: OPEN) {
          totalCount
        }
      }
    }
  }
  rateLimit {
    remaining
    limit
    resetAt
  }
}
"#;

pub const USER_REPOS_QUERY: &str = r#"
query($user: String!, $cursor: String) {
  user(login: $user) {
    repositories(first: 100, after: $cursor, orderBy: {field: UPDATED_AT, direction: DESC}, ownerAffiliations: OWNER) {
      pageInfo {
        hasNextPage
        endCursor
      }
      nodes {
        name
        owner { login }
        url
        description
        isArchived
        pullRequests(states: OPEN) {
          totalCount
        }
      }
    }
  }
  rateLimit {
    remaining
    limit
    resetAt
  }
}
"#;

#[allow(dead_code)]
pub const REPO_PRS_QUERY: &str = r#"
query($owner: String!, $name: String!, $cursor: String) {
  repository(owner: $owner, name: $name) {
    pullRequests(first: 100, after: $cursor, states: OPEN, orderBy: {field: UPDATED_AT, direction: DESC}) {
      pageInfo {
        hasNextPage
        endCursor
      }
      nodes {
        number
        title
        author { login }
        url
        createdAt
        updatedAt
        isDraft
        additions
        deletions
        reviewDecision
        labels(first: 10) {
          nodes { name }
        }
      }
    }
  }
  rateLimit {
    remaining
    limit
    resetAt
  }
}
"#;

pub const SEARCH_PRS_QUERY: &str = r#"
query($query: String!, $cursor: String) {
  search(query: $query, type: ISSUE, first: 100, after: $cursor) {
    pageInfo {
      hasNextPage
      endCursor
    }
    nodes {
      ... on PullRequest {
        number
        title
        author { login }
        repository {
          name
          owner { login }
        }
        url
        createdAt
        updatedAt
        isDraft
        additions
        deletions
        reviewDecision
        labels(first: 10) {
          nodes { name }
        }
      }
    }
  }
  rateLimit {
    remaining
    limit
    resetAt
  }
}
"#;
