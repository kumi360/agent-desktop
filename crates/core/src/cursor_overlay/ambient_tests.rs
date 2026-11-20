use super::*;
use crate::refs::refs_test_support::HomeGuard;

#[test]
fn the_ambient_cursor_is_on_and_purple_by_default() {
    let _guard = HomeGuard::new();
    let config = ambient_config();
    assert!(config.is_enabled());
    assert!(ambient_active());
    let json = serde_json::to_value(&config).unwrap();
    assert_eq!(json["style"]["accent"], "#9B5CFF");
}

#[test]
fn off_values_are_recognised_case_insensitively() {
    for value in ["off", "OFF", " 0 ", "false", "No", "none", "disabled"] {
        assert!(env_value_is_off(value), "{value}");
    }
    for value in ["on", "1", "", "purple"] {
        assert!(!env_value_is_off(value), "{value}");
    }
}

#[test]
fn a_saved_disable_turns_the_ambient_cursor_off_and_enable_restores_it() {
    let _guard = HomeGuard::new();
    save_ambient_config(&CursorOverlayConfig::default()).unwrap();
    assert!(ambient_config().is_disabled());
    assert!(!ambient_active());
    let labelled = CursorOverlayConfig::enabled(Some("Nimbus".into()), 6).unwrap();
    save_ambient_config(&labelled).unwrap();
    assert_eq!(ambient_config().label(), Some("Nimbus"));
    assert!(ambient_active());
}

#[test]
fn the_ambient_id_is_a_valid_session_key_that_generated_ids_never_take() {
    crate::context::validate_session_id(AMBIENT_CURSOR_ID).unwrap();
    assert!(!crate::session::new_session_id().starts_with('_'));
}
