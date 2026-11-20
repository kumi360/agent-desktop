use super::narration;
use crate::{Action, Direction, RefEntry};

fn entry(name: Option<&str>, app: Option<&str>) -> RefEntry {
    let mut entry = crate::adapter::minimal_ref_entry("button", name);
    entry.source.source_app = app.map(str::to_owned);
    entry
}

#[test]
fn every_step_says_its_verb_and_target_before_it_acts() {
    let bold = entry(Some("Bold"), Some("TextEdit"));
    assert_eq!(narration(&bold, &Action::Click), "Clicking Bold");
    assert_eq!(
        narration(&bold, &Action::DoubleClick),
        "Double-clicking Bold"
    );
    assert_eq!(narration(&bold, &Action::Check), "Turning on Bold");
    assert_eq!(
        narration(&bold, &Action::Scroll(Direction::Down, 3)),
        "Scrolling Bold down"
    );
}

#[test]
fn typing_names_the_field_or_falls_back_to_the_app() {
    let unnamed = entry(None, Some("TextEdit"));
    assert_eq!(
        narration(&unnamed, &Action::TypeText("hi".into())),
        "Typing into TextEdit"
    );
    let bare = entry(None, None);
    assert_eq!(narration(&bare, &Action::TypeText("hi".into())), "Typing");
    assert_eq!(narration(&bare, &Action::Click), "Clicking");
}

#[test]
fn long_names_are_clipped_so_the_label_stays_one_calm_line() {
    let long = entry(Some(&"x".repeat(80)), None);
    let said = narration(&long, &Action::Click);
    assert!(said.chars().count() <= "Clicking ".len() + 33, "{said}");
    assert!(said.ends_with('…'));
}
