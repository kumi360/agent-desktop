/// How long a platform renderer may take to report that the cursor reached its
/// destination before the action dispatches anyway.
pub const CURSOR_ARRIVAL_TIMEOUT_MS: u64 = 1_400;

/// How long a step's label is on screen, with the cursor still, before the
/// cursor sets off toward the target, so a person watching reads what is
/// about to happen before it happens.
pub const CURSOR_ANNOUNCE_LEAD_MS: u64 = 450;

/// The largest cursor travel plus dwell. With the announcement lead it still
/// leaves room for the socket round trip and frame pacing inside the arrival
/// timeout.
pub const CURSOR_MOTION_BUDGET_MS: u64 = 700;

const _: () =
    assert!(CURSOR_ANNOUNCE_LEAD_MS + CURSOR_MOTION_BUDGET_MS + 200 <= CURSOR_ARRIVAL_TIMEOUT_MS);

/// How long a session's cursor waits with nothing to do before it shows the
/// small thinking spinner in its label spot.
pub const CURSOR_THINK_DELAY_MS: u64 = 700;

/// How long a session's opening note, "Starting session: ...", stays on
/// screen at the centre of the main display before the first step.
pub const CURSOR_INTRO_HOLD_MS: u64 = 1_500;

/// How long the closing note stays beside the cursor before it fades out.
pub const CURSOR_FAREWELL_HOLD_MS: u64 = 1_500;

/// What the cursor says when its session ends.
pub const CURSOR_FAREWELL: &str = "Completed, ending session";

/// How long the overlay stays visible with no instruction before it rests.
pub const CURSOR_IDLE_REST_MS: u64 = 6_000;

/// How long an action's narration, such as "Restoring TextEdit", stays on
/// the label once drawn. It then fades, so a caption never outlives its
/// action and never describes the next one.
pub const CURSOR_NARRATION_HOLD_MS: u64 = 2_400;

/// How long the outline around a clicked element stays on screen.
pub const CURSOR_HIGHLIGHT_HOLD_MS: u64 = 900;

/// How long the overlay takes to fade out once it rests.
///
/// The macOS bridge steps its window alpha down in thirteen 12ms hops, which
/// is where this number comes from. It is stated here so both renderers can
/// name one value rather than each carrying its own; the bridge still
/// hardcodes its own copy in Objective-C and consuming this is a promotion
/// item, not a change either platform can make alone.
pub const CURSOR_REST_FADE_MS: u64 = 156;

/// How long the label card takes to appear once its text changes.
///
/// macOS reveals it with a 0.18s opacity fade and an ease-out scale pop, so a card that
/// arrives does not simply blink into place. The same caveat applies: the
/// bridge holds its own copy for now.
pub const CURSOR_LABEL_REVEAL_MS: u64 = 180;
