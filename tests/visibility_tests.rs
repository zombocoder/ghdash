use ghdash::app::event_loop::filter_repos;
use ghdash::github::graphql::{build_all_open_prs_query, build_inbox_queries};
use ghdash::github::models::{Repo, RepoVisibility};

fn repo(name: &str, is_private: bool) -> Repo {
    Repo {
        name: name.into(),
        owner: "acme".into(),
        url: format!("https://github.com/acme/{}", name),
        description: None,
        open_pr_count: 0,
        is_archived: false,
        is_private,
    }
}

#[test]
fn default_visibility_is_all() {
    assert_eq!(RepoVisibility::default(), RepoVisibility::All);
}

#[test]
fn all_allows_public_and_private() {
    assert!(RepoVisibility::All.allows(false));
    assert!(RepoVisibility::All.allows(true));
}

#[test]
fn public_allows_only_public() {
    assert!(RepoVisibility::Public.allows(false));
    assert!(!RepoVisibility::Public.allows(true));
}

#[test]
fn private_allows_only_private() {
    assert!(!RepoVisibility::Private.allows(false));
    assert!(RepoVisibility::Private.allows(true));
}

#[test]
fn all_has_no_search_qualifier() {
    assert_eq!(RepoVisibility::All.search_qualifier(), None);
}

#[test]
fn public_search_qualifier_is_is_public() {
    assert_eq!(RepoVisibility::Public.search_qualifier(), Some("is:public"));
}

#[test]
fn private_search_qualifier_is_is_private() {
    assert_eq!(
        RepoVisibility::Private.search_qualifier(),
        Some("is:private")
    );
}

#[test]
fn all_has_no_label_but_restricted_modes_do() {
    assert_eq!(RepoVisibility::All.label(), None);
    assert_eq!(RepoVisibility::Public.label(), Some("public only"));
    assert_eq!(RepoVisibility::Private.label(), Some("private only"));
}

#[test]
fn cache_suffix_distinguishes_modes() {
    assert_eq!(RepoVisibility::All.cache_suffix(), "all");
    assert_eq!(RepoVisibility::Public.cache_suffix(), "public");
    assert_eq!(RepoVisibility::Private.cache_suffix(), "private");
}

#[test]
fn all_open_prs_query_has_no_qualifier_when_visibility_is_all() {
    let q = build_all_open_prs_query(&["acme".into()], &["alice".into()], RepoVisibility::All);
    assert_eq!(q, "is:open is:pr archived:false org:acme user:alice");
}

#[test]
fn all_open_prs_query_restricts_to_public_repos() {
    let q = build_all_open_prs_query(&["acme".into()], &[], RepoVisibility::Public);
    assert_eq!(q, "is:open is:pr archived:false org:acme is:public");
}

#[test]
fn all_open_prs_query_restricts_to_private_repos() {
    let q = build_all_open_prs_query(&[], &["alice".into()], RepoVisibility::Private);
    assert_eq!(q, "is:open is:pr archived:false user:alice is:private");
}

#[test]
fn inbox_queries_have_no_qualifier_when_visibility_is_all() {
    let (review, assigned) = build_inbox_queries("alice", RepoVisibility::All);
    assert_eq!(
        review,
        "is:open is:pr review-requested:alice archived:false"
    );
    assert_eq!(assigned, "is:open is:pr assignee:alice archived:false");
}

#[test]
fn both_inbox_queries_restrict_to_public_repos() {
    let (review, assigned) = build_inbox_queries("alice", RepoVisibility::Public);
    assert_eq!(
        review,
        "is:open is:pr review-requested:alice archived:false is:public"
    );
    assert_eq!(
        assigned,
        "is:open is:pr assignee:alice archived:false is:public"
    );
}

#[test]
fn filter_repos_keeps_both_visibilities_by_default() {
    let repos = vec![repo("pub", false), repo("priv", true)];
    let kept = filter_repos(repos, &[], &[], RepoVisibility::All);
    assert_eq!(kept.len(), 2);
}

#[test]
fn filter_repos_drops_private_repos_in_public_mode() {
    let repos = vec![repo("pub", false), repo("priv", true)];
    let kept = filter_repos(repos, &[], &[], RepoVisibility::Public);
    let names: Vec<_> = kept.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, vec!["pub"]);
}

#[test]
fn filter_repos_drops_public_repos_in_private_mode() {
    let repos = vec![repo("pub", false), repo("priv", true)];
    let kept = filter_repos(repos, &[], &[], RepoVisibility::Private);
    let names: Vec<_> = kept.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, vec!["priv"]);
}

#[test]
fn filter_repos_applies_visibility_on_top_of_include_and_exclude() {
    let repos = vec![
        repo("important-pub", false),
        repo("important-priv", true),
        repo("other-pub", false),
        repo("important-legacy", false),
    ];
    let kept = filter_repos(
        repos,
        &["important-*".into()],
        &["*-legacy".into()],
        RepoVisibility::Public,
    );
    let names: Vec<_> = kept.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, vec!["important-pub"]);
}
