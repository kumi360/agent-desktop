use super::*;

#[test]
fn only_a_driven_session_opens_and_closes_with_a_note() {
    assert!(!is_session(AMBIENT_CURSOR_ID));
    assert!(is_session("0123456789abcdef"));
}

#[test]
fn a_still_cursor_carries_its_note() {
    let point = Point { x: 40.0, y: 50.0 };
    let note = still(&point, Some(CURSOR_FAREWELL)).unwrap();
    assert_eq!(note.label(), Some("Completed, ending session"));
    assert_eq!(note.destination(), &point);
    assert_eq!(still(&point, None).unwrap().label(), None);
}

#[test]
fn a_helper_cursor_never_starts_on_another_helpers_landing() {
    assert!(resumes_shared_landing(false, None));
    assert!(!resumes_shared_landing(false, Some("calc")));
    assert!(!resumes_shared_landing(true, None));
}
