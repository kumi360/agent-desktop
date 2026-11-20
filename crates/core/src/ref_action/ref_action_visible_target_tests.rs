//! Regression tests for phantom clicks: a click must never be aimed at, or
//! drawn around, a part of the screen where its element is not.

use crate::adapter::hit_test::{HitTestResult, TargetVisibility};
use crate::adapter::{ActionOps, InputOps, NativeHandle, ObservationOps, SystemOps};
use crate::ref_action::*;
use crate::{
    Action, ActionResult, AdapterError, CursorOverlayControl, ErrorCode, actions::capability,
};
use std::sync::Mutex;

const LIVE: crate::Rect = crate::Rect {
    x: 1.0,
    y: 1.0,
    width: 20.0,
    height: 20.0,
};

struct ScreenAdapter {
    visibility: TargetVisibility,
    hit: HitTestResult,
    presented: Mutex<Vec<CursorOverlayControl>>,
    dispatched: Mutex<Vec<Option<crate::Point>>>,
}

impl ScreenAdapter {
    fn new(visibility: TargetVisibility, hit: HitTestResult) -> Self {
        Self {
            visibility,
            hit,
            presented: Mutex::new(Vec::new()),
            dispatched: Mutex::new(Vec::new()),
        }
    }

    fn drawn(&self) -> Vec<(crate::Point, Option<crate::Rect>, bool)> {
        self.presented
            .lock()
            .unwrap()
            .iter()
            .filter_map(|control| control.instruction())
            .map(|instruction| {
                (
                    instruction.destination().clone(),
                    instruction.target().copied(),
                    instruction.is_click(),
                )
            })
            .collect()
    }
}

impl ObservationOps for ScreenAdapter {
    fn resolve_element_strict(
        &self,
        _entry: &RefEntry,
        _deadline: crate::Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        Ok(NativeHandle::null())
    }

    fn target_visibility(
        &self,
        _handle: &NativeHandle,
        _deadline: crate::Deadline,
    ) -> Result<TargetVisibility, AdapterError> {
        Ok(self.visibility.clone())
    }

    crate::adapter::complete_live_observation!("button", "Save", [capability::CLICK]);
    crate::adapter::saved_ref_scope!("button", Some("Save"));
}

impl ActionOps for ScreenAdapter {
    fn execute_action(
        &self,
        _handle: &NativeHandle,
        request: ActionRequest,
        _lease: &crate::InteractionLease,
    ) -> Result<ActionResult, AdapterError> {
        self.dispatched
            .lock()
            .unwrap()
            .push(request.verified_point().cloned());
        Ok(ActionResult::delivered_unverified("click"))
    }
}

impl InputOps for ScreenAdapter {}

impl SystemOps for ScreenAdapter {
    crate::adapter::guarded_interaction_lease!();
    crate::adapter::exact_window_focus!();

    fn update_cursor_overlay(&self, control: &CursorOverlayControl) -> Result<(), AdapterError> {
        self.presented.lock().unwrap().push(control.clone());
        Ok(())
    }
}

/// `hit_test` is answered by the shared live-observation macro with
/// `ReachesTarget`; this wrapper lets a test choose the answer instead.
struct Occluding(ScreenAdapter);

impl ObservationOps for Occluding {
    fn resolve_element_strict(
        &self,
        entry: &RefEntry,
        deadline: crate::Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        self.0.resolve_element_strict(entry, deadline)
    }

    fn target_visibility(
        &self,
        handle: &NativeHandle,
        deadline: crate::Deadline,
    ) -> Result<TargetVisibility, AdapterError> {
        self.0.target_visibility(handle, deadline)
    }

    fn get_live_element(
        &self,
        handle: &NativeHandle,
        deadline: crate::Deadline,
    ) -> Result<crate::LiveElement, AdapterError> {
        self.0.get_live_element(handle, deadline)
    }

    fn get_live_state(
        &self,
        handle: &NativeHandle,
        deadline: crate::Deadline,
    ) -> Result<Option<crate::ElementState>, AdapterError> {
        self.0.get_live_state(handle, deadline)
    }

    fn get_element_bounds(
        &self,
        handle: &NativeHandle,
        deadline: crate::Deadline,
    ) -> Result<Option<crate::Rect>, AdapterError> {
        self.0.get_element_bounds(handle, deadline)
    }

    fn get_live_actions(
        &self,
        handle: &NativeHandle,
        deadline: crate::Deadline,
    ) -> Result<Option<Vec<String>>, AdapterError> {
        self.0.get_live_actions(handle, deadline)
    }

    fn hit_test(
        &self,
        _handle: &NativeHandle,
        _point: crate::Point,
        _deadline: crate::Deadline,
    ) -> Result<HitTestResult, AdapterError> {
        Ok(self.0.hit.clone())
    }

    crate::adapter::saved_ref_scope!("button", Some("Save"));
}

impl ActionOps for Occluding {
    fn execute_action(
        &self,
        handle: &NativeHandle,
        request: ActionRequest,
        lease: &crate::InteractionLease,
    ) -> Result<ActionResult, AdapterError> {
        self.0.execute_action(handle, request, lease)
    }
}

impl InputOps for Occluding {}

impl SystemOps for Occluding {
    crate::adapter::guarded_interaction_lease!();
    crate::adapter::exact_window_focus!();

    fn update_cursor_overlay(&self, control: &CursorOverlayControl) -> Result<(), AdapterError> {
        self.0.update_cursor_overlay(control)
    }
}

