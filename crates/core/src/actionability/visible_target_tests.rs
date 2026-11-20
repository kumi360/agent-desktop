use super::*;
use crate::Action;
use crate::adapter::hit_test::HitTestResult;
use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};
use std::sync::atomic::{AtomicUsize, Ordering};

const VISIBLE: Rect = Rect {
    x: 100.0,
    y: 200.0,
    width: 40.0,
    height: 10.0,
};

struct Fake {
    visibility: TargetVisibility,
    hit: HitTestResult,
    hit_error: Option<ErrorCode>,
    nonspatial: bool,
    hit_tests: AtomicUsize,
}

impl Fake {
    fn new(visibility: TargetVisibility, hit: HitTestResult) -> Self {
        Self {
            visibility,
            hit,
            hit_error: None,
            nonspatial: false,
            hit_tests: AtomicUsize::new(0),
        }
    }
}

impl ObservationOps for Fake {
    fn is_nonspatial_target_with_deadline(
        &self,
        _handle: &NativeHandle,
        _deadline: crate::Deadline,
    ) -> Result<bool, AdapterError> {
        Ok(self.nonspatial)
    }

    fn target_visibility(
        &self,
        _handle: &NativeHandle,
        _deadline: crate::Deadline,
    ) -> Result<TargetVisibility, AdapterError> {
        Ok(self.visibility.clone())
    }

    fn hit_test(
        &self,
        _handle: &NativeHandle,
        _point: Point,
        _deadline: crate::Deadline,
    ) -> Result<HitTestResult, AdapterError> {
        self.hit_tests.fetch_add(1, Ordering::SeqCst);
        match &self.hit_error {
            Some(code) => Err(AdapterError::new(code.clone(), "hit test failed")),
            None => Ok(self.hit.clone()),
        }
    }
}
impl ActionOps for Fake {}
impl InputOps for Fake {}
impl SystemOps for Fake {}

fn run(
    fake: &Fake,
    action: Action,
    delivery: PointerDelivery,
    verified: Option<&Point>,
    presents: bool,
) -> Result<Option<VisibleTarget>, AdapterError> {
    let handle = NativeHandle::null();
    let request = ActionRequest::headless(action);
    visible_target(VisibleTargetInput {
        handle: &handle,
        adapter: fake,
        request: &request,
        pointer_delivery: delivery,
        verified_point: verified,
        presents,
        deadline: crate::Deadline::standard().unwrap(),
    })
}

fn visible() -> TargetVisibility {
    TargetVisibility::Visible { bounds: VISIBLE }
}

#[test]
fn only_pointer_actions_on_spatial_targets_are_gated() {
    let fake = Fake::new(
        TargetVisibility::Hidden {
            reason: "minimized".into(),
        },
        HitTestResult::ReachesTarget,
    );
    assert_eq!(
        run(
            &fake,
            Action::SetFocus,
            PointerDelivery::Semantic,
            None,
            true
        )
        .unwrap(),
        None
    );
    assert_eq!(
        run(
            &fake,
            Action::Click,
            PointerDelivery::NotApplicable,
            None,
            true
        )
        .unwrap(),
        None
    );
    let mut menu = Fake::new(fake.visibility.clone(), HitTestResult::ReachesTarget);
    menu.nonspatial = true;
    assert_eq!(
        run(&menu, Action::Click, PointerDelivery::Semantic, None, true).unwrap(),
        None,
        "closed-menu items keep semantic delivery"
    );
}

#[test]
fn a_hidden_pointer_target_fails_the_visible_check() {
    let fake = Fake::new(
        TargetVisibility::Hidden {
            reason: "its window is minimized".into(),
        },
        HitTestResult::ReachesTarget,
    );
    for delivery in [PointerDelivery::Semantic, PointerDelivery::Physical] {
        let error = run(&fake, Action::DoubleClick, delivery, None, false).unwrap_err();
        assert_eq!(error.code, ErrorCode::ActionFailed);
        assert!(error.message.contains("its window is minimized"));
        assert_eq!(error.disposition, crate::DeliverySemantics::not_delivered());
        assert!(crate::ref_action::ref_action_wait_evidence::failed_check(
            &error, "visible"
        ));
    }
}

#[test]
fn unknown_visibility_keeps_the_preflight() {
    let fake = Fake::new(TargetVisibility::Unknown, HitTestResult::ReachesTarget);
    assert_eq!(
        run(&fake, Action::Click, PointerDelivery::Semantic, None, true).unwrap(),
        None
    );
}

#[test]
fn a_physical_click_without_a_verified_point_aims_at_the_visible_centre() {
    let fake = Fake::new(visible(), HitTestResult::Unknown);
    let gate = run(&fake, Action::Click, PointerDelivery::Physical, None, true)
        .unwrap()
        .unwrap();
    assert_eq!(gate.point, Some(Point { x: 120.0, y: 205.0 }));
    assert_eq!(gate.bounds, Some(VISIBLE));
    assert_eq!(gate.verdict, "visible_unverified");
    let verified = Point { x: 105.0, y: 202.0 };
    let gate = run(
        &fake,
        Action::Click,
        PointerDelivery::Physical,
        Some(&verified),
        true,
    )
    .unwrap()
    .unwrap();
    assert_eq!(gate.point, Some(verified));
    assert_eq!(gate.verdict, "hit");
}

#[test]
fn semantic_delivery_hit_tests_only_when_the_cursor_is_drawn() {
    let fake = Fake::new(visible(), HitTestResult::ReachesTarget);
    let gate = run(&fake, Action::Click, PointerDelivery::Semantic, None, false)
        .unwrap()
        .unwrap();
    assert_eq!(fake.hit_tests.load(Ordering::SeqCst), 0);
    assert_eq!(gate.verdict, "visible");
    let gate = run(&fake, Action::Click, PointerDelivery::Semantic, None, true)
        .unwrap()
        .unwrap();
    assert!(fake.hit_tests.load(Ordering::SeqCst) > 0);
    assert_eq!(gate.verdict, "hit");
    assert_eq!(gate.bounds, Some(VISIBLE));
}

#[test]
fn an_occluded_semantic_target_is_not_drawn() {
    let fake = Fake::new(
        visible(),
        HitTestResult::InterceptedBy {
            role: Some("window".into()),
            name: None,
            bounds: None,
        },
    );
    let gate = run(&fake, Action::Click, PointerDelivery::Semantic, None, true)
        .unwrap()
        .unwrap();
    assert_eq!(gate.verdict, "occluded");
    assert_eq!(gate.point, None);
    assert_eq!(gate.bounds, None);
    assert!(gate.occluder.is_some());
}

#[test]
fn an_unproven_semantic_hit_test_draws_the_visible_centre_and_a_timeout_ends_it() {
    let mut fake = Fake::new(visible(), HitTestResult::ReachesTarget);
    fake.hit_error = Some(ErrorCode::ActionFailed);
    let gate = run(&fake, Action::Click, PointerDelivery::Semantic, None, true)
        .unwrap()
        .unwrap();
    assert_eq!(gate.verdict, "visible_unverified");
    assert_eq!(gate.bounds, Some(VISIBLE));
    assert_eq!(gate.point, center(VISIBLE));
    fake.hit_error = Some(ErrorCode::Timeout);
    let error = run(&fake, Action::Click, PointerDelivery::Semantic, None, true).unwrap_err();
    assert_eq!(error.code, ErrorCode::Timeout);
}
