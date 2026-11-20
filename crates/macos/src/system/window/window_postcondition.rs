use agent_desktop_core::{AdapterError, Deadline, DeliverySemantics, Rect};

pub(crate) const TOLERANCE: f64 = 2.0;

pub(crate) fn wait_for_geometry(
    element: &crate::tree::AXElement,
    expected: Rect,
    position: bool,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    wait_until(deadline, || {
        let bounds = crate::tree::element_bounds::read_bounds_with_deadline(
            element,
            deadline_instant(deadline)?,
        )?;
        Ok(bounds.is_some_and(|actual| geometry_matches(actual, expected, position)))
    })
}

/// How far the window manager may keep a maximized window from the work
/// area, for example below a menu bar taller than AppKit's visible frame.
pub(crate) const WORK_AREA_SLACK: f64 = 16.0;

/// Waits until a window being maximized has reached the work area's corner
/// or, with `filled`, fills the whole work area. Both allow the small
/// adjustment the window manager makes to keep a window on screen. The
/// corner must settle before the resize, or the window manager sizes the
/// window from where it was and pushes it back.
pub(crate) fn wait_for_work_area(
    element: &crate::tree::AXElement,
    work_area: Rect,
    filled: bool,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    let mut held_since: Option<std::time::Instant> = None;
    wait_until(deadline, || {
        let bounds = crate::tree::element_bounds::read_bounds_with_deadline(
            element,
            deadline_instant(deadline)?,
        )?;
        let reached = bounds.is_some_and(|actual| {
            if filled {
                fills(actual, work_area)
            } else {
                at_corner(actual, work_area)
            }
        });
        let now = std::time::Instant::now();
        held_since = reached.then(|| held_since.unwrap_or(now));
        Ok(held_since.is_some_and(|since| now.duration_since(since) >= SETTLE))
    })
}

/// How long a window must stay where it was put before the next step: a
/// window manager can still pull a just-moved window back for a moment.
const SETTLE: std::time::Duration = std::time::Duration::from_millis(250);

fn at_corner(actual: Rect, area: Rect) -> bool {
    (actual.x - area.x).abs() <= WORK_AREA_SLACK && (actual.y - area.y).abs() <= WORK_AREA_SLACK
}

fn fills(actual: Rect, area: Rect) -> bool {
    let left = (actual.x - area.x).abs();
    let top = (actual.y - area.y).abs();
    let right = ((actual.x + actual.width) - (area.x + area.width)).abs();
    let bottom = ((actual.y + actual.height) - (area.y + area.height)).abs();
    [left, top, right, bottom]
        .into_iter()
        .all(|gap| gap <= WORK_AREA_SLACK)
}

pub(crate) fn wait_for_minimized(
    element: &crate::tree::AXElement,
    expected: bool,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    wait_until(deadline, || {
        let read =
            crate::tree::surface_read::boolean(element, "AXMinimized", deadline_instant(deadline)?);
        Ok(settled(read)? == Some(expected))
    })
}

/// A window mid-animation, such as one flying into the Dock, answers its
/// minimized read with "cannot complete". That is a busy read, not a
/// failure: keep waiting for the window to settle until the deadline.
fn settled(read: Result<Option<bool>, AdapterError>) -> Result<Option<bool>, AdapterError> {
    match read {
        Err(error) if busy(&error) => Ok(None),
        other => other,
    }
}

fn busy(error: &AdapterError) -> bool {
    error
        .details
        .as_ref()
        .and_then(|details| details.get("retryable"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
}

fn wait_until(
    deadline: Deadline,
    mut complete: impl FnMut() -> Result<bool, AdapterError>,
) -> Result<(), AdapterError> {
    loop {
        if complete().map_err(after_delivery)? {
            return Ok(());
        }
        if deadline.is_expired() {
            return Err(after_delivery(AdapterError::timeout(
                "Window operation did not reach its requested postcondition",
            )));
        }
        let pause = deadline
            .remaining_slice(std::time::Duration::from_millis(10))
            .map_err(after_delivery)?;
        std::thread::sleep(pause.min(std::time::Duration::from_millis(10)));
    }
}

fn geometry_matches(actual: Rect, expected: Rect, position: bool) -> bool {
    let values = if position {
        [(actual.x, expected.x), (actual.y, expected.y)]
    } else {
        [
            (actual.width, expected.width),
            (actual.height, expected.height),
        ]
    };
    values
        .into_iter()
        .all(|(actual, expected)| (actual - expected).abs() <= TOLERANCE)
}

fn deadline_instant(deadline: Deadline) -> Result<std::time::Instant, AdapterError> {
    std::time::Instant::now()
        .checked_add(deadline.remaining())
        .ok_or_else(|| AdapterError::internal("Window operation deadline is out of range"))
}

fn after_delivery(error: AdapterError) -> AdapterError {
    error.with_disposition(DeliverySemantics::delivered_unverified())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geometry_verification_tolerates_ax_rounding_only() {
        let expected = Rect {
            x: 10.0,
            y: 20.0,
            width: 100.0,
            height: 80.0,
        };
        let rounded = Rect {
            x: 11.0,
            y: 19.0,
            width: 99.0,
            height: 81.0,
        };
        let wrong = Rect {
            width: 90.0,
            ..rounded
        };

        assert!(geometry_matches(rounded, expected, true));
        assert!(geometry_matches(rounded, expected, false));
        assert!(!geometry_matches(wrong, expected, false));
    }

    #[test]
    fn a_maximized_window_kept_below_a_taller_menu_bar_still_fills_the_work_area() {
        let area = Rect {
            x: 0.0,
            y: 25.0,
            width: 2560.0,
            height: 1415.0,
        };
        let below_menu_bar = Rect {
            x: 0.0,
            y: 30.0,
            width: 2560.0,
            height: 1410.0,
        };
        let half = Rect {
            x: 700.0,
            y: 160.0,
            width: 1860.0,
            height: 1280.0,
        };
        assert!(fills(below_menu_bar, area));
        assert!(!fills(half, area));
        assert!(at_corner(below_menu_bar, area));
        assert!(!at_corner(half, area));
    }

    #[test]
    fn a_busy_minimized_read_mid_animation_keeps_waiting() {
        let busy_read = crate::tree::surface_read::map_error(
            "AXMinimized",
            accessibility_sys::kAXErrorCannotComplete,
        );
        assert_eq!(settled(Err(busy_read)).unwrap(), None);
        let denied = crate::tree::surface_read::map_error(
            "AXMinimized",
            accessibility_sys::kAXErrorAPIDisabled,
        );
        assert!(settled(Err(denied)).is_err());
        assert_eq!(settled(Ok(Some(true))).unwrap(), Some(true));
    }
}
