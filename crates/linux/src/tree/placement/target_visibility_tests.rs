use super::*;
use crate::system::process::toolkit_id::ToolkitId;
use crate::system::x11::window::window_x11_state::WindowX11State;
use crate::tree::hit_test::tests::rect;
use crate::tree::surfaces::window_tie::WindowTie;

fn window(hidden: bool) -> WindowX11State {
    WindowX11State {
        client_id: 7,
        hidden,
        client_origin: (100, 200),
        shadow: (0, 0, 0, 0),
    }
}

/// A list row inside a 420x420 window at screen (100, 200) whose scroll area
/// shows window rows 40..240.
fn placed(node: Extents) -> Placement {
    Placement {
        node: Some(node),
        frame: Some(rect(0, 0, 420, 420)),
        container: Some(rect(0, 40, 420, 200)),
        frame_iconified: Some(false),
        x11: Some(window(false)),
        displays: Some(vec![rect(0, 0, 1920, 1080)]),
        ..Placement::default()
    }
}

fn visible(x: i32, y: i32, width: i32, height: i32) -> TargetVisibility {
    TargetVisibility::Visible {
        bounds: Rect::from(rect(x, y, width, height)),
    }
}

#[test]
fn a_row_inside_its_scroll_area_is_visible_in_screen_coordinates() {
    assert_eq!(
        target_visibility(&placed(rect(10, 60, 200, 20))),
        visible(110, 260, 200, 20)
    );
}

#[test]
fn a_row_half_scrolled_out_is_cut_to_its_visible_part() {
    assert_eq!(
        target_visibility(&placed(rect(10, 230, 200, 20))),
        visible(110, 430, 200, 10)
    );
}

#[test]
fn a_row_scrolled_out_of_its_scroll_area_is_hidden() {
    for y in [245, 300, 5000] {
        assert!(
            matches!(
                target_visibility(&placed(rect(10, y, 200, 20))),
                TargetVisibility::Hidden { .. }
            ),
            "row at window y {y} must be hidden"
        );
    }
}

#[test]
fn a_minimized_or_unmapped_window_hides_its_targets() {
    let mut iconified = placed(rect(10, 60, 200, 20));
    iconified.frame_iconified = Some(true);
    assert!(matches!(
        target_visibility(&iconified),
        TargetVisibility::Hidden { .. }
    ));
    let mut unmapped = placed(rect(10, 60, 200, 20));
    unmapped.x11 = Some(window(true));
    assert!(matches!(
        target_visibility(&unmapped),
        TargetVisibility::Hidden { .. }
    ));
}

#[test]
fn display_edges_cut_the_target_and_no_display_hides_it() {
    let mut edge = placed(rect(10, 60, 200, 20));
    edge.x11 = Some(WindowX11State {
        client_origin: (1800, 200),
        ..window(false)
    });
    assert_eq!(target_visibility(&edge), visible(1810, 260, 110, 20));
    edge.displays = Some(vec![rect(0, 0, 1000, 1000)]);
    assert!(matches!(
        target_visibility(&edge),
        TargetVisibility::Hidden { .. }
    ));
    edge.displays = None;
    assert_eq!(target_visibility(&edge), visible(1810, 260, 200, 20));
}

#[test]
fn missing_or_ambiguous_geometry_stays_unknown() {
    let base = placed(rect(10, 60, 200, 20));
    let cases = [
        Placement {
            node: None,
            ..base.clone()
        },
        Placement {
            frame: None,
            ..base.clone()
        },
        Placement {
            node: Some(rect(i32::MIN, i32::MIN, 200, 20)),
            ..base.clone()
        },
        Placement {
            container: Some(rect(0, 40, 0, 0)),
            ..base.clone()
        },
        Placement {
            x11: None,
            ..base.clone()
        },
        Placement {
            wayland_unplaced: true,
            ..base.clone()
        },
        Placement {
            tie: Some(WindowTie { hidden: None }),
            ..base.clone()
        },
    ];
    for case in cases {
        assert_eq!(
            target_visibility(&case),
            TargetVisibility::Unknown,
            "{case:?}"
        );
    }
}

#[test]
fn the_screen_origin_matches_the_physical_aim_rule() {
    let mut gtk4 = placed(rect(10, 60, 200, 20));
    gtk4.x11 = Some(WindowX11State {
        shadow: (12, 8, 12, 16),
        ..window(false)
    });
    assert_eq!(screen_origin(&gtk4), Some((112, 208)));
    gtk4.toolkit = ToolkitId {
        name: Some("GTK".to_owned()),
        version: Some("3.24.41".to_owned()),
    };
    if reports_global_position(&gtk4.toolkit) {
        assert_eq!(screen_origin(&gtk4), Some((100, 200)));
    }
    gtk4.popup_origin = Some((5, 6));
    assert_eq!(screen_origin(&gtk4), Some((5, 6)));
}
