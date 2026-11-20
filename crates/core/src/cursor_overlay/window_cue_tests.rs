use super::CursorOverlayConfig;

#[test]
fn window_cue_points_at_title_bar_and_narrates() {
    let window = crate::Rect {
        x: 100.0,
        y: 50.0,
        width: 400.0,
        height: 300.0,
    };
    let point = super::window_cue_point(&window);
    assert_eq!((point.x, point.y), (300.0, 64.0));
    let tiny = crate::Rect {
        x: 0.0,
        y: 0.0,
        width: 10.0,
        height: 10.0,
    };
    assert_eq!(super::window_cue_point(&tiny).y, 5.0);
    let config = CursorOverlayConfig::enabled(None, 6).unwrap();
    let instruction = super::CursorOverlayInstruction::new(point.clone(), &config, false)
        .unwrap()
        .with_narration(Some("Opening TextEdit"));
    assert_eq!(instruction.label(), Some("Opening TextEdit"));
    let long = "x".repeat(80);
    let clipped = super::CursorOverlayInstruction::new(point.clone(), &config, false)
        .unwrap()
        .with_narration(Some(&long));
    assert_eq!(clipped.label().unwrap().chars().count(), 49);
    let kept = super::CursorOverlayInstruction::new(point.clone(), &config, false)
        .unwrap()
        .with_narration(Some("  "));
    assert_eq!(kept.label(), None);
}
