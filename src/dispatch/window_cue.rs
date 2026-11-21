//! Calm cues for window-level commands. Opening an app, switching windows,
//! moving, resizing, minimizing or closing used to happen with no visible
//! sign on screen. Each now sends the cursor gliding to the window's title
//! bar, with a soft outline and a short label, so a person watching can see
//! what the agent is doing. Cues are presentation only: a cue that cannot be
//! drawn never changes a command's result.

use agent_desktop_core::{
    PlatformAdapter, Rect, WindowFilter, WindowInfo, context::CommandContext, present_window_cue,
    present_window_gone,
};
use serde_json::Value;

/// Shows the cue for a window that already exists before the command acts
/// on it, so the label is read before anything on screen changes. Returns
/// whether a cue was shown; a command whose window does not exist yet (an
/// app being launched) shows its cue after it acts instead.
pub(super) fn before(
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
    app: Option<&str>,
    window_id: Option<&str>,
    narration: &str,
) -> bool {
    before_at(adapter, context, app, window_id, narration).is_some()
}

/// Like [`before`], returning the outlined window's bounds so a command that
/// takes the window away can clear its outline with [`gone`] afterwards.
pub(super) fn before_at(
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
    app: Option<&str>,
    window_id: Option<&str>,
    narration: &str,
) -> Option<Rect> {
    context.overlay_key()?;
    let bounds = lookup(adapter, app, window_id)?;
    present_window_cue(adapter, context, &bounds, narration);
    Some(bounds)
}

/// Like [`before_at`] for a command that takes the window away (closing or
/// minimizing it). The label then stays on the window for the announcement
/// lead before the command acts, so a person reads which window is leaving
/// even when the cursor was already on it and had no glide to make.
pub(super) fn before_leaving(
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
    app: Option<&str>,
    window_id: Option<&str>,
    narration: &str,
) -> Option<Rect> {
    let bounds = before_at(adapter, context, app, window_id, narration)?;
    std::thread::sleep(std::time::Duration::from_millis(
        agent_desktop_core::CURSOR_ANNOUNCE_LEAD_MS,
    ));
    Some(bounds)
}

/// After a window left the screen (closed or minimized), says so in the past
/// tense and clears its outline, so nothing is left pointing at empty space.
pub(super) fn gone(
    context: &CommandContext,
    adapter: &dyn PlatformAdapter,
    left: Option<Rect>,
    narration: &str,
) {
    if let Some(bounds) = left {
        present_window_gone(adapter, context, &bounds, narration);
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
    let target = reported_target_window(result);
    let bounds = found
        .and_then(|(bounds, _)| bounds)
        .or_else(|| lookup(adapter, app_name.as_deref(), target));
    if let Some(bounds) = bounds {
        present_window_cue(adapter, context, &bounds, &narration(app_name.as_deref()));
    }
}

/// After a command changed a window's frame (moving or resizing it), says so
/// and outlines the window where it now is, so no outline is left on where it
/// used to be. `asked` is the frame the command set; the window list is read
/// only when there is none.
pub(super) fn changed(
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
    scope: (Option<&str>, Option<&str>),
    asked: Option<Rect>,
    narration: &str,
) {
    if context.overlay_key().is_none() {
        return;
    }
    let bounds = settled(asked, || lookup(adapter, scope.0, scope.1));
    if let Some(bounds) = bounds {
        present_window_cue(adapter, context, &bounds, narration);
    }
}

/// The frame a changed window ends on: the frame the command asked for.
/// Some apps animate a move or resize, so the window list read straight
/// after the command can still report the old frame, and a cue drawn there
/// would point at where the window used to be.
fn settled(asked: Option<Rect>, live: impl FnOnce() -> Option<Rect>) -> Option<Rect> {
    asked.or_else(live)
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
    pick(&windows, window_id)
}

/// The window a cue should point at: the one with the given id, or else the
/// app's largest window. The first listed window can be a tiny floating
/// affordance, such as the Writing Tools button, and a cue on it looks lost.
fn pick(windows: &[WindowInfo], window_id: Option<&str>) -> Option<Rect> {
    let sized = || {
        windows
            .iter()
            .filter_map(|window| window.bounds.map(|b| (window, b)))
    };
    if let Some(id) = window_id
        && let Some((_, bounds)) = sized().find(|(window, _)| window.id == id)
    {
        return Some(bounds);
    }
    sized()
        .map(|(_, bounds)| bounds)
        .max_by(|a, b| (a.width * a.height).total_cmp(&(b.width * b.height)))
}

/// The id of the window a result names as its target, such as move-window's
/// `target: {kind: "window", id: "w-1"}`.
fn reported_target_window(value: &Value) -> Option<&str> {
    let target = value.get("target")?;
    (target.get("kind").and_then(Value::as_str) == Some("window"))
        .then(|| target.get("id").and_then(Value::as_str))
        .flatten()
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
