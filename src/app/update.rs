use crate::app::actions::{Action, DataPayload, SideEffect};
use crate::app::state::{
    AppState, ContentView, DiffEntry, FocusedPane, NavNode, OrgData, Overlay, PrDetailEntry,
};

pub fn update(state: &mut AppState, action: Action) -> Vec<SideEffect> {
    match action {
        Action::Quit => {
            state.should_quit = true;
            vec![]
        }
        Action::MoveUp => {
            // While the diff overlay is open, j/k scroll the diff instead of moving
            // the underlying selection.
            if state.overlay == Overlay::Diff {
                state.diff_scroll = state.diff_scroll.saturating_sub(1);
                return vec![];
            }
            if state.overlay == Overlay::GitLog {
                return vec![];
            }
            match state.focused_pane {
                FocusedPane::Navigation => {
                    if state.nav_cursor > 0 {
                        state.nav_cursor -= 1;
                    }
                }
                FocusedPane::Content => {
                    if state.content_cursor > 0 {
                        state.content_cursor -= 1;
                    }
                }
            }
            vec![]
        }
        Action::MoveDown => {
            if state.overlay == Overlay::Diff {
                state.diff_scroll = state.diff_scroll.saturating_add(1);
                return vec![];
            }
            if state.overlay == Overlay::GitLog {
                return vec![];
            }
            match state.focused_pane {
                FocusedPane::Navigation => {
                    if state.nav_cursor + 1 < state.nav_nodes.len() {
                        state.nav_cursor += 1;
                    }
                }
                FocusedPane::Content => {
                    let max = state.current_pr_list().len().saturating_sub(1);
                    if state.content_cursor < max {
                        state.content_cursor += 1;
                    }
                }
            }
            vec![]
        }
        Action::Select => {
            if state.focused_pane == FocusedPane::Navigation {
                if let Some(node) = state.nav_nodes.get(state.nav_cursor).cloned() {
                    match node {
                        NavNode::Org(ref org) => {
                            if state.nav_expanded.contains(org) {
                                state.nav_expanded.remove(org);
                            } else {
                                state.nav_expanded.insert(org.clone());
                            }
                            state.content_view = ContentView::OrgOverview(org.clone());
                            state.content_cursor = 0;
                            state.rebuild_nav_tree();
                        }
                        NavNode::Repo { owner, name, .. } => {
                            state.content_view = ContentView::RepoPrList {
                                owner: owner.clone(),
                                name: name.clone(),
                            };
                            state.content_cursor = 0;
                        }
                        NavNode::AllPrs => {
                            state.content_view = ContentView::AllOpenPrs;
                            state.content_cursor = 0;
                        }
                        NavNode::MyInbox => {
                            state.content_view = ContentView::Inbox;
                            state.content_cursor = 0;
                        }
                    }
                }
            } else {
                // In content pane, Enter opens PR in browser
                if let Some(url) = state.selected_pr_url() {
                    return vec![SideEffect::OpenUrl(url)];
                }
            }
            vec![]
        }
        Action::Back => {
            if state.help_open {
                state.help_open = false;
            } else if state.search_active {
                state.search_active = false;
                state.search_query.clear();
            } else if state.error_message.is_some() {
                state.error_message = None;
            } else if state.overlay != Overlay::None {
                state.overlay = Overlay::None;
            } else if state.focused_pane == FocusedPane::Content {
                state.focused_pane = FocusedPane::Navigation;
            }
            vec![]
        }
        Action::SwitchPane => {
            state.focused_pane = match state.focused_pane {
                FocusedPane::Navigation => FocusedPane::Content,
                FocusedPane::Content => FocusedPane::Navigation,
            };
            vec![]
        }
        Action::Refresh => {
            state.loading = true;
            state.error_message = None;
            // Drop cached PR details / diffs so they are re-fetched fresh.
            state.pr_details.clear();
            state.pr_diffs.clear();
            vec![SideEffect::RefreshAll]
        }
        Action::OpenInBrowser => {
            let url = match state.focused_pane {
                FocusedPane::Content => state.selected_pr_url(),
                FocusedPane::Navigation => state.selected_nav_url(),
            };
            if let Some(url) = url {
                vec![SideEffect::OpenUrl(url)]
            } else {
                vec![]
            }
        }
        Action::ToggleSearch => {
            if state.search_active {
                state.search_active = false;
                state.search_query.clear();
            } else {
                state.search_active = true;
                state.search_query.clear();
            }
            vec![]
        }
        Action::ToggleGitLog => {
            // Only meaningful in the content pane; the event loop fetches the PR's
            // commits (debounced) while the overlay is open.
            state.overlay = if state.overlay == Overlay::GitLog {
                Overlay::None
            } else {
                Overlay::GitLog
            };
            vec![]
        }
        Action::ToggleDiff => {
            state.diff_scroll = 0;
            state.overlay = if state.overlay == Overlay::Diff {
                Overlay::None
            } else {
                Overlay::Diff
            };
            vec![]
        }
        Action::CloseOverlay => {
            state.overlay = Overlay::None;
            vec![]
        }
        Action::ToggleHelp => {
            state.help_open = !state.help_open;
            vec![]
        }
        Action::CycleMergeFilter => {
            state.merge_filter = state.merge_filter.next();
            // Row set changed; reset the cursor so it stays in range.
            state.content_cursor = 0;
            vec![]
        }
        Action::SearchInput(ch) => {
            if state.search_active {
                state.search_query.push(ch);
                state.content_cursor = 0;
            }
            vec![]
        }
        Action::SearchBackspace => {
            if state.search_active {
                state.search_query.pop();
                state.content_cursor = 0;
            }
            vec![]
        }
        Action::SearchClear => {
            state.search_query.clear();
            state.content_cursor = 0;
            vec![]
        }
        Action::DataLoaded(payload) => {
            match payload {
                DataPayload::OrgRepos {
                    org,
                    repos,
                    rate_limit,
                } => {
                    state.loading_orgs.remove(&org);
                    state.rate_limit = rate_limit;
                    state.orgs.insert(org.clone(), OrgData { name: org, repos });
                    state.rebuild_nav_tree();
                }
                DataPayload::InboxPrs { prs, rate_limit } => {
                    state.rate_limit = rate_limit;
                    state.inbox = prs;
                }
                DataPayload::AllOpenPrs { prs, rate_limit } => {
                    state.rate_limit = rate_limit;
                    state.all_open_prs = prs;
                }
                DataPayload::PrDetailLoaded {
                    key,
                    detail,
                    rate_limit,
                } => {
                    state.rate_limit = rate_limit;
                    // Upgrade the list column to the freshly computed merge state.
                    state.apply_fresh_merge_state(
                        &key,
                        detail.mergeable.clone(),
                        detail.merge_state_status.clone(),
                    );
                    state.pr_details.insert(key, PrDetailEntry::Loaded(detail));
                    // A detail fetch is not part of the initial load; leave the
                    // global loading flag untouched by returning early.
                    return vec![];
                }
                DataPayload::PrDetailFailed { key, msg } => {
                    state.pr_details.insert(key, PrDetailEntry::Failed(msg));
                    return vec![];
                }
                DataPayload::PrDiffLoaded { key, diff } => {
                    state.pr_diffs.insert(key, DiffEntry::Loaded(diff));
                    return vec![];
                }
                DataPayload::PrDiffFailed { key, msg } => {
                    state.pr_diffs.insert(key, DiffEntry::Failed(msg));
                    return vec![];
                }
            }

            // Check if all loading complete
            if state.loading_orgs.is_empty() {
                state.loading = false;
                state.last_refresh = Some(chrono::Utc::now());
            }

            vec![]
        }
        Action::LoadError(msg) => {
            state.loading = false;
            state.loading_orgs.clear();
            state.error_message = Some(msg);
            vec![]
        }
        Action::DismissError => {
            state.error_message = None;
            vec![]
        }
        Action::Tick => vec![],
    }
}
