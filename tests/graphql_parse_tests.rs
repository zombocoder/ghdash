use ghdash::github::models::{PullRequest, Repo};

#[test]
fn test_repo_full_name() {
    let repo = Repo {
        name: "my-repo".into(),
        owner: "my-org".into(),
        url: "https://github.com/my-org/my-repo".into(),
        description: Some("A repo".into()),
        open_pr_count: 5,
        is_archived: false,
    };
    assert_eq!(repo.full_name(), "my-org/my-repo");
}

#[test]
fn test_pr_repo_full_name() {
    let pr = PullRequest {
        number: 1,
        title: "Test".into(),
        author: "user".into(),
        repo_owner: "org".into(),
        repo_name: "repo".into(),
        url: "https://github.com/org/repo/pull/1".into(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        is_draft: false,
        additions: 0,
        deletions: 0,
        review_decision: None,
        labels: vec![],
    };
    assert_eq!(pr.repo_full_name(), "org/repo");
}

#[test]
fn test_repo_serialization_roundtrip() {
    let repo = Repo {
        name: "test-repo".into(),
        owner: "test-owner".into(),
        url: "https://github.com/test-owner/test-repo".into(),
        description: None,
        open_pr_count: 3,
        is_archived: false,
    };

    let json = serde_json::to_string(&repo).unwrap();
    let deserialized: Repo = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.name, repo.name);
    assert_eq!(deserialized.owner, repo.owner);
    assert_eq!(deserialized.open_pr_count, repo.open_pr_count);
    assert_eq!(deserialized.is_archived, repo.is_archived);
    assert_eq!(deserialized.description, repo.description);
}

#[test]
fn test_pr_serialization_roundtrip() {
    let pr = PullRequest {
        number: 42,
        title: "Add feature".into(),
        author: "alice".into(),
        repo_owner: "org".into(),
        repo_name: "repo".into(),
        url: "https://github.com/org/repo/pull/42".into(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        is_draft: true,
        additions: 100,
        deletions: 50,
        review_decision: Some("APPROVED".into()),
        labels: vec!["bug".into(), "urgent".into()],
    };

    let json = serde_json::to_string(&pr).unwrap();
    let deserialized: PullRequest = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.number, 42);
    assert_eq!(deserialized.title, "Add feature");
    assert_eq!(deserialized.author, "alice");
    assert!(deserialized.is_draft);
    assert_eq!(deserialized.review_decision, Some("APPROVED".into()));
    assert_eq!(deserialized.labels, vec!["bug", "urgent"]);
}

#[test]
fn test_rate_limit_default() {
    let rl = ghdash::github::models::RateLimit::default();
    assert_eq!(rl.remaining, 0);
    assert_eq!(rl.limit, 0);
    assert!(rl.reset_at.is_none());
}

#[test]
fn test_repo_with_description() {
    let repo = Repo {
        name: "repo".into(),
        owner: "owner".into(),
        url: "https://github.com/owner/repo".into(),
        description: Some("A cool project".into()),
        open_pr_count: 0,
        is_archived: true,
    };

    assert_eq!(repo.description, Some("A cool project".into()));
    assert!(repo.is_archived);
}

#[test]
fn test_pr_with_no_review_decision() {
    let pr = PullRequest {
        number: 1,
        title: "WIP".into(),
        author: "dev".into(),
        repo_owner: "org".into(),
        repo_name: "repo".into(),
        url: "url".into(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        is_draft: true,
        additions: 0,
        deletions: 0,
        review_decision: None,
        labels: vec![],
    };

    assert!(pr.review_decision.is_none());
    assert!(pr.labels.is_empty());
}
