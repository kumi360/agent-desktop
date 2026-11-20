use super::ensure_reachable;
use agent_desktop_core::Rect;

const MAIN: Rect = Rect {
    x: 0.0,
    y: 0.0,
    width: 2560.0,
    height: 1440.0,
};

#[test]
fn a_spot_on_the_display_is_allowed() {
    assert!(ensure_reachable(700.0, 160.0, &[MAIN]).is_ok());
    assert!(ensure_reachable(0.0, 0.0, &[MAIN]).is_ok());
}

#[test]
fn a_spot_past_every_display_is_refused_and_names_the_displays() {
    let error = ensure_reachable(3000.0, 200.0, &[MAIN]).unwrap_err();
    assert!(error.message.contains("off every display"));
    assert!(
        error
            .suggestion
            .as_deref()
            .unwrap_or("")
            .contains("2560x1440 at (0, 0)")
    );
    assert!(ensure_reachable(700.0, 1500.0, &[MAIN]).is_err());
    assert!(ensure_reachable(700.0, -200.0, &[MAIN]).is_err());
}

#[test]
fn a_second_display_counts() {
    let right = Rect {
        x: 2560.0,
        y: 0.0,
        width: 1920.0,
        height: 1080.0,
    };
    assert!(ensure_reachable(3000.0, 200.0, &[MAIN, right]).is_ok());
}

#[test]
fn no_known_display_does_not_block_the_move() {
    assert!(ensure_reachable(3000.0, 200.0, &[]).is_ok());
}
