use super::{named, pick, reported_target_window, reported_window, settled};
use agent_desktop_core::{Rect, WindowInfo};
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

fn window(id: &str, width: f64, height: f64) -> WindowInfo {
    serde_json::from_value(json!({
        "id": id,
        "title": id,
        "app_name": "TextEdit",
        "pid": 1,
        "bounds": {"x": 10.0, "y": 20.0, "width": width, "height": height},
        "is_focused": false,
    }))
    .expect("window info")
}

#[test]
fn a_cue_never_lands_on_a_tiny_affordance_listed_first() {
    let windows = [window("w-tools", 53.0, 48.0), window("w-doc", 760.0, 560.0)];
    let bounds: Rect = pick(&windows, None).expect("a window");
    assert_eq!((bounds.width, bounds.height), (760.0, 560.0));
}

#[test]
fn a_cue_prefers_the_window_the_result_names() {
    let windows = [window("w-big", 900.0, 700.0), window("w-doc", 760.0, 560.0)];
    let target = json!({"target": {"kind": "window", "id": "w-doc"}, "x": 700.0});
    let id = reported_target_window(&target);
    assert_eq!(id, Some("w-doc"));
    assert_eq!(pick(&windows, id).expect("a window").width, 760.0);
    assert_eq!(
        pick(&windows, Some("w-gone")).expect("largest").width,
        900.0
    );
    assert!(reported_target_window(&json!({"target": {"kind": "app", "id": "x"}})).is_none());
}

#[test]
fn the_leaving_hold_fits_inside_the_arrival_budget() {
    const _: () = assert!(
        agent_desktop_core::CURSOR_ANNOUNCE_LEAD_MS * 2
            <= agent_desktop_core::CURSOR_ARRIVAL_TIMEOUT_MS
    );
}

fn rect(x: f64, y: f64, width: f64, height: f64) -> Rect {
    Rect {
        x,
        y,
        width,
        height,
    }
}

#[test]
fn a_moved_window_is_outlined_where_it_now_is() {
    let was = rect(1490.0, 250.0, 230.0, 280.0);
    let asked = rect(820.0, 220.0, 230.0, 280.0);
    let live = rect(822.0, 220.0, 230.0, 280.0);
    assert_eq!(settled(Some(live), Some(was), Some(asked)), Some(live));
}

#[test]
fn a_window_list_still_showing_the_old_frame_uses_the_frame_asked_for() {
    let was = rect(1490.0, 250.0, 230.0, 280.0);
    let asked = rect(820.0, 220.0, 230.0, 280.0);
    assert_eq!(settled(Some(was), Some(was), Some(asked)), Some(asked));
    assert_eq!(settled(None, Some(was), Some(asked)), Some(asked));
    assert_eq!(settled(Some(was), None, None), Some(was));
}
