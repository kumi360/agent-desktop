use super::{already_there, shows_pointer};
use crate::Action;
use crate::actionability::PointerDelivery;

#[test]
fn pressable_controls_show_the_pointer_however_they_are_reached() {
    for role in [
        "button",
        "link",
        "checkbox",
        "tab",
        "menuitem",
        "switch",
        "disclosure",
    ] {
        for action in [
            Action::Click,
            Action::ScrollTo,
            Action::SetFocus,
            Action::Toggle,
        ] {
            assert!(shows_pointer(role, &action), "{role} {action:?}");
        }
    }
}

#[test]
fn text_entry_keeps_the_arrow_even_when_clicked() {
    for role in ["textfield", "combobox", "datefield"] {
        for action in [Action::Click, Action::DoubleClick, Action::SetFocus] {
            assert!(!shows_pointer(role, &action), "{role} {action:?}");
        }
    }
}

#[test]
fn other_elements_show_the_pointer_only_when_clicked() {
    for role in ["group", "cell", "statictext", "unknown", "row"] {
        assert!(shows_pointer(role, &Action::Click), "{role}");
        assert!(shows_pointer(role, &Action::RightClick), "{role}");
        assert!(!shows_pointer(role, &Action::ScrollTo), "{role}");
        assert!(
            !shows_pointer(role, &Action::SetValue("x".into())),
            "{role}"
        );
    }
}

#[path = "presentation_lease_tests.rs"]
mod lease;

#[path = "presentation_shared_cursor_tests.rs"]
mod shared_cursor;

#[path = "presentation_caret_tests.rs"]
mod caret;

fn preflight(delivery: PointerDelivery) -> super::ActionabilityPreflight {
    super::ActionabilityPreflight {
        verified_point: None,
        presentation_point: Some(crate::Point { x: 200.0, y: 900.0 }),
        presentation_bounds: Some(crate::Rect {
            x: 100.0,
            y: 100.0,
            width: 200.0,
            height: 1000.0,
        }),
        pointer_delivery: delivery,
    }
}

#[test]
fn a_cursor_already_on_a_long_field_does_not_glide_again() {
    let planned = crate::Point { x: 200.0, y: 900.0 };
    let semantic = preflight(PointerDelivery::Semantic);
    assert!(already_there(&semantic, &planned, (200.0, 300.0)));
    assert!(already_there(&semantic, &planned, (200.5, 900.0)));
    assert!(!already_there(&semantic, &planned, (500.0, 300.0)));
}

#[test]
fn a_physical_click_still_glides_to_its_verified_point() {
    let planned = crate::Point { x: 200.0, y: 900.0 };
    let physical = preflight(PointerDelivery::Physical);
    assert!(!already_there(&physical, &planned, (200.0, 300.0)));
}
