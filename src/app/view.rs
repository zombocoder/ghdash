use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
};

use crate::app::state::AppState;
use crate::ui::widgets;

pub fn render(f: &mut Frame, state: &AppState) {
    // Main layout: body + status bar
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(f.area());

    let body_area = vertical[0];
    let status_area = vertical[1];

    // Body: nav pane + content pane
    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(body_area);

    let nav_area = horizontal[0];
    let content_area = horizontal[1];

    widgets::render_nav_pane(f, nav_area, state);
    widgets::render_content_pane(f, content_area, state);
    widgets::render_status_bar(f, status_area, state);

    // Overlays
    widgets::render_search_overlay(f, state);
    if state.error_message.is_some() {
        widgets::render_error_modal(f, f.area(), state);
    }
}
