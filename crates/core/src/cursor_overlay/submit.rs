use super::CursorPhase;
use crate::{
    AdapterError, CommandContext, DeliveryDisposition, DeliverySemantics, MouseEvent,
    OperationReport, PlatformAdapter, Point, Rect,
};

const DISPATCH_RESERVE_MS: u64 = 100;

pub(crate) fn travel_scope(
    lease: &crate::InteractionLease,
) -> Option<crate::wait::deadline::DeadlineScope> {
    travel_scope_within(lease.deadline())
}

fn travel_scope_within(deadline: crate::Deadline) -> Option<crate::wait::deadline::DeadlineScope> {
    if deadline.remaining_ms()
        <= crate::CURSOR_ARRIVAL_TIMEOUT_MS.saturating_add(DISPATCH_RESERVE_MS)
    {
        return None;
    }
    Some(crate::wait::deadline::enter_scope(Some(deadline.capped(
        std::time::Duration::from_millis(crate::CURSOR_ARRIVAL_TIMEOUT_MS),
    ))))
}

/// Moves the drawn cursor toward an action's target before the interaction
/// lease is held. Travel is presentation only, so holding the machine-wide
/// lease across it would serialize every other caller behind an animation;
/// the travel submitted under the lease then only covers any distance the
/// live target moved since the snapshot.
pub(crate) fn submit_travel_before_lease(
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
    destination: Point,
    deadline: crate::Deadline,
    cue: super::TravelCue<'_>,
) -> bool {
    travel_within(adapter, context, destination, deadline, cue)
}

/// Moves the drawn cursor to `destination` before an action; `pointer` arrives showing the
/// pointer image because the action lands on a pressable control. The flag is sent only
/// when a pointer image is configured, so controls without one stay byte-identical to what
/// earlier renderers accept.
pub(crate) fn submit_travel(
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
    destination: Point,
    lease: &crate::InteractionLease,
    cue: super::TravelCue<'_>,
) {
    travel_within(adapter, context, destination, lease.deadline(), cue);
}

fn travel_within(
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
    destination: Point,
    deadline: crate::Deadline,
    cue: super::TravelCue<'_>,
) -> bool {
    let Some(_scope) = travel_scope_within(deadline) else {
        return false;
    };
    let instruction = super::CursorOverlayInstruction::new(
        destination,
        context.cursor_overlay(),
        false,
    )
    .map(|instruction| {
        instruction
            .with_pointer(cue.pointer && context.cursor_overlay().style().pointer_image().is_some())
            .with_narration(cue.narration)
    });
    send(adapter, context, instruction)
}

/// Places the agent's text caret at `caret`, where the app is inserting text,
/// without moving the cursor.
pub(crate) fn submit_caret(adapter: &dyn PlatformAdapter, context: &CommandContext, caret: Rect) {
    let destination = Point {
        x: caret.x,
        y: caret.y + caret.height / 2.0,
    };
    let instruction =
        super::CursorOverlayInstruction::new(destination, context.cursor_overlay(), false).map(
            |instruction| {
                instruction
                    .with_target(Some(caret))
                    .with_phase(CursorPhase::Caret)
            },
        );
    let _ = send(adapter, context, instruction);
}

/// Moves a caret the overlay is already showing to `caret`, or hides it when
/// `caret` is `None`.
pub(crate) fn submit_caret_follow(
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
    caret: Option<Rect>,
) {
    let destination = caret.map_or(Point { x: 0.0, y: 0.0 }, |caret| Point {
        x: caret.x,
        y: caret.y + caret.height / 2.0,
    });
    let instruction =
        super::CursorOverlayInstruction::new(destination, context.cursor_overlay(), false).map(
            |instruction| {
                instruction
                    .with_target(caret)
                    .with_phase(CursorPhase::CaretFollow)
            },
        );
    let _ = send(adapter, context, instruction);
}

pub(crate) fn input_was_delivered(result: &Result<OperationReport, AdapterError>) -> bool {
    result.as_ref().map_or_else(
        |error| confirms_delivery(error.disposition),
        |report| confirms_delivery(report.disposition()),
    )
}

pub(crate) fn confirms_delivery(disposition: DeliverySemantics) -> bool {
    matches!(
        disposition.delivery(),
        DeliveryDisposition::DeliveryUncertain
            | DeliveryDisposition::DeliveredUnverified
            | DeliveryDisposition::DeliveredVerified
    )
}

pub(crate) fn submit(
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
    destination: Point,
    target: Option<Rect>,
    click: bool,
    phase: CursorPhase,
) {
    let instruction =
        super::CursorOverlayInstruction::new(destination, context.cursor_overlay(), click)
            .map(|instruction| instruction.with_target(target).with_phase(phase));
    let _ = send(adapter, context, instruction);
}

