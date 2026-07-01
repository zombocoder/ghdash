use ratatui::{
    Frame,
    layout::{Constraint, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, List, ListItem, Paragraph, Row, Table},
};

use crate::app::state::{AppState, ContentView, FocusedPane, NavNode, PrDetailEntry};
use crate::github::models::{CiStatus, PrDetail, PullRequest};
use crate::ui::theme;
use crate::util::time::relative_time;

pub fn render_nav_pane(f: &mut Frame, area: Rect, state: &AppState) {
    let border_style = if state.focused_pane == FocusedPane::Navigation {
        theme::BORDER_FOCUSED
    } else {
        theme::BORDER_UNFOCUSED
    };

    let block = Block::default()
        .title(" Navigation ")
        .borders(Borders::ALL)
        .border_style(border_style);

    let items: Vec<ListItem> = state
        .nav_nodes
        .iter()
        .enumerate()
        .map(|(i, node)| {
            let (text, style) = match node {
                NavNode::MyInbox => {
                    let count = state.inbox.len();
                    let label = if count > 0 {
                        format!("  Inbox ({})", count)
                    } else {
                        "  Inbox".to_string()
                    };
                    (label, theme::NAV_VIRTUAL)
                }
                NavNode::AllPrs => {
                    let count = state.all_open_prs.len();
                    let label = if count > 0 {
                        format!("  All PRs ({})", count)
                    } else {
                        "  All PRs".to_string()
                    };
                    (label, theme::NAV_VIRTUAL)
                }
                NavNode::Org(name) => {
                    let icon = if state.nav_expanded.contains(name) {
                        "▼"
                    } else {
                        "▶"
                    };
                    let repo_count = state
                        .orgs
                        .get(name)
                        .map(|o| o.repos.iter().filter(|r| !r.is_archived).count())
                        .unwrap_or(0);
                    let loading = state.loading_orgs.contains(name);
                    let suffix = if loading {
                        " ...".to_string()
                    } else if repo_count > 0 {
                        format!(" ({})", repo_count)
                    } else {
                        String::new()
                    };
                    (format!("{} {}{}", icon, name, suffix), theme::NAV_ORG)
                }
                NavNode::Repo { name, open_prs, .. } => {
                    let pr_info = if *open_prs > 0 {
                        format!(" [{}]", open_prs)
                    } else {
                        String::new()
                    };
                    (format!("    {}{}", name, pr_info), theme::NAV_REPO)
                }
            };

            let style = if i == state.nav_cursor && state.focused_pane == FocusedPane::Navigation {
                theme::HIGHLIGHT
            } else {
                style
            };

            ListItem::new(Line::from(Span::styled(text, style)))
        })
        .collect();

    let list = List::new(items).block(block);
    f.render_widget(list, area);
}

pub fn render_content_pane(f: &mut Frame, area: Rect, state: &AppState) {
    let border_style = if state.focused_pane == FocusedPane::Content {
        theme::BORDER_FOCUSED
    } else {
        theme::BORDER_UNFOCUSED
    };

    match &state.content_view {
        ContentView::Inbox => {
            render_pr_table(f, area, state, "Inbox", border_style);
        }
        ContentView::AllOpenPrs => {
            render_pr_table(f, area, state, "All Open PRs", border_style);
        }
        ContentView::RepoPrList { owner, name } => {
            let title = format!("{}/{}", owner, name);
            render_pr_table(f, area, state, &title, border_style);
        }
        ContentView::OrgOverview(org) => {
            render_org_overview(f, area, state, org, border_style);
        }
    }
}

/// Compact, colorblind-safe label + color for a PR's merge state.
/// Driven by GitHub's `mergeable` enum; `UNKNOWN`/absent renders as a dim `?`
/// because the search API computes `mergeable` lazily (often `UNKNOWN` at first).
fn merge_state_display(pr: &PullRequest) -> (&'static str, ratatui::style::Style) {
    match pr.mergeable.as_deref() {
        Some("MERGEABLE") => ("✓ ok", theme::MERGE_CLEAN),
        Some("CONFLICTING") => ("✗ cf", theme::MERGE_CONFLICT),
        _ => ("?", theme::DIM),
    }
}

