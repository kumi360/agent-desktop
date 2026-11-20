use super::{CursorOverlayConfig, CursorOverlayInstruction, CursorPhase};
use crate::Point;

fn instruction(x: f64, phase: CursorPhase) -> CursorOverlayInstruction {
    let config = CursorOverlayConfig::enabled(None, 6).expect("valid config");
    CursorOverlayInstruction::new(Point { x, y: 50.0 }, &config, false)
        .expect("valid instruction")
        .with_phase(phase)
}

#[test]
fn a_travel_somewhere_else_departs_and_one_that_stays_put_does_not() {
    let at = Point { x: 100.0, y: 50.0 };

    assert!(instruction(140.0, CursorPhase::Travel).departs_from(Some(&at)));
    assert!(!instruction(101.0, CursorPhase::Travel).departs_from(Some(&at)));
    assert!(instruction(101.0, CursorPhase::Travel).departs_from(None));
}

#[test]
fn only_travels_and_drags_depart() {
    let at = Point { x: 100.0, y: 50.0 };
    for phase in [CursorPhase::Effect, CursorPhase::Caret] {
        assert!(
            !instruction(400.0, phase).departs_from(Some(&at)),
            "{phase:?}"
        );
    }
    let drag = instruction(400.0, CursorPhase::Drag).with_drag_from(Some(at.clone()));
    assert!(
        !drag.departs_from(Some(&at)),
        "a drag sets off from its origin, where the cursor already is"
    );
}

#[test]
fn only_a_narration_marks_the_label_as_fading() {
    let plain = instruction(10.0, CursorPhase::Travel);
    assert!(!plain.is_narrated());
    let wire = serde_json::to_value(&plain).expect("serializes");
    assert!(
        wire.get("narrated").is_none(),
        "plain instructions stay byte-identical"
    );

    let narrated = plain.with_narration(Some("Restoring TextEdit"));
    assert!(narrated.is_narrated());
    let back: CursorOverlayInstruction =
        serde_json::from_value(serde_json::to_value(&narrated).expect("serializes"))
            .expect("round trips");
    assert!(back.is_narrated());
    assert_eq!(back.label(), Some("Restoring TextEdit"));
}

#[test]
fn each_agent_names_itself_before_what_it_is_doing() {
    let narrated = instruction(10.0, CursorPhase::Travel).with_narration(Some("Clicking 5"));

    assert_eq!(
        narrated.clone().named_for(Some("calc")).label(),
        Some("calc · Clicking 5")
    );
    assert_eq!(narrated.clone().named_for(None).label(), Some("Clicking 5"));
    assert_eq!(
        narrated
            .named_for(Some("calc"))
            .named_for(Some("calc"))
            .label(),
        Some("calc · Clicking 5")
    );
}

#[test]
fn a_label_that_is_not_narration_is_left_alone() {
    let quiet = instruction(10.0, CursorPhase::Travel);

    assert_eq!(quiet.clone().named_for(Some("calc")).label(), quiet.label());
}

#[test]
fn a_note_is_kept_whole_and_blank_keeps_the_label() {
    let note = "Starting session: one two three four five six seven eight nine ten eleven twelve";
    let held = instruction(10.0, CursorPhase::Travel).with_note(Some(note));
    assert_eq!(held.label(), Some(note));
    assert!(!held.is_narrated());
    assert_eq!(held.clone().with_note(Some("  ")).label(), Some(note));
}