pub(crate) fn submit_drag(
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
    drag_from: Point,
    destination: Point,
    lease: &crate::InteractionLease,
) -> bool {
    let Some(_scope) = travel_scope(lease) else {
        return false;
    };
    let instruction =
        super::CursorOverlayInstruction::new(destination, context.cursor_overlay(), false).map(
            |instruction| {
                instruction
                    .with_drag_from(Some(drag_from))
                    .with_phase(CursorPhase::Drag)
            },
        );
    send(adapter, context, instruction)
}

pub(crate) fn submit_drag_effect(
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
    drag_from: Point,
    destination: Point,
) {
    let instruction =
        super::CursorOverlayInstruction::new(destination, context.cursor_overlay(), false).map(
            |instruction| {
                instruction
                    .with_drag_from(Some(drag_from))
                    .with_phase(CursorPhase::Effect)
            },
        );
    let _ = send(adapter, context, instruction);
}

/// Where the cursor rests to point at a whole window: the middle of its
/// title bar, inside the window even when it is short.
pub fn window_cue_point(window: &Rect) -> Point {
    Point {
        x: window.x + window.width / 2.0,
        y: window.y + (window.height / 2.0).min(14.0),
    }
}

/// A calm cue for a window-level action (opening an app, switching to a
/// window, moving, resizing, minimizing or closing it): the cursor glides to
/// the window's title bar, the window gets a soft outline, and the label
/// says what is happening. Presentation only; returns whether it was drawn.
pub fn present_window_cue(
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
    window: &Rect,
    narration: &str,
) -> bool {
    if window.validate().is_err() || window.width < 1.0 || window.height < 1.0 {
        return false;
    }
    present_cue(
        adapter,
        context,
        window_cue_point(window),
        Some(*window),
        narration,
    )
}

/// Glides the cursor to `point` with a short narration, outlining `outline`
/// on arrival when given. Presentation only; returns whether it was drawn.
pub(crate) fn present_cue(
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
    point: Point,
    outline: Option<Rect>,
    narration: &str,
) -> bool {
    let Ok(deadline) = crate::Deadline::detached_after(crate::CURSOR_ARRIVAL_TIMEOUT_MS) else {
        return false;
    };
    let _scope = crate::wait::deadline::enter_scope(Some(deadline));
    let instruction = super::CursorOverlayInstruction::new(point, context.cursor_overlay(), false)
        .map(|instruction| {
            instruction
                .with_target(outline)
                .with_narration(Some(narration))
        });
    send(adapter, context, instruction)
}

pub(crate) fn cancel_drag(adapter: &dyn PlatformAdapter, context: &CommandContext) {
    let Ok(cleanup_deadline) = crate::Deadline::detached_after(crate::CURSOR_ARRIVAL_TIMEOUT_MS)
    else {
        return;
    };
    let _scope = crate::wait::deadline::enter_scope(Some(cleanup_deadline));
    let Some(session_id) = enabled_session(context) else {
        return;
    };
    let control = super::CursorOverlayControl::hide(session_id.to_owned())
        .with_agent_id(agent_route(context));
    let _ = update(adapter, &control);
}

fn send(
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
    instruction: Result<super::CursorOverlayInstruction, AdapterError>,
) -> bool {
    let Some(session_id) = enabled_session(context) else {
        return false;
    };
    let instruction = match instruction {
        Ok(instruction) => instruction,
        Err(error) => {
            tracing::warn!(code = %error.code.as_str(), "agent cursor instruction was skipped");
            return false;
        }
    };
    let control = super::CursorOverlayControl::present_with_style(
        session_id.to_owned(),
        instruction,
        context.cursor_overlay().style().clone(),
    )
    .with_motion(context.cursor_overlay().motion().clone())
    .with_agent_id(agent_route(context));
    update(adapter, &control)
}

pub(super) fn agent_route(context: &CommandContext) -> Option<String> {
    context
        .cursor_overlay()
        .is_multi_agent()
        .then(|| context.agent_id().map(str::to_owned))
        .flatten()
}

fn enabled_session(context: &CommandContext) -> Option<&str> {
    context.overlay_key()
}

fn update(adapter: &dyn PlatformAdapter, control: &super::CursorOverlayControl) -> bool {
    if let Err(error) = adapter.update_cursor_overlay(control) {
        tracing::warn!(code = %error.code.as_str(), "agent cursor presentation was skipped");
        return false;
    }
    true
}

pub(crate) fn dispatch_mouse_event_with_cursor(
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
    event: MouseEvent,
    click: bool,
    lease: &crate::InteractionLease,
) -> Result<OperationReport, AdapterError> {
    let point = event.point.clone();
    crate::cursor_overlay::submit_travel(
        adapter,
        context,
        point.clone(),
        lease,
        super::TravelCue::new(click, None),
    );
    let result = adapter.mouse_event(event, lease);
    if crate::cursor_overlay::input_was_delivered(&result) {
        crate::cursor_overlay::submit(
            adapter,
            context,
            point,
            None,
            click,
            crate::CursorPhase::Effect,
        );
    }
    result
}

#[cfg(test)]
#[path = "submit_tests.rs"]
mod tests;
