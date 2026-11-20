//! The visible-target gate for pointer actions: a click may only aim at, and
//! the drawn cursor may only highlight, the part of an element that is on
//! screen now and, when it can be proven, the element the pointer would hit.

use super::{
    PointerDelivery, check_result::fail, receives_events::receives_events_check,
    report::ActionabilityReport, status::ActionabilityStatus,
};
use crate::{
    AdapterError, ErrorCode, Point, Rect,
    actions::action_request::ActionRequest,
    adapter::{NativeHandle, PlatformAdapter, hit_test::TargetVisibility},
};
use serde_json::json;

/// Where a pointer action aims and what the drawn cursor outlines. `point`
/// and `bounds` are `None` when the cursor must not be drawn at all.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct VisibleTarget {
    pub(crate) point: Option<Point>,
    pub(crate) bounds: Option<Rect>,
    pub(crate) verdict: &'static str,
    pub(crate) visibility: TargetVisibility,
    pub(crate) occluder: Option<serde_json::Value>,
}

/// The resolved element and the delivery its preflight chose. `presents`
/// says the cursor overlay will draw this action, which is the only case that
/// pays for hit-testing a semantic delivery.
pub(crate) struct VisibleTargetInput<'a> {
    pub(crate) handle: &'a NativeHandle,
    pub(crate) adapter: &'a dyn PlatformAdapter,
    pub(crate) request: &'a ActionRequest,
    pub(crate) pointer_delivery: PointerDelivery,
    pub(crate) verified_point: Option<&'a Point>,
    pub(crate) presents: bool,
    pub(crate) deadline: crate::Deadline,
}

/// `Ok(None)` keeps the preflight as it is: the action is not a pointer
/// action, the target is nonspatial, or the adapter cannot tell what is on
/// screen. A target proven hidden is refused with a failed `visible` check,
/// which the caller's scroll-into-view recovery and retry loop understand.
pub(crate) fn visible_target(
    input: VisibleTargetInput<'_>,
) -> Result<Option<VisibleTarget>, AdapterError> {
    if !input.request.action.requires_hit_test()
        || matches!(input.pointer_delivery, PointerDelivery::NotApplicable)
        || input
            .adapter
            .is_nonspatial_target_with_deadline(input.handle, input.deadline)
            .unwrap_or(false)
    {
        return Ok(None);
    }
    let visibility = match input
        .adapter
        .target_visibility(input.handle, input.deadline)
    {
        Ok(visibility) => visibility,
        Err(_) => return Ok(None),
    };
    let bounds = match &visibility {
        TargetVisibility::Unknown => return Ok(None),
        TargetVisibility::Hidden { reason } => {
            return Err(hidden_error(reason, input.pointer_delivery));
        }
        TargetVisibility::Visible { bounds } => *bounds,
    };
    let target = |point: Option<Point>, verdict| VisibleTarget {
        point,
        bounds: Some(bounds),
        verdict,
        visibility: visibility.clone(),
        occluder: None,
    };
    if matches!(input.pointer_delivery, PointerDelivery::Physical) {
        return Ok(Some(match input.verified_point {
            Some(point) if contains(bounds, point) => target(Some(point.clone()), "hit"),
            _ => target(center(bounds), "visible_unverified"),
        }));
    }
    if !input.presents {
        return Ok(Some(target(center(bounds), "visible")));
    }
    let (check, point) = receives_events_check(
        Some(bounds),
        input.handle,
        input.adapter,
        input.request,
        input.deadline,
    )?;
    Ok(Some(match check.status {
        ActionabilityStatus::Pass => target(point.or_else(|| center(bounds)), "hit"),
        ActionabilityStatus::Fail => VisibleTarget {
            point: None,
            bounds: None,
            verdict: "occluded",
            visibility,
            occluder: check.occluder.as_ref().map(|occluder| json!(occluder)),
        },
        ActionabilityStatus::Unknown => target(center(bounds), "visible_unverified"),
    }))
}

fn hidden_error(reason: &str, delivery: PointerDelivery) -> AdapterError {
    let report = ActionabilityReport::from_checks(
        vec![fail(
            "visible",
            format!("target is not on screen: {reason}"),
        )],
        None,
        None,
        None,
        delivery,
    );
    AdapterError::new(
        ErrorCode::ActionFailed,
        format!("Target is not actionable: {}", report.failure_reasons()),
    )
    .with_details(json!(report))
    .with_suggestion(
        "The element is not visible on screen. Bring its window forward or scroll it into view, then take a fresh snapshot and click the new ref.",
    )
    .with_disposition(crate::DeliverySemantics::not_delivered())
}

fn contains(bounds: Rect, point: &Point) -> bool {
    point.x >= bounds.x
        && point.y >= bounds.y
        && point.x <= bounds.x + bounds.width
        && point.y <= bounds.y + bounds.height
}

fn center(bounds: Rect) -> Option<Point> {
    (bounds.width > 0.0 && bounds.height > 0.0).then(|| Point {
        x: bounds.x + bounds.width / 2.0,
        y: bounds.y + bounds.height / 2.0,
    })
}

#[cfg(test)]
#[path = "visible_target_tests.rs"]
mod tests;
