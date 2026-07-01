use ghdash::github::models::{CiStatus, PullRequest, Repo};

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
        mergeable: None,
        merge_state_status: None,
        checks_status: None,
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
        mergeable: Some("MERGEABLE".into()),
        merge_state_status: Some("CLEAN".into()),
        checks_status: Some("SUCCESS".into()),
        labels: vec!["bug".into(), "urgent".into()],
    };

    let json = serde_json::to_string(&pr).unwrap();
    let deserialized: PullRequest = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.number, 42);
    assert_eq!(deserialized.title, "Add feature");
    assert_eq!(deserialized.author, "alice");
    assert!(deserialized.is_draft);
    assert_eq!(deserialized.review_decision, Some("APPROVED".into()));
    assert_eq!(deserialized.mergeable, Some("MERGEABLE".into()));
    assert_eq!(deserialized.merge_state_status, Some("CLEAN".into()));
    assert_eq!(deserialized.checks_status, Some("SUCCESS".into()));
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
        mergeable: None,
        merge_state_status: None,
        checks_status: None,
        labels: vec![],
    };

    assert!(pr.review_decision.is_none());
    assert!(pr.labels.is_empty());
}

#[test]
fn test_pr_deserializes_without_merge_fields() {
    // Older cache entries won't have mergeable / mergeStateStatus. #[serde(default)]
    // must let them deserialize to None rather than failing (which would drop the
    // whole cache entry). Guards the CI-3 cache-schema-evolution concern.
    let legacy = r#"{
        "number": 7,
        "title": "Legacy cached PR",
        "author": "bob",
        "repo_owner": "org",
        "repo_name": "repo",
        "url": "https://github.com/org/repo/pull/7",
        "created_at": "2026-01-01T00:00:00Z",
        "updated_at": "2026-01-02T00:00:00Z",
        "is_draft": false,
        "additions": 3,
        "deletions": 1,
        "review_decision": null,
        "labels": []
    }"#;

    let pr: PullRequest = serde_json::from_str(legacy).expect("legacy cache must deserialize");
    assert_eq!(pr.number, 7);
    assert!(pr.mergeable.is_none());
    assert!(pr.merge_state_status.is_none());
    assert!(pr.checks_status.is_none());
}

#[test]
fn test_pr_conflicting_merge_state_roundtrip() {
    let pr = PullRequest {
        number: 9,
        title: "Conflicting PR".into(),
        author: "carol".into(),
        repo_owner: "org".into(),
        repo_name: "repo".into(),
        url: "https://github.com/org/repo/pull/9".into(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        is_draft: false,
        additions: 1,
        deletions: 1,
        review_decision: None,
        mergeable: Some("CONFLICTING".into()),
        merge_state_status: Some("DIRTY".into()),
        checks_status: Some("FAILURE".into()),
        labels: vec![],
    };

    let json = serde_json::to_string(&pr).unwrap();
    let deserialized: PullRequest = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.mergeable, Some("CONFLICTING".into()));
    assert_eq!(deserialized.merge_state_status, Some("DIRTY".into()));
    assert_eq!(deserialized.checks_status, Some("FAILURE".into()));
}

// --- CI status classification (task 9h3o) ---

fn pr_with_checks(state: Option<&str>) -> PullRequest {
    PullRequest {
        number: 1,
        title: "t".into(),
        author: "a".into(),
        repo_owner: "o".into(),
        repo_name: "r".into(),
        url: "u".into(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        is_draft: false,
        additions: 0,
        deletions: 0,
        review_decision: None,
        mergeable: None,
        merge_state_status: None,
        checks_status: state.map(|s| s.to_string()),
        labels: vec![],
    }
}

#[test]
fn test_ci_status_success() {
    assert_eq!(
        pr_with_checks(Some("SUCCESS")).ci_status(),
        CiStatus::Passing
    );
}

#[test]
fn test_ci_status_failure() {
    assert_eq!(
        pr_with_checks(Some("FAILURE")).ci_status(),
        CiStatus::Failing
    );
    assert_eq!(pr_with_checks(Some("ERROR")).ci_status(), CiStatus::Failing);
}

#[test]
fn test_ci_status_pending() {
    assert_eq!(
        pr_with_checks(Some("PENDING")).ci_status(),
        CiStatus::Pending
    );
    assert_eq!(
        pr_with_checks(Some("EXPECTED")).ci_status(),
        CiStatus::Pending
    );
}

#[test]
fn test_ci_status_none() {
    assert_eq!(pr_with_checks(None).ci_status(), CiStatus::None);
    // Unknown/other states fall back to None rather than misreporting.
    assert_eq!(pr_with_checks(Some("WEIRD")).ci_status(), CiStatus::None);
}
