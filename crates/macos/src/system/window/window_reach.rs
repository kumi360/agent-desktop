use agent_desktop_core::{AdapterError, DeliverySemantics, ErrorCode, Rect};

/// How much of a window's top-left corner must land on a display for a person
/// to see and grab it: a stretch of title bar wide enough to drag.
const GRIP: Rect = Rect {
    x: 0.0,
    y: 0.0,
    width: 80.0,
    height: 20.0,
};

/// Refuses a move that would put the window's title bar on no display. The
/// window server would pull such a window back to the screen edge, so the
/// requested spot can never be reached; saying so up front leaves the window
/// where the person last saw it instead of failing after a long wait.
pub(crate) fn ensure_reachable(x: f64, y: f64, displays: &[Rect]) -> Result<(), AdapterError> {
    if displays.is_empty() || displays.iter().any(|display| holds_grip(*display, x, y)) {
        return Ok(());
    }
    Err(AdapterError::new(
        ErrorCode::InvalidArgs,
        format!("({x}, {y}) is off every display, so the window was not moved"),
    )
    .with_suggestion(format!(
        "Pick a spot on a display. They are: {}. Run `agent-desktop list-displays` for details.",
        displays
            .iter()
            .map(|d| format!("{}x{} at ({}, {})", d.width, d.height, d.x, d.y))
            .collect::<Vec<_>>()
            .join("; ")
    ))
    .with_disposition(DeliverySemantics::not_delivered()))
}

fn holds_grip(display: Rect, x: f64, y: f64) -> bool {
    x >= display.x - GRIP.width + GRIP.height
        && x + GRIP.width <= display.x + display.width + GRIP.width - GRIP.height
        && y >= display.y - 1.0
        && y + GRIP.height <= display.y + display.height
}

#[cfg(test)]
#[path = "window_reach_tests.rs"]
mod tests;
