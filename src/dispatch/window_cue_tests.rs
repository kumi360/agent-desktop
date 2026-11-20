use super::{named, reported_window};
use serde_json::json;

#[test]
fn finds_window_reported_by_launch() {
    let result = json!({
        "launched": {
            "id": "w-1",
            "title": "Untitled",
            "app_name": "TextEdit",
            "bounds": {"x": 100.0, "y": 80.0, "width": 600.0, "height": 400.0}
        }
    });
    let (bounds, name) = reported_window(&result).expect("window");
    let bounds = bounds.expect("bounds");
    assert_eq!((bounds.x, bounds.width), (100.0, 600.0));
    assert_eq!(name.as_deref(), Some("TextEdit"));
}

#[test]
fn window_without_bounds_still_names_app() {
    let result = json!({"focused": {"app_name": "Finder", "bounds": null}});
    let (bounds, name) = reported_window(&result).expect("window");
    assert!(bounds.is_none());
    assert_eq!(name.as_deref(), Some("Finder"));
}

#[test]
fn result_without_window_is_ignored() {
    assert!(reported_window(&json!({"closed": true, "app": "TextEdit"})).is_none());
}

#[test]
fn narration_names_app_or_window() {
    assert_eq!(named("Opening", Some("TextEdit")), "Opening TextEdit");
    assert_eq!(named("Moving", Some("  ")), "Moving window");
    assert_eq!(named("Moving", None), "Moving window");
}
