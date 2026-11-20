//! Runs the visible-target gate after the live actionability preflight, so
//! a pointer action is refused when its element is proven off screen and the
//! drawn cursor aims only at the element's visible, hit-tested part.

use super::{ActionabilityPreflight, ResolvedRefAction};
use crate::actionability::{
    PointerDelivery,
    visible_target::{VisibleTargetInput, visible_target},
};
use crate::{AppError, actions::action_request::ActionRequest};
use serde_json::json;

/// The stable preflight followed by the visible-target gate.
pub(super) fn gated(
    target: &ResolvedRefAction<'_>,
    request: &ActionRequest,
) -> Result<ActionabilityPreflight, AppError> {
    let mut preflight = super::stable_preflight(target, request)?;
    apply(target, request, &mut preflight)?;
    Ok(preflight)
}

/// Narrows `preflight` to the visible target, or refuses a hidden one.
pub(super) fn apply(
    target: &ResolvedRefAction<'_>,
    request: &ActionRequest,
    preflight: &mut ActionabilityPreflight,
) -> Result<(), AppError> {
    let gate = visible_target(VisibleTargetInput {
        handle: target.handle,
        adapter: target.adapter,
        request,
        pointer_delivery: preflight.pointer_delivery,
        verified_point: preflight.verified_point.as_ref(),
        presents: target.context.cursor_overlay().is_enabled(),
        deadline: target.deadline,
    });
    let gate = match gate {
        Ok(Some(gate)) => gate,
        Ok(None) => return Ok(()),
        Err(error) => {
            target.context.trace_lazy("presentation.target", || {
                json!({
                    "ref": target.ref_id,
                    "verdict": "hidden",
                    "message": error.message.clone(),
                })
            })?;
            return Err(error.into());
        }
    };
    target.context.trace_lazy("presentation.target", || {
        json!({
            "ref": target.ref_id,
            "verdict": gate.verdict,
            "visibility": &gate.visibility,
            "point": &gate.point,
            "bounds": &gate.bounds,
            "occluder": &gate.occluder,
            "delivery": format!("{:?}", preflight.pointer_delivery),
        })
    })?;
    if matches!(preflight.pointer_delivery, PointerDelivery::Physical)
        && preflight.verified_point.is_none()
    {
        preflight.verified_point = gate.point.clone();
    }
    preflight.presentation_point = gate.point;
    preflight.presentation_bounds = gate.bounds;
    Ok(())
}
