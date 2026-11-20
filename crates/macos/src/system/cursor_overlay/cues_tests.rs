use super::*;
use agent_desktop_core::{CursorOverlayConfig, Point};

fn instruction(narration: Option<&str>) -> CursorOverlayInstruction {
    let config = CursorOverlayConfig::enabled(None, 12).unwrap();
    CursorOverlayInstruction::new(Point { x: 10.0, y: 10.0 }, &config, false)
        .unwrap()
        .with_narration(narration)
}

#[test]
fn an_ambient_narration_fades_once_its_hold_has_passed() {
    let start = Instant::now();
    let hold = Duration::from_millis(CURSOR_NARRATION_HOLD_MS);
    let mut cues = Cues::default();
    cues.drawn(&instruction(Some("Restoring TextEdit")), start);
    assert!(!cues.narration_due(start + hold - Duration::from_millis(1)));
    assert!(cues.narration_due(start + hold));
    cues.drawn(&instruction(None), start);
    assert!(!cues.narration_due(start + hold * 4));
}

#[test]
fn the_effect_keeps_the_narration_of_the_step_it_finishes() {
    let mut cues = Cues::default();
    cues.drawn(&instruction(Some("Clicking Bold")), Instant::now());
    let effect = instruction(None).with_phase(CursorPhase::Effect);
    let carried = cues
        .carry(&effect)
        .expect("the effect carries the narration");
    assert_eq!(carried.label(), Some("Clicking Bold"));
    assert_eq!(carried.phase(), CursorPhase::Effect);
    assert!(cues.carry(&instruction(None)).is_none());
}

#[test]
fn the_spinner_waits_longer_while_a_step_may_still_be_running() {
    let mut cues = Cues::default();
    let think = Duration::from_millis(CURSOR_THINK_DELAY_MS);
    cues.drawn(&instruction(Some("Typing into Notes")), Instant::now());
    assert!(!cues.think_due(think));
    assert!(cues.think_due(Duration::from_millis(THINK_DELAY_MID_STEP_MS)));
    cues.drawn(
        &instruction(None).with_phase(CursorPhase::Effect),
        Instant::now(),
    );
    assert!(!cues.think_due(think - Duration::from_millis(1)));
    assert!(cues.think_due(think));
    cues.start_thinking();
    assert!(!cues.think_due(think * 10), "the spinner is shown once");
    assert!(cues.stop_thinking());
    assert!(!cues.stop_thinking());
}

#[test]
fn after_the_opening_note_the_spinner_follows_quickly() {
    let mut cues = Cues::default();
    cues.opened();
    assert!(cues.think_due(Duration::from_millis(CURSOR_THINK_DELAY_MS)));
}
