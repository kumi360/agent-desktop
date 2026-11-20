use super::*;
use agent_desktop_core::{CursorOverlayConfig, CursorPhase, Point};

fn instruction(narration: Option<&str>) -> CursorOverlayInstruction {
    let config = CursorOverlayConfig::enabled(None, 12).unwrap();
    CursorOverlayInstruction::new(Point { x: 10.0, y: 10.0 }, &config, false)
        .unwrap()
        .with_narration(narration)
}

#[test]
fn a_narration_fades_once_its_hold_has_passed() {
    let start = Instant::now();
    let hold = Duration::from_millis(CURSOR_NARRATION_HOLD_MS);
    assert!(!due(Some(start), start));
    assert!(!due(Some(start), start + hold - Duration::from_millis(1)));
    assert!(due(Some(start), start + hold));
    assert!(!due(None, start + hold * 4));
}

#[test]
fn a_narrated_cue_restarts_the_hold_and_a_plain_travel_ends_it() {
    let earlier = Instant::now();
    let now = earlier + Duration::from_millis(500);
    let narrated = instruction(Some("Restoring TextEdit"));
    assert_eq!(after(&narrated, Some(earlier), now), Some(now));
    assert_eq!(after(&instruction(None), Some(earlier), now), None);
    assert_eq!(after(&instruction(Some("   ")), Some(earlier), now), None);
}

#[test]
fn a_caret_keeps_the_narration_it_was_typed_under() {
    let earlier = Instant::now();
    let caret = instruction(None).with_phase(CursorPhase::Caret);
    assert_eq!(
        after(&caret, Some(earlier), earlier + Duration::from_millis(50)),
        Some(earlier)
    );
}
