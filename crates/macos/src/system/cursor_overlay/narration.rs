//! How long an action's narration stays on the cursor's label.

use agent_desktop_core::{CURSOR_NARRATION_HOLD_MS, CursorOverlayInstruction};
use std::time::{Duration, Instant};

/// When the label now on screen started narrating an action: a narrated
/// instruction restarts the hold, a caret keeps it, and anything else drew
/// its own label over the narration.
pub(super) fn after(
    instruction: &CursorOverlayInstruction,
    previous: Option<Instant>,
    now: Instant,
) -> Option<Instant> {
    if instruction.is_narrated() {
        Some(now)
    } else if super::bridge::is_caret(instruction) {
        previous
    } else {
        None
    }
}

/// A narration such as "Restoring TextEdit" fades once its action has had
/// time to settle, so it never captions the next, different action.
pub(super) fn due(narration: Option<Instant>, now: Instant) -> bool {
    narration.is_some_and(|at| {
        now.saturating_duration_since(at) >= Duration::from_millis(CURSOR_NARRATION_HOLD_MS)
    })
}

#[cfg(test)]
#[path = "narration_tests.rs"]
mod tests;