fn entry(stored: crate::Rect) -> RefEntry {
    let mut entry = crate::adapter::minimal_ref_entry("button", Some("Save"));
    entry.process = crate::RefProcess {
        pid: crate::ProcessId::new(1),
        process_instance: Some("test-instance".into()),
    };
    entry.identity.unique_at_capture = true;
    entry.geometry = crate::RefGeometry {
        bounds_space: None,
        bounds: Some(stored),
        bounds_hash: LIVE.bounds_hash(),
    };
    entry.capabilities.available_actions = vec![capability::CLICK.into()];
    entry.source.source_window_id = Some("w-1".into());
    entry
}

fn overlay() -> CommandContext {
    let config = crate::CursorOverlayConfig::enabled(None, 6).expect("valid config");
    CommandContext::default().with_cursor_overlay_session("test-session", config)
}

fn inside(rect: crate::Rect, point: &crate::Point) -> bool {
    point.x >= rect.x
        && point.y >= rect.y
        && point.x <= rect.x + rect.width
        && point.y <= rect.y + rect.height
}

fn click(
    adapter: &dyn crate::PlatformAdapter,
    context: &CommandContext,
) -> Result<ActionResult, AdapterError> {
    execute_entry_with_context(
        adapter,
        &entry(LIVE),
        ActionRequest::headless(Action::Click).with_timeout_ms(Some(1_200)),
        context,
    )
}

#[test]
fn a_click_on_a_hidden_target_is_refused_and_nothing_is_drawn() {
    for reason in [
        "its window is minimized",
        "it is clipped out of view by its AXScrollArea",
        "the element is outside every attached display",
    ] {
        let adapter = ScreenAdapter::new(
            TargetVisibility::Hidden {
                reason: reason.into(),
            },
            HitTestResult::ReachesTarget,
        );
        let error = click(&adapter, &overlay()).expect_err("hidden target is refused");
        assert_eq!(error.code, ErrorCode::ActionFailed, "{reason}");
        assert!(error.message.contains("not on screen"), "{}", error.message);
        assert!(
            crate::ref_action::ref_action_wait_evidence::failed_check(&error, "visible"),
            "the refusal is a failed visible check, which scroll recovery understands"
        );
        assert!(
            adapter.dispatched.lock().unwrap().is_empty(),
            "never dispatched"
        );
        assert!(
            adapter.drawn().iter().all(|(_, _, is_click)| !is_click),
            "no click effect or highlight for a hidden target"
        );
    }
}

#[test]
fn a_hidden_target_is_refused_with_the_overlay_off_too() {
    let adapter = ScreenAdapter::new(
        TargetVisibility::Hidden {
            reason: "its window is minimized".into(),
        },
        HitTestResult::ReachesTarget,
    );
    let error = click(&adapter, &CommandContext::default()).expect_err("refused");
    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert!(adapter.dispatched.lock().unwrap().is_empty());
}

#[test]
fn a_clipped_target_is_highlighted_only_where_it_is_visible() {
    let visible = crate::Rect {
        x: 1.0,
        y: 16.0,
        width: 20.0,
        height: 5.0,
    };
    let adapter = ScreenAdapter::new(
        TargetVisibility::Visible { bounds: visible },
        HitTestResult::ReachesTarget,
    );
    click(&adapter, &overlay()).expect("visible part is clicked");
    let drawn = adapter.drawn();
    let (point, target, _) = drawn
        .iter()
        .find(|(_, _, is_click)| *is_click)
        .expect("click effect drawn");
    assert_eq!(
        *target,
        Some(visible),
        "highlight is the visible part, not the full frame"
    );
    assert!(
        inside(visible, point),
        "ripple lands inside the visible part: {point:?}"
    );
    for (point, _, _) in &drawn {
        assert!(
            inside(visible, point),
            "every cursor move stays on the visible part: {point:?}"
        );
    }
}

#[test]
fn stale_snapshot_bounds_never_steer_the_cursor() {
    let adapter = ScreenAdapter::new(
        TargetVisibility::Visible { bounds: LIVE },
        HitTestResult::ReachesTarget,
    );
    let stale = crate::Rect {
        x: 900.0,
        y: 700.0,
        width: 40.0,
        height: 20.0,
    };
    execute_entry_with_context(
        &adapter,
        &entry(stale),
        ActionRequest::headless(Action::Click),
        &overlay(),
    )
    .expect("click succeeds");
    let drawn = adapter.drawn();
    assert!(!drawn.is_empty());
    for (point, target, _) in drawn {
        assert!(
            inside(LIVE, &point),
            "cursor went to stale snapshot place {point:?}"
        );
        if let Some(target) = target {
            assert_eq!(target, LIVE);
        }
    }
}

#[test]
fn an_occluded_semantic_click_is_delivered_but_never_drawn_over_the_occluder() {
    let adapter = Occluding(ScreenAdapter::new(
        TargetVisibility::Visible { bounds: LIVE },
        HitTestResult::InterceptedBy {
            role: Some("window".into()),
            name: Some("Other".into()),
            bounds: None,
        },
    ));
    click(&adapter, &overlay()).expect("headless delivery still works behind a window");
    assert_eq!(adapter.0.dispatched.lock().unwrap().len(), 1);
    assert!(
        adapter
            .0
            .drawn()
            .iter()
            .all(|(_, target, is_click)| !is_click && target.is_none()),
        "no ripple or highlight drawn over the window that covers the target: {:?}",
        adapter.0.drawn()
    );
}

#[test]
fn unknown_visibility_keeps_the_previous_presentation() {
    let adapter = ScreenAdapter::new(TargetVisibility::Unknown, HitTestResult::ReachesTarget);
    click(&adapter, &overlay()).expect("click succeeds");
    let drawn = adapter.drawn();
    let (_, target, _) = drawn
        .iter()
        .find(|(_, _, is_click)| *is_click)
        .expect("click effect drawn");
    assert_eq!(*target, Some(LIVE));
}
