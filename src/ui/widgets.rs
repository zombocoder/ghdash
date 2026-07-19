use ratatui::{
    Frame,
    layout::{Constraint, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, List, ListItem, Paragraph, Row, Table},
};

use crate::app::state::{
    AppState, ContentView, DiffEntry, FocusedPane, NavNode, Overlay, PrDetailEntry,
};
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
    let merge_suffix = match state.merge_filter.label() {
        Some(l) => format!(" [state: {}]", l),
        None => String::new(),
    };

    let title = format!(
        " {} ({}){}{} ",
        title,
        prs.len(),
        merge_suffix,
        search_suffix
    );

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
        repos_with_prs.sort_by_key(|r| std::cmp::Reverse(r.open_pr_count));

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
    // Active-profile chip (design ④): always visible, marks the current context.
    let chip = format!("⦗● {}⦆ ", state.active_profile);

    let key_hints = if state.search_active {
        "Esc: close search | Enter: filter"
    } else {
        "j/k: nav | Enter: select | l: log | d: diff | f: filter | /: search | p: profiles | r: refresh | o: open | ?: help | q: quit"
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

    // Calculate available space (chip + hints share the left region).
    let total_width = area.width as usize;
    let left_len = chip.chars().count() + key_hints.chars().count();
    let right_len = right_text.chars().count();

    let center_start = left_len + 1;
    let center_width = total_width.saturating_sub(left_len + right_len + 2);
    let status_truncated = if status.len() > center_width {
        format!("{}...", &status[..center_width.saturating_sub(3)])
    } else {
        status
    };

    let padding = center_width.saturating_sub(status_truncated.len());

    let line = Line::from(vec![
        Span::styled(chip, theme::STATUS_CHIP),
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

/// Profile picker overlay (design ①): a centered, type-to-filter modal listing
/// the configured profiles. Navigate with the arrow keys, `Enter` switches,
/// `Esc` cancels. Modeled on the search overlay's filtered-list pattern.
pub fn render_profile_picker(f: &mut Frame, state: &AppState) {
    if !state.profile_picker_active {
        return;
    }

    let modal_area = overlay_area(f, 60, 50);
    let block = Block::default()
        .title(" Switch Profile ")
        .title_bottom(Line::from(Span::styled(
            " type: filter · ↑/↓: move · Enter: switch · Esc: cancel ",
            theme::DIM,
        )))
        .borders(Borders::ALL)
        .border_style(theme::BORDER_FOCUSED);

    let filtered = state.filtered_profiles();

    let mut lines: Vec<Line> = Vec::new();
    // Filter input line.
    lines.push(Line::from(vec![
        Span::styled("> ", theme::HEADER),
        Span::styled(state.profile_picker_query.clone(), theme::HEADER),
    ]));

    if filtered.is_empty() {
        lines.push(Line::from(Span::styled(
            "  (no matching profiles)",
            theme::DIM,
        )));
    } else {
        for (i, p) in filtered.iter().enumerate() {
            let marker = if p.name == state.active_profile {
                "●"
            } else {
                " "
            };
            let scope = if p.scope_count == 1 {
                "1 scope".to_string()
            } else {
                format!("{} scopes", p.scope_count)
            };
            let text = format!("{} {:<16} {} · {}", marker, p.name, scope, p.host);
            let style = if i == state.profile_picker_cursor {
                theme::HIGHLIGHT
            } else if p.name == state.active_profile {
                theme::MERGE_CLEAN
            } else {
                ratatui::style::Style::default()
            };
            lines.push(Line::from(Span::styled(text, style)));
        }
    }

    f.render_widget(Clear, modal_area);
    let para = Paragraph::new(lines).block(block);
    f.render_widget(para, modal_area);
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

/// Render the active PR overlay (git log or diff) for the highlighted PR, if any.
pub fn render_pr_overlay(f: &mut Frame, state: &AppState) {
    match state.overlay {
        Overlay::None => {}
        Overlay::GitLog => render_git_log_overlay(f, state),
        Overlay::Diff => render_diff_overlay(f, state),
    }
}

/// Centered modal rect covering the given fraction of the screen.
fn overlay_area(f: &Frame, width_pct: u16, height_pct: u16) -> Rect {
    let area = f.area();
    let modal_width = (area.width * width_pct / 100).clamp(40, area.width.saturating_sub(2));
    let modal_height = (area.height * height_pct / 100).clamp(6, area.height.saturating_sub(2));
    Rect {
        x: (area.width.saturating_sub(modal_width)) / 2,
        y: (area.height.saturating_sub(modal_height)) / 2,
        width: modal_width,
        height: modal_height,
    }
}

/// Git-log overlay: recent commits (plus fresh merge/CI) for the highlighted PR.
fn render_git_log_overlay(f: &mut Frame, state: &AppState) {
    let Some(pr) = state.selected_pr() else {
        return;
    };

    let modal_area = overlay_area(f, 75, 60);
    let title = format!(" Git log — PR #{} — {} ", pr.number, pr.title);
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
            vec![Line::from(Span::styled("Loading commits…", theme::DIM))]
        }
    };
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "l/Esc: close · d: diff",
        theme::DIM,
    )));

    f.render_widget(Clear, modal_area);
    let para = Paragraph::new(lines).block(block);
    f.render_widget(para, modal_area);
}

