//! How a driven session opens and closes, and where its cursor comes back.
//! A session opens with its cursor fading in at the centre of the main
//! screen under a note of what it is about to do, and holds a moment before
//! the first step. It closes with "Completed, ending session" beside the
//! cursor, held long enough to read, before the cursor fades out. The last
//! point the cursor landed on is remembered, so a cursor brought back for a
//! later step continues from there instead of starting somewhere new.

use agent_desktop_core::{
    AMBIENT_CURSOR_ID, AdapterError, CURSOR_FAREWELL, CURSOR_FAREWELL_HOLD_MS,
    CURSOR_INTRO_HOLD_MS, CursorOverlayConfig, CursorOverlayInstruction, CursorPose, Point,
};
use std::path::PathBuf;
use std::time::Duration;

use super::bridge::{self, RenderPace};

/// How long the farewell fade takes once the note has been read.
const FAREWELL_FADE_SECONDS: f64 = 0.6;
const LANDING_FILE: &str = "cursor-at.json";

pub(super) fn is_session(session_id: &str) -> bool {
    session_id != AMBIENT_CURSOR_ID
}

/// A cursor at `point` with `label` beside it, standing still.
pub(super) fn still(
    point: &Point,
    label: Option<&str>,
) -> Result<CursorOverlayInstruction, AdapterError> {
    let config = CursorOverlayConfig::enabled(label.map(str::to_owned), 12)?;
    CursorOverlayInstruction::new(point.clone(), &config, false)
}

/// Draws the opening: the cursor fades in where it is (or at the centre of
/// the main screen when it is new) with the session's note, and a session
/// holds it there so the note can be read before the first step.
pub(super) fn open(
    instruction: &CursorOverlayInstruction,
    fresh: bool,
    session: bool,
) -> Result<(), AdapterError> {
    let point = instruction.destination();
    let (screen, fps, reduce_motion) = bridge::screen_at(point)?;
    let pace = RenderPace {
        fps,
        reduce_motion,
        announce: false,
        fade_in: fresh,
    };
    bridge::run(
        &[CursorPose::still(point.clone())],
        pace,
        instruction,
        &screen,
    )?;
    if session && instruction.label().is_some() {
        bridge::hold(Duration::from_millis(CURSOR_INTRO_HOLD_MS));
    }
    Ok(())
}

/// The closing note, held, then a calm fade, for a session that completed.
/// A session that expired or was switched off fades without the note, so the
/// screen never claims a task finished when it did not. Nothing is drawn when
/// the cursor was never placed.
pub(super) fn farewell(at: Option<&Point>, completed: bool) {
    let Some(at) = at else {
        return;
    };
    let Ok((screen, fps, reduce_motion)) = bridge::screen_at(at) else {
        return;
    };
    let fade = if reduce_motion {
        0.2
    } else {
        FAREWELL_FADE_SECONDS
    };
    if !completed {
        bridge::fade_out(fade);
        return;
    }
    let Ok(note) = still(at, Some(CURSOR_FAREWELL)) else {
        return;
    };
    let pace = RenderPace {
        fps,
        reduce_motion,
        announce: false,
        fade_in: false,
    };
    if bridge::run(&[CursorPose::still(at.clone())], pace, &note, &screen).is_err() {
        return;
    }
    bridge::hold(Duration::from_millis(CURSOR_FAREWELL_HOLD_MS));
    bridge::fade_out(fade);
}

fn landing_file() -> Option<PathBuf> {
    Some(
        agent_desktop_core::session::agent_desktop_dir()
            .ok()?
            .join(LANDING_FILE),
    )
}

/// Remembers where the cursor last landed. Best effort: a cursor that cannot
/// remember starts the next time from beside its target, as before.
pub(super) fn remember(at: Option<&Point>) {
    let (Some(at), Some(path)) = (at, landing_file()) else {
        return;
    };
    let Ok(text) = serde_json::to_vec(at) else {
        return;
    };
    let staged = path.with_extension("json.tmp");
    if std::fs::write(&staged, text).is_ok() {
        let _ = std::fs::rename(&staged, &path);
    }
}

/// Where the cursor last landed, when that point is still on a screen.
pub(super) fn recall() -> Option<Point> {
    let text = std::fs::read(landing_file()?).ok()?;
    let point: Point = serde_json::from_slice(&text).ok()?;
    bridge::screen_at(&point).ok().map(|_| point)
}

#[cfg(test)]
#[path = "session_flow_tests.rs"]
mod tests;
