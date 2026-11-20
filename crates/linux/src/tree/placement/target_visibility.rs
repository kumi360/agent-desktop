//! The on-screen part of a resolved element: its window rectangle cut by its
//! frame and scroll container, moved to screen coordinates, then cut by the
//! attached displays. Mirrors the macOS visible-target gate so the drawn
//! cursor never outlines, and a pointer never aims at, the clipped-away part.

use agent_desktop_core::{Rect, adapter::hit_test::TargetVisibility};

use super::Placement;
use super::frame_geometry::to_pointer_frame;
use super::offscreen::{OffscreenInput, offscreen};
use super::visibility::is_sentinel;
use crate::tree::Extents;
use crate::tree::hit_test::hit_guard::aimable;
use crate::tree::properties::toolkit_trust::reports_global_position;

/// Where the window-coordinate rectangles of `placed` sit on screen: the
/// matched popup's origin, or the X11 client origin plus the frame shadow a
/// non-global toolkit draws inside it. `None` when neither was read.
pub(crate) fn screen_origin(placed: &Placement) -> Option<(i32, i32)> {
    placed.popup_origin.or_else(|| {
        let window = placed.x11?;
        let shadow = if reports_global_position(&placed.toolkit) {
            (0, 0, 0, 0)
        } else {
            window.shadow
        };
        Some((
            window.client_origin.0.checked_add(shadow.0)?,
            window.client_origin.1.checked_add(shadow.1)?,
        ))
    })
}

fn hidden(reason: &str) -> TargetVisibility {
    TargetVisibility::Hidden {
        reason: reason.to_owned(),
    }
}

fn intersect(left: Extents, right: Extents) -> Option<Extents> {
    let x = left.x.max(right.x);
    let y = left.y.max(right.y);
    let right_edge = (i64::from(left.x) + i64::from(left.width))
        .min(i64::from(right.x) + i64::from(right.width));
    let bottom_edge = (i64::from(left.y) + i64::from(left.height))
        .min(i64::from(right.y) + i64::from(right.height));
    let width = i32::try_from(right_edge - i64::from(x)).ok()?;
    let height = i32::try_from(bottom_edge - i64::from(y)).ok()?;
    (width > 0 && height > 0).then_some(Extents {
        x,
        y,
        width,
        height,
    })
}

/// The largest part of `visible` that falls on one display; a target split
/// across two displays is aimed at where most of it is.
fn on_displays(visible: Extents, displays: &[Extents]) -> Option<Extents> {
    displays
        .iter()
        .filter_map(|display| intersect(visible, *display))
        .max_by_key(|part| i64::from(part.width) * i64::from(part.height))
}

/// Judges one live placement. Positive evidence only: a minimized or unmapped
/// window, or rectangles that were all read and leave no visible area, are
/// `Hidden`; anything unread, a Wayland surface without a position, or a
/// window several clients share is `Unknown`, which keeps the old behaviour.
pub(crate) fn target_visibility(placed: &Placement) -> TargetVisibility {
    if placed.frame_iconified == Some(true) || placed.x11.is_some_and(|window| window.hidden) {
        return hidden("its window is minimized or hidden");
    }
    if placed.wayland_unplaced || placed.tie.is_some() {
        return TargetVisibility::Unknown;
    }
    let (Some(node), Some(frame)) = (placed.node, placed.frame) else {
        return TargetVisibility::Unknown;
    };
    if !aimable(node) || !aimable(frame) {
        return TargetVisibility::Unknown;
    }
    let container = match placed.container {
        Some(container) if is_sentinel(container) => None,
        Some(container) if !aimable(container) => return TargetVisibility::Unknown,
        other => other,
    };
    let wholly_outside = offscreen(OffscreenInput {
        node: Some(node),
        frame: Some(frame),
        container,
        visible: true,
        showing: true,
        sentinel: false,
        minimized: Some(false),
    }) == Some(true);
    let clipped = intersect(node, frame).and_then(|inside| match container {
        Some(container) => intersect(inside, container),
        None => Some(inside),
    });
    let Some(clipped) = clipped.filter(|_| !wholly_outside) else {
        return hidden("it is scrolled or clipped out of its window");
    };
    let Some(screen) = screen_origin(placed).and_then(|origin| to_pointer_frame(origin, clipped))
    else {
        return TargetVisibility::Unknown;
    };
    let bounds = match placed.displays.as_deref() {
        Some(displays) if !displays.is_empty() => match on_displays(screen, displays) {
            Some(part) => part,
            None => return hidden("it is outside every attached display"),
        },
        _ => screen,
    };
    TargetVisibility::Visible {
        bounds: Rect::from(bounds),
    }
}

#[cfg(test)]
#[path = "target_visibility_tests.rs"]
mod tests;