/// Style a single unified-diff line by its leading marker.
fn diff_line_style(line: &str) -> ratatui::style::Style {
    use ratatui::style::{Color, Style};
    if line.starts_with("diff --git") || line.starts_with("index ") {
        theme::NAV_ORG
    } else if line.starts_with("@@") {
        theme::PR_NUMBER
    } else if line.starts_with("+++") || line.starts_with("---") {
        theme::HEADER
    } else if line.starts_with('+') {
        Style::new().fg(Color::Green)
    } else if line.starts_with('-') {
        Style::new().fg(Color::Red)
    } else {
        Style::default()
    }
}

/// Diff overlay: full unified diff for the highlighted PR, scrollable with j/k.
fn render_diff_overlay(f: &mut Frame, state: &AppState) {
    let Some(pr) = state.selected_pr() else {
        return;
    };

    let modal_area = overlay_area(f, 90, 90);
    let title = format!(" Diff — PR #{} — {} ", pr.number, pr.title);

    let body_height = modal_area.height.saturating_sub(3) as usize;

    let (lines, scrollable): (Vec<Line>, bool) = match state.pr_diffs.get(&pr.url) {
        Some(DiffEntry::Loaded(diff)) if !diff.is_empty() => (
            diff.lines()
                .map(|l| Line::from(Span::styled(l.to_string(), diff_line_style(l))))
                .collect(),
            true,
        ),
        Some(DiffEntry::Loaded(_)) => (
            vec![Line::from(Span::styled("(empty diff)", theme::DIM))],
            false,
        ),
        Some(DiffEntry::Failed(msg)) => (
            vec![Line::from(Span::styled(msg.clone(), theme::ERROR))],
            false,
        ),
        Some(DiffEntry::Loading) | None => (
            vec![Line::from(Span::styled("Loading diff…", theme::DIM))],
            false,
        ),
    };

    // Clamp scroll so we can't page past the end.
    let max_scroll = lines.len().saturating_sub(body_height) as u16;
    let scroll = if scrollable {
        state.diff_scroll.min(max_scroll)
    } else {
        0
    };

    let hint = if scrollable {
        format!(
            " j/k: scroll · d/Esc: close · l: log ({}/{}) ",
            scroll, max_scroll
        )
    } else {
        " d/Esc: close · l: log ".to_string()
    };
    let block = Block::default()
        .title(title)
        .title_bottom(Line::from(Span::styled(hint, theme::DIM)))
        .borders(Borders::ALL)
        .border_style(theme::BORDER_FOCUSED);

    f.render_widget(Clear, modal_area);
    let para = Paragraph::new(lines).block(block).scroll((scroll, 0));
    f.render_widget(para, modal_area);
}

/// Help overlay: keybindings plus the State/CI glyph legends (accessibility — glyphs
/// are otherwise undocumented). Independent of the per-PR `Overlay` state.
pub fn render_help_overlay(f: &mut Frame, state: &AppState) {
    if !state.help_open {
        return;
    }

    let area = f.area();
    let modal_width = 66u16.clamp(40, area.width.saturating_sub(4));
    let modal_height = 19u16.min(area.height.saturating_sub(2));
    let x = (area.width.saturating_sub(modal_width)) / 2;
    let y = (area.height.saturating_sub(modal_height)) / 2;
    let modal_area = Rect {
        x,
        y,
        width: modal_width,
        height: modal_height,
    };

    let block = Block::default()
        .title(" Help ")
        .borders(Borders::ALL)
        .border_style(theme::BORDER_FOCUSED);

    let key = |k: &'static str, desc: &'static str| {
        Line::from(vec![
            Span::styled(format!("  {:<12}", k), theme::PR_NUMBER),
            Span::raw(desc),
        ])
    };

    let lines = vec![
        Line::from(Span::styled("Keys", theme::HEADER)),
        key("j / k", "move up / down (scroll in diff)"),
        key("Enter", "select / expand"),
        key("l", "git-log overlay (content pane)"),
        key("d", "diff overlay (content pane)"),
        key("f", "cycle merge filter: all -> conflicting -> clean"),
        key(
            "p",
            "switch profile (modal picker; active shown in status bar)",
        ),
        key("/", "search    r  refresh    o  open in browser"),
        key("Tab", "switch pane    h / Esc  back / close    q  quit"),
        Line::from(""),
        Line::from(Span::styled("State column", theme::HEADER)),
        Line::from(vec![
            Span::styled("  ✓ ok", theme::MERGE_CLEAN),
            Span::raw(" mergeable   "),
            Span::styled("✗ cf", theme::MERGE_CONFLICT),
            Span::raw(" conflicting   "),
            Span::styled("?", theme::DIM),
            Span::raw(" unknown (not yet computed)"),
        ]),
        Line::from(Span::styled("CI column", theme::HEADER)),
        Line::from(vec![
            Span::styled("  ✓", theme::MERGE_CLEAN),
            Span::raw(" passing   "),
            Span::styled("✗", theme::MERGE_CONFLICT),
            Span::raw(" failing   "),
            Span::styled("…", theme::WARNING),
            Span::raw(" pending   "),
            Span::styled("·", theme::DIM),
            Span::raw(" no checks"),
        ]),
        Line::from(""),
        Line::from(Span::styled("Press ? or Esc to close", theme::DIM)),
    ];

    f.render_widget(Clear, modal_area);
    let para = Paragraph::new(lines).block(block);
    f.render_widget(para, modal_area);
}
