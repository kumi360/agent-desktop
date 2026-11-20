//! A cue for a target with nothing on screen to point at, such as an item in
//! a closed menu, an off-screen row, or an element Accessibility reports
//! without bounds. Instead of acting with no visible sign, the cursor glides
//! to the menu bar (for menu items) or to the target window's title bar and
//! says what it is choosing, so a person watching still sees the step.

use crate::{CommandContext, PlatformAdapter, Point, Rect, RefEntry, SnapshotSurface};

const MENU_BAR_INSET_X: f64 = 160.0;
const MENU_BAR_MID_Y: f64 = 12.0;

pub(super) fn present(adapter: &dyn PlatformAdapter, context: &CommandContext, entry: &RefEntry) {
    if context.overlay_key().is_none() {
        return;
    }
    let narration = narration(entry);
    let Ok(deadline) = crate::Deadline::detached_after(600) else {
        return;
    };
    if !is_menu(&entry.source.source_surface)
        && let Some(window) = source_window(adapter, entry, deadline)
    {
        crate::cursor_overlay::present_window_cue(adapter, context, &window, &narration);
        return;
    }
    if let Some(point) = menu_bar_point(adapter, deadline) {
        crate::cursor_overlay::present_cue(adapter, context, point, None, &narration);
    }
}

pub(super) fn narration(entry: &RefEntry) -> String {
    let name = entry
        .identity
        .name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty());
    match name {
        Some(name) if is_menu(&entry.source.source_surface) => format!("Choosing {name}"),
        Some(name) => name.to_owned(),
        None => "Working here".to_owned(),
    }
}

fn is_menu(surface: &SnapshotSurface) -> bool {
    matches!(surface, SnapshotSurface::Menu | SnapshotSurface::Menubar)
}

fn source_window(
    adapter: &dyn PlatformAdapter,
    entry: &RefEntry,
    deadline: crate::Deadline,
) -> Option<Rect> {
    let filter = crate::WindowFilter {
        focused_only: false,
        app: entry.source.source_app.clone(),
    };
    let windows = adapter.list_windows(&filter, deadline).ok()?;
    let wanted = entry.source.source_window_id.as_deref();
    windows
        .iter()
        .find(|window| Some(window.id.as_str()) == wanted)
        .or_else(|| {
            windows
                .iter()
                .find(|window| window.pid == entry.process.pid)
        })
        .and_then(|window| window.bounds)
}

fn menu_bar_point(adapter: &dyn PlatformAdapter, deadline: crate::Deadline) -> Option<Point> {
    let displays = adapter.list_displays(deadline).ok()?;
    let display = displays
        .iter()
        .find(|display| display.is_primary)
        .or_else(|| displays.first())?;
    Some(Point {
        x: display.bounds.x + MENU_BAR_INSET_X.min(display.bounds.width / 2.0),
        y: display.bounds.y + MENU_BAR_MID_Y,
    })
}

#[cfg(test)]
#[path = "hidden_cue_tests.rs"]
mod tests;