/// Single-glyph CI check indicator for the list column. `statusCheckRollup` is not
/// lazily computed, so this is reliable straight from the search API.
fn ci_display(pr: &PullRequest) -> (&'static str, ratatui::style::Style) {
    match pr.ci_status() {
        CiStatus::Passing => ("✓", theme::MERGE_CLEAN),
        CiStatus::Failing => ("✗", theme::MERGE_CONFLICT),
        CiStatus::Pending => ("…", theme::WARNING),
        CiStatus::None => ("·", theme::DIM),
    }
}

fn render_pr_table(
    f: &mut Frame,
    area: Rect,
    state: &AppState,
    title: &str,
    border_style: ratatui::style::Style,
) {
    let prs = state.current_pr_list();

    let search_suffix = if state.search_active && !state.search_query.is_empty() {
        format!(" [filter: {}]", state.search_query)
    } else {
        String::new()
    };

    let title = format!(" {} ({}) {} ", title, prs.len(), search_suffix);

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(border_style);

    if prs.is_empty() {
        let msg = if state.loading {
            "Loading..."
        } else if state.search_active && !state.search_query.is_empty() {
            "No matching pull requests"
        } else {
            "No open pull requests"
        };
        let para = Paragraph::new(msg).style(theme::DIM).block(block);
        f.render_widget(para, area);
        return;
    }

    let header = Row::new(vec![
        Cell::from("#").style(theme::HEADER),
        Cell::from("State").style(theme::HEADER),
        Cell::from("CI").style(theme::HEADER),
        Cell::from("Title").style(theme::HEADER),
        Cell::from("Author").style(theme::HEADER),
        Cell::from("Repo").style(theme::HEADER),
        Cell::from("Updated").style(theme::HEADER),
    ])
    .height(1);

    let rows: Vec<Row> = prs
        .iter()
        .enumerate()
        .map(|(i, pr)| {
            let style = if i == state.content_cursor && state.focused_pane == FocusedPane::Content {
                theme::HIGHLIGHT
            } else if pr.is_draft {
                theme::DRAFT
            } else {
                ratatui::style::Style::default()
            };

            let review_icon = match pr.review_decision.as_deref() {
                Some("APPROVED") => " +",
                Some("CHANGES_REQUESTED") => " !",
                _ => "",
            };

            let (merge_label, merge_style) = merge_state_display(pr);
            let (ci_label, ci_style) = ci_display(pr);

            Row::new(vec![
                Cell::from(format!("#{}", pr.number)).style(if style == theme::HIGHLIGHT {
                    style
                } else {
                    theme::PR_NUMBER
                }),
                Cell::from(merge_label).style(if style == theme::HIGHLIGHT {
                    style
                } else {
                    merge_style
                }),
                Cell::from(ci_label).style(if style == theme::HIGHLIGHT {
                    style
                } else {
                    ci_style
                }),
                Cell::from(format!(
                    "{}{}{}",
                    if pr.is_draft { "[Draft] " } else { "" },
                    pr.title.as_str(),
                    review_icon,
                ))
                .style(style),
                Cell::from(pr.author.as_str()).style(if style == theme::HIGHLIGHT {
                    style
                } else {
                    theme::PR_AUTHOR
                }),
                Cell::from(pr.repo_name.as_str()).style(style),
                Cell::from(relative_time(&pr.updated_at)).style(if style == theme::HIGHLIGHT {
                    style
                } else {
                    theme::DIM
                }),
            ])
            .height(1)
        })
        .collect();

    let widths = [
        Constraint::Length(7),
        Constraint::Length(5),
        Constraint::Length(3),
        Constraint::Min(20),
        Constraint::Length(16),
        Constraint::Length(24),
        Constraint::Length(10),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(block)
        .row_highlight_style(theme::HIGHLIGHT);

    f.render_widget(table, area);
}

fn render_org_overview(
    f: &mut Frame,
    area: Rect,
    state: &AppState,
    org: &str,
    border_style: ratatui::style::Style,
) {
    let block = Block::default()
        .title(format!(" {} ", org))
        .borders(Borders::ALL)
        .border_style(border_style);

    let org_data = state.orgs.get(org);

    let mut lines = vec![
        Line::from(Span::styled(
            format!("Organization: {}", org),
            theme::HEADER,
        )),
        Line::from(""),
    ];

    if let Some(data) = org_data {
        let active_repos = data.repos.iter().filter(|r| !r.is_archived).count();
        let total_prs: u32 = data
            .repos
            .iter()
            .filter(|r| !r.is_archived)
            .map(|r| r.open_pr_count)
            .sum();

        lines.push(Line::from(format!("Repositories: {}", active_repos)));
        lines.push(Line::from(format!("Open PRs: {}", total_prs)));
        lines.push(Line::from(""));

        // Top repos by PR count
        let mut repos_with_prs: Vec<_> = data
            .repos
            .iter()
            .filter(|r| !r.is_archived && r.open_pr_count > 0)
            .collect();
        repos_with_prs.sort_by(|a, b| b.open_pr_count.cmp(&a.open_pr_count));

        if !repos_with_prs.is_empty() {
            lines.push(Line::from(Span::styled(
                "Top repos by open PRs:",
                theme::HEADER,
            )));
            for repo in repos_with_prs.iter().take(10) {
                lines.push(Line::from(format!(
                    "  {} — {} PRs",
                    repo.name, repo.open_pr_count
                )));
            }
        }
    } else {
        lines.push(Line::from(Span::styled("Loading...", theme::DIM)));
    }

    let para = Paragraph::new(lines).block(block);
    f.render_widget(para, area);
}

pub fn render_status_bar(f: &mut Frame, area: Rect, state: &AppState) {
    let key_hints = if state.search_active {
        "Esc: close search | Enter: filter"
    } else {
        "j/k: nav | Tab: switch pane | Enter: select | d: detail | /: search | r: refresh | o: open | q: quit"
    };

    let status = if state.loading {
        "Loading...".to_string()
    } else if let Some(ref err) = state.error_message {
        format!("Error: {} (Esc to dismiss)", err)
    } else {
        String::new()
    };

    let rate_info = format!(
        "API: {}/{}",
        state.rate_limit.remaining, state.rate_limit.limit
    );

    let refresh_info = state
        .last_refresh
        .as_ref()
        .map(|t| format!(" | {}", relative_time(t)))
        .unwrap_or_default();

    let right_text = format!("{}{}", rate_info, refresh_info);

    // Calculate available space
    let total_width = area.width as usize;
    let left_len = key_hints.len();
    let right_len = right_text.len();

    let center_start = left_len + 1;
    let center_width = total_width.saturating_sub(left_len + right_len + 2);
    let status_truncated = if status.len() > center_width {
        format!("{}...", &status[..center_width.saturating_sub(3)])
    } else {
        status
    };

    let padding = center_width.saturating_sub(status_truncated.len());

    let line = Line::from(vec![
        Span::styled(key_hints, theme::STATUS_BAR),
        Span::styled(" ".repeat(center_start.min(1)), theme::STATUS_BAR),
        Span::styled(
            status_truncated,
            if state.error_message.is_some() {
                theme::ERROR.bg(ratatui::style::Color::DarkGray)
            } else {
                theme::STATUS_BAR
            },
        ),
        Span::styled(" ".repeat(padding), theme::STATUS_BAR),
        Span::styled(right_text, theme::STATUS_BAR),
    ]);

    let bar = Paragraph::new(line).style(theme::STATUS_BAR);
    f.render_widget(bar, area);
}

pub fn render_search_overlay(f: &mut Frame, state: &AppState) {
    if !state.search_active {
        return;
    }

    let full = f.area();
    let search_area = Rect {
        x: 0,
        y: full.height.saturating_sub(2),
        width: full.width,
        height: 1,
    };

    let text = format!("/{}", state.search_query);
    let para = Paragraph::new(Span::styled(text, theme::HEADER)).style(theme::STATUS_BAR);
    f.render_widget(Clear, search_area);
    f.render_widget(para, search_area);
}

pub fn render_error_modal(f: &mut Frame, area: Rect, state: &AppState) {
    let Some(ref msg) = state.error_message else {
        return;
    };

    let modal_width = (area.width / 2).max(40).min(area.width - 4);
    let modal_height = 5u16;
    let x = (area.width.saturating_sub(modal_width)) / 2;
    let y = (area.height.saturating_sub(modal_height)) / 2;

    let modal_area = Rect {
        x,
        y,
        width: modal_width,
        height: modal_height,
    };

    f.render_widget(Clear, modal_area);

    let block = Block::default()
        .title(" Error ")
        .borders(Borders::ALL)
        .border_style(theme::ERROR);

    let text = vec![
        Line::from(Span::styled(msg.as_str(), theme::ERROR)),
        Line::from(""),
        Line::from(Span::styled("Press Esc to dismiss", theme::DIM)),
    ];

    let para = Paragraph::new(text).block(block);
    f.render_widget(para, modal_area);
}

/// Human label + style for a PR's `mergeable` value, used in the detail pane.
fn mergeable_label(mergeable: Option<&str>) -> (String, ratatui::style::Style) {
    match mergeable {
        Some("MERGEABLE") => ("✓ mergeable".to_string(), theme::MERGE_CLEAN),
        Some("CONFLICTING") => ("✗ conflicting".to_string(), theme::MERGE_CONFLICT),
        _ => ("? unknown".to_string(), theme::DIM),
    }
}

/// Human label + style for a `statusCheckRollup.state` value.
fn checks_label(checks: Option<&str>) -> (String, ratatui::style::Style) {
    match checks {
        Some("SUCCESS") => ("✓ passing".to_string(), theme::MERGE_CLEAN),
        Some("FAILURE") | Some("ERROR") => ("✗ failing".to_string(), theme::MERGE_CONFLICT),
        Some("PENDING") | Some("EXPECTED") => ("… pending".to_string(), theme::WARNING),
        Some(other) => (other.to_string(), theme::DIM),
        None => ("— no checks".to_string(), theme::DIM),
    }
}

fn detail_body_lines(detail: &PrDetail, max_commits: usize) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    let (merge_text, merge_style) = mergeable_label(detail.mergeable.as_deref());
    let (checks_text, checks_style) = checks_label(detail.checks_status.as_deref());
    let state_suffix = detail
        .merge_state_status
        .as_deref()
        .map(|s| format!(" ({})", s))
        .unwrap_or_default();

    lines.push(Line::from(vec![
        Span::styled("Merge: ", theme::HEADER),
        Span::styled(format!("{}{}", merge_text, state_suffix), merge_style),
        Span::raw("    "),
        Span::styled("CI: ", theme::HEADER),
        Span::styled(checks_text, checks_style),
    ]));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled("Recent commits:", theme::HEADER)));

    if detail.commits.is_empty() {
        lines.push(Line::from(Span::styled("  (none)", theme::DIM)));
    } else {
        // GitHub returns oldest-first; show newest first.
        for commit in detail.commits.iter().rev().take(max_commits) {
            lines.push(Line::from(vec![
                Span::styled(format!("  {} ", commit.short_oid()), theme::PR_NUMBER),
                Span::raw(commit.headline.clone()),
                Span::styled(
                    format!("  ({})", relative_time(&commit.committed_date)),
                    theme::DIM,
                ),
            ]));
        }
    }

    lines
}

