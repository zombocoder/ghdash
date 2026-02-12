use ratatui::style::{Color, Modifier, Style};

pub const HIGHLIGHT: Style = Style::new()
    .fg(Color::Black)
    .bg(Color::Cyan)
    .add_modifier(Modifier::BOLD);

pub const HEADER: Style = Style::new().fg(Color::White).add_modifier(Modifier::BOLD);

pub const DIM: Style = Style::new().fg(Color::DarkGray);

pub const ERROR: Style = Style::new().fg(Color::Red).add_modifier(Modifier::BOLD);

pub const DRAFT: Style = Style::new().fg(Color::DarkGray);

#[allow(dead_code)]
pub const SUCCESS: Style = Style::new().fg(Color::Green);

#[allow(dead_code)]
pub const WARNING: Style = Style::new().fg(Color::Yellow);

pub const BORDER_FOCUSED: Style = Style::new().fg(Color::Cyan);

pub const BORDER_UNFOCUSED: Style = Style::new().fg(Color::DarkGray);

pub const STATUS_BAR: Style = Style::new().fg(Color::White).bg(Color::DarkGray);

pub const NAV_ORG: Style = Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD);

pub const NAV_REPO: Style = Style::new().fg(Color::White);

pub const NAV_VIRTUAL: Style = Style::new().fg(Color::Magenta).add_modifier(Modifier::BOLD);

pub const PR_NUMBER: Style = Style::new().fg(Color::Cyan);

pub const PR_AUTHOR: Style = Style::new().fg(Color::Yellow);
