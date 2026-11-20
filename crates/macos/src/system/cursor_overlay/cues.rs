//! What the cursor's label spot shows between steps. A step's narration,
//! such as "Clicking Bold", stays through its effect; once the agent has
//! been quiet a moment the label gives way to a small thinking spinner, so
//! a person watching can tell the agent is working out its next step. An
//! ambient cursor (no session) has no agent to wait for: its narration just
//! fades after a while.

use agent_desktop_core::{
    CURSOR_NARRATION_HOLD_MS, CURSOR_THINK_DELAY_MS, CursorOverlayInstruction, CursorPhase,
};
use std::time::{Duration, Instant};

/// How long the cursor waits after a travel with no effect yet before it
/// shows the spinner: the action itself may still be running, such as a long
/// typing burst, and the spinner must not cover a step in progress.
const THINK_DELAY_MID_STEP_MS: u64 = 2_500;

#[derive(Debug, Default)]
pub(super) struct Cues {
    narrated_at: Option<Instant>,
    label: Option<String>,
    settled: bool,
    thinking: bool,
}

impl Cues {
    /// An effect sent with no label of its own keeps the narration of the
    /// step it finishes on screen, so "Clicking Bold" stays while the click
    /// lands instead of blinking away.
    pub(super) fn carry(
        &self,
        instruction: &CursorOverlayInstruction,
    ) -> Option<CursorOverlayInstruction> {
        let label = self.label.as_deref()?;
        (instruction.label().is_none() && instruction.phase() == CursorPhase::Effect)
            .then(|| instruction.clone().with_narration(Some(label)))
    }

    pub(super) fn drawn(&mut self, instruction: &CursorOverlayInstruction, now: Instant) {
        let phase = instruction.phase();
        let caret = matches!(phase, CursorPhase::Caret | CursorPhase::CaretFollow);
        if instruction.is_narrated() {
            self.narrated_at = Some(now);
            self.label = instruction.label().map(str::to_owned);
        } else if !caret {
            self.narrated_at = None;
            self.label = None;
        }
        self.settled = caret || phase == CursorPhase::Effect;
        self.thinking = false;
    }

    /// The opening note was shown and the session is waiting for its first
    /// step.
    pub(super) fn opened(&mut self) {
        self.narrated_at = None;
        self.label = None;
        self.settled = true;
        self.thinking = false;
    }

    pub(super) fn think_due(&self, quiet_for: Duration) -> bool {
        let delay = if self.settled {
            CURSOR_THINK_DELAY_MS
        } else {
            THINK_DELAY_MID_STEP_MS
        };
        !self.thinking && quiet_for >= Duration::from_millis(delay)
    }

    pub(super) fn start_thinking(&mut self) {
        self.thinking = true;
        self.narrated_at = None;
        self.label = None;
    }

    pub(super) fn stop_thinking(&mut self) -> bool {
        std::mem::take(&mut self.thinking)
    }

    /// For an ambient cursor: whether its narration has had time to settle
    /// and should fade, so it never captions the next, different action.
    pub(super) fn narration_due(&self, now: Instant) -> bool {
        self.narrated_at.is_some_and(|at| {
            now.saturating_duration_since(at) >= Duration::from_millis(CURSOR_NARRATION_HOLD_MS)
        })
    }

    pub(super) fn narration_faded(&mut self) {
        self.narrated_at = None;
        self.label = None;
    }
}

#[cfg(test)]
#[path = "cues_tests.rs"]
mod tests;
