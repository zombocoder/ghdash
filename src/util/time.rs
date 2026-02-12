use chrono::{DateTime, Utc};

/// Format a timestamp as a human-readable relative time string.
pub fn relative_time(dt: &DateTime<Utc>) -> String {
    let now = Utc::now();
    let duration = now.signed_duration_since(dt);

    if duration.num_seconds() < 0 {
        return "just now".to_string();
    }

    let seconds = duration.num_seconds();
    if seconds < 60 {
        return "just now".to_string();
    }

    let minutes = duration.num_minutes();
    if minutes < 60 {
        return format!("{}m ago", minutes);
    }

    let hours = duration.num_hours();
    if hours < 24 {
        return format!("{}h ago", hours);
    }

    let days = duration.num_days();
    if days < 30 {
        return format!("{}d ago", days);
    }

    if days < 365 {
        let months = days / 30;
        return format!("{}mo ago", months);
    }

    let years = days / 365;
    format!("{}y ago", years)
}
