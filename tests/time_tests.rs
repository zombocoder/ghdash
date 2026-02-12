use chrono::{Duration, Utc};
use ghdash::util::time::relative_time;

#[test]
fn test_just_now() {
    let now = Utc::now();
    assert_eq!(relative_time(&now), "just now");
}

#[test]
fn test_seconds_ago() {
    let t = Utc::now() - Duration::seconds(30);
    assert_eq!(relative_time(&t), "just now");
}

#[test]
fn test_one_minute_ago() {
    let t = Utc::now() - Duration::minutes(1);
    assert_eq!(relative_time(&t), "1m ago");
}

#[test]
fn test_minutes_ago() {
    let t = Utc::now() - Duration::minutes(45);
    assert_eq!(relative_time(&t), "45m ago");
}

#[test]
fn test_one_hour_ago() {
    let t = Utc::now() - Duration::hours(1);
    assert_eq!(relative_time(&t), "1h ago");
}

#[test]
fn test_hours_ago() {
    let t = Utc::now() - Duration::hours(23);
    assert_eq!(relative_time(&t), "23h ago");
}

#[test]
fn test_one_day_ago() {
    let t = Utc::now() - Duration::days(1);
    assert_eq!(relative_time(&t), "1d ago");
}

#[test]
fn test_days_ago() {
    let t = Utc::now() - Duration::days(15);
    assert_eq!(relative_time(&t), "15d ago");
}

#[test]
fn test_one_month_ago() {
    let t = Utc::now() - Duration::days(35);
    assert_eq!(relative_time(&t), "1mo ago");
}

#[test]
fn test_months_ago() {
    let t = Utc::now() - Duration::days(200);
    assert_eq!(relative_time(&t), "6mo ago");
}

#[test]
fn test_one_year_ago() {
    let t = Utc::now() - Duration::days(400);
    assert_eq!(relative_time(&t), "1y ago");
}

#[test]
fn test_years_ago() {
    let t = Utc::now() - Duration::days(1000);
    assert_eq!(relative_time(&t), "2y ago");
}

#[test]
fn test_future_timestamp() {
    let t = Utc::now() + Duration::hours(5);
    assert_eq!(relative_time(&t), "just now");
}

#[test]
fn test_boundary_59_seconds() {
    let t = Utc::now() - Duration::seconds(59);
    assert_eq!(relative_time(&t), "just now");
}

#[test]
fn test_boundary_60_seconds() {
    let t = Utc::now() - Duration::seconds(61);
    assert_eq!(relative_time(&t), "1m ago");
}

#[test]
fn test_boundary_59_minutes() {
    let t = Utc::now() - Duration::minutes(59);
    assert_eq!(relative_time(&t), "59m ago");
}

#[test]
fn test_boundary_29_days() {
    let t = Utc::now() - Duration::days(29);
    assert_eq!(relative_time(&t), "29d ago");
}

#[test]
fn test_boundary_30_days() {
    let t = Utc::now() - Duration::days(30);
    assert_eq!(relative_time(&t), "1mo ago");
}

#[test]
fn test_boundary_364_days() {
    let t = Utc::now() - Duration::days(364);
    assert_eq!(relative_time(&t), "12mo ago");
}

#[test]
fn test_boundary_365_days() {
    let t = Utc::now() - Duration::days(365);
    assert_eq!(relative_time(&t), "1y ago");
}
