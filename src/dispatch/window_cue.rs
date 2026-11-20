//! Calm cues for window-level commands. Opening an app, switching windows,
//! moving, resizing, minimizing or closing used to happen with no visible
//! sign on screen. Each now sends the cursor gliding to the window's title
//! bar, with a soft outline and a short label, so a person watching can see
//! what the agent is doing. Cues are presentation only: a cue that cannot be
//! drawn never changes a command's result.

use agent_desktop_core::{
    PlatformAdapter, Rect, WindowFilter, context::CommandContext, present_window_cue,
};
use serde_json::Value;

/// Shows the cue for a window that already exists before the command acts
/// on it, such as a window about to be minimized or an app about to close.
pub(super) fn before(
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
    app: Option<&str>,
    window_id: Option<&str>,
    narration: &str,
) {
    if context.overlay_key().is_none() {
        return;
    }
    if let Some(bounds) = lookup(adapter, app, window_id) {
        present_window_cue(adapter, context, &bounds, narration);
    }
}

/// Shows the cue after a command succeeded, using the window the result
/// reports, or else the app's frontmost window.
pub(super) fn after(
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
    result: &Value,
    app: Option<&str>,
    narration: impl FnOnce(Option<&str>) -> String,
) {
    if context.overlay_key().is_none() {
        return;
    }
    let found = reported_window(result);
    let app_name = found
        .as_ref()
        .and_then(|(_, name)| name.clone())
        .or_else(|| app.map(str::to_owned));
    let bounds = found
        .and_then(|(bounds, _)| bounds)
        .or_else(|| lookup(adapter, app_name.as_deref(), None));
    if let Some(bounds) = bounds {
        present_window_cue(adapter, context, &bounds, &narration(app_name.as_deref()));
    }
}

pub(super) fn named(verb: &str, app: Option<&str>) -> String {
    match app.map(str::trim).filter(|app| !app.is_empty()) {
        Some(app) => format!("{verb} {app}"),
        None => format!("{verb} window"),
    }
}

fn lookup(
    adapter: &dyn PlatformAdapter,
    app: Option<&str>,
    window_id: Option<&str>,
) -> Option<Rect> {
    if app.is_none() && window_id.is_none() {
        return None;
    }
    let deadline = agent_desktop_core::Deadline::detached_after(1_500).ok()?;
    let filter = WindowFilter {
        focused_only: false,
        app: app.map(str::to_owned),
    };
    let windows = adapter.list_windows(&filter, deadline).ok()?;
    windows
        .into_iter()
        .filter(|window| window_id.is_none_or(|id| window.id == id))
        .find_map(|window| window.bounds)
}

/// Finds the first object in a result that describes a window, by its
/// `bounds` and `app_name`.
fn reported_window(value: &Value) -> Option<(Option<Rect>, Option<String>)> {
    match value {
        Value::Object(map) => {
            if let Some(bounds) = map.get("bounds") {
                let rect = serde_json::from_value::<Rect>(bounds.clone()).ok();
                if rect.is_some() || map.contains_key("app_name") {
                    let name = map
                        .get("app_name")
                        .and_then(Value::as_str)
                        .map(str::to_owned);
                    return Some((rect, name));
                }
            }
            map.values().find_map(reported_window)
        }
        Value::Array(items) => items.iter().find_map(reported_window),
        _ => None,
    }
}

#[cfg(test)]
#[path = "window_cue_tests.rs"]
mod tests;