/// Detail-on-highlight overlay: fresh merge state, CI rollup, and recent commits
/// for the currently highlighted PR.
pub fn render_pr_detail_overlay(f: &mut Frame, state: &AppState) {
    if !state.detail_open {
        return;
    }
    let Some(pr) = state.selected_pr() else {
        return;
    };

    let area = f.area();
    let modal_width = (area.width * 3 / 4).clamp(40, area.width.saturating_sub(4));
    let modal_height = 14u16.min(area.height.saturating_sub(2));
    let x = (area.width.saturating_sub(modal_width)) / 2;
    let y = (area.height.saturating_sub(modal_height)) / 2;
    let modal_area = Rect {
        x,
        y,
        width: modal_width,
        height: modal_height,
    };

    let title = format!(" PR #{} — {} ", pr.number, pr.title);
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(theme::BORDER_FOCUSED);

    let body_capacity = modal_area.height.saturating_sub(4) as usize;
    let mut lines: Vec<Line> = match state.pr_details.get(&pr.url) {
        Some(PrDetailEntry::Loaded(detail)) => {
            detail_body_lines(detail, body_capacity.saturating_sub(3))
        }
        Some(PrDetailEntry::Failed(msg)) => {
            vec![Line::from(Span::styled(msg.clone(), theme::ERROR))]
        }
        Some(PrDetailEntry::Loading) | None => {
            vec![Line::from(Span::styled("Loading detail…", theme::DIM))]
        }
    };
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Press d or Esc to close",
        theme::DIM,
    )));

    f.render_widget(Clear, modal_area);
    let para = Paragraph::new(lines).block(block);
    f.render_widget(para, modal_area);
}
