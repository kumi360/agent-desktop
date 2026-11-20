//! The on-screen part of a resolved element: the element's live frame cut by
//! every clipping ancestor, refused when its window is minimized or its
//! application hidden, then cut by the attached displays.

use agent_desktop_core::{AdapterError, Deadline, Rect, adapter::hit_test::TargetVisibility};

use super::AXElement;
use super::element_bounds::{clips_descendants, intersect_rects, read_bounds_with_deadline};

/// Reads where `element` can be seen right now. Any read that fails, any
/// ancestry that cannot be walked to its application, and any element with no
/// owning window (a closed menu's items have that shape) yields `Unknown`, so
/// missing evidence never turns into a false refusal.
pub(crate) fn target_visibility(
    element: &AXElement,
    deadline: Deadline,
) -> Result<TargetVisibility, AdapterError> {
    let instant = super::locator_deadline::from_operation(deadline)?;
    let Some(bounds) = read_bounds_with_deadline(element, instant).ok().flatten() else {
        return Ok(TargetVisibility::Unknown);
    };
    let visible = match clip_by_ancestors(element, bounds, instant) {
        Clip::Visible(_) if bounds.width <= 0.0 || bounds.height <= 0.0 => {
            return Ok(hidden("the element has no on-screen area"));
        }
        Clip::Visible(visible) => visible,
        Clip::Hidden(reason) => return Ok(hidden(reason)),
        Clip::Unknown => return Ok(TargetVisibility::Unknown),
    };
    let displays = match crate::system::display::list_displays_impl(deadline) {
        Ok(displays) if !displays.is_empty() => displays,
        _ => return Ok(TargetVisibility::Visible { bounds: visible }),
    };
    Ok(
        on_displays(visible, displays.iter().map(|display| display.bounds)).map_or_else(
            || hidden("the element is outside every attached display"),
            |bounds| TargetVisibility::Visible { bounds },
        ),
    )
}

/// The largest part of `visible` that falls on one display. A target split
/// across two displays is aimed at where most of it is.
pub(crate) fn on_displays(visible: Rect, displays: impl Iterator<Item = Rect>) -> Option<Rect> {
    displays
        .filter_map(|display| intersect_rects(visible, display))
        .max_by(|left, right| (left.width * left.height).total_cmp(&(right.width * right.height)))
}

enum Clip {
    Visible(Rect),
    Hidden(String),
    Unknown,
}

fn hidden(reason: impl Into<String>) -> TargetVisibility {
    TargetVisibility::Hidden {
        reason: reason.into(),
    }
}

fn clip_by_ancestors(element: &AXElement, bounds: Rect, deadline: std::time::Instant) -> Clip {
    let mut visible = bounds;
    let mut in_window = false;
    let mut current = Some(element.clone());
    for _ in 0..super::properties::element::ABSOLUTE_MAX_DEPTH {
        let Some(node) = current else {
            return if in_window {
                Clip::Visible(visible)
            } else {
                Clip::Unknown
            };
        };
        let Ok(Some(role)) = super::surface_read::string(&node, "AXRole", deadline) else {
            return Clip::Unknown;
        };
        if role == "AXApplication" {
            if !in_window {
                return Clip::Unknown;
            }
            if super::surface_read::boolean(&node, "AXHidden", deadline)
                .ok()
                .flatten()
                == Some(true)
            {
                return Clip::Hidden("its application is hidden".into());
            }
            return Clip::Visible(visible);
        }
        in_window |= role == "AXWindow";
        if role == "AXWindow"
            && super::surface_read::boolean(&node, "AXMinimized", deadline)
                .ok()
                .flatten()
                == Some(true)
        {
            return Clip::Hidden("its window is minimized".into());
        }
        if clips_descendants(&role) && bounds.width > 0.0 && bounds.height > 0.0 {
            let Ok(Some(frame)) = read_bounds_with_deadline(&node, deadline) else {
                return Clip::Unknown;
            };
            match intersect_rects(visible, frame) {
                Some(inside) => visible = inside,
                None => return Clip::Hidden(format!("it is clipped out of view by its {role}")),
            }
        }
        current = match super::surface_read::element(&node, "AXParent", deadline) {
            Ok(parent) => parent,
            Err(_) => return Clip::Unknown,
        };
    }
    Clip::Unknown
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: f64, y: f64, width: f64, height: f64) -> Rect {
        Rect {
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn a_target_off_every_display_has_no_visible_part() {
        let displays = [rect(0.0, 0.0, 2560.0, 1440.0)];
        assert_eq!(
            on_displays(rect(3000.0, 100.0, 80.0, 20.0), displays.into_iter()),
            None
        );
    }

    #[test]
    fn a_target_straddling_the_display_edge_keeps_only_its_on_screen_part() {
        let displays = [rect(0.0, 0.0, 2560.0, 1440.0)];
        assert_eq!(
            on_displays(rect(2540.0, 100.0, 80.0, 20.0), displays.into_iter()),
            Some(rect(2540.0, 100.0, 20.0, 20.0))
        );
    }

    #[test]
    fn a_target_across_two_displays_is_aimed_at_its_larger_part() {
        let displays = [
            rect(0.0, 0.0, 1000.0, 800.0),
            rect(1000.0, 0.0, 1000.0, 800.0),
        ];
        assert_eq!(
            on_displays(rect(980.0, 10.0, 100.0, 20.0), displays.into_iter()),
            Some(rect(1000.0, 10.0, 80.0, 20.0))
        );
    }
}
