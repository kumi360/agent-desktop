use agent_desktop_core::{
    AdapterError, CURSOR_IDLE_REST_MS, CursorMotion, CursorMotionProfile, CursorOverlayControl,
    CursorOverlayInstruction, CursorOverlayStyle, CursorPhase, CursorPose, Point,
};
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;
use std::time::{Duration, Instant};

use super::bridge;

#[path = "child_socket.rs"]
mod socket;
use socket::{cleanup, prepare_socket, prepare_stream, read_control, socket_path};

pub(super) const MARKER: &str = "AGENT_DESKTOP_CURSOR_OVERLAY_CHILD";
pub(super) const SOCKET_ENV: &str = "AGENT_DESKTOP_CURSOR_OVERLAY_SOCKET";

#[derive(Default)]
struct OverlayState {
    style: CursorOverlayStyle,
    motion: CursorMotionProfile,
    moves: u64,
    at: Option<Point>,
    dragging: bool,
    resting: bool,
    cues: super::cues::Cues,
}

pub(crate) fn entry_from_env() -> Option<Result<(), AdapterError>> {
    match std::env::var(MARKER) {
        Err(_) => None,
        Ok(value) if value == super::endpoint::PROTOCOL_VERSION => Some(run()),
        Ok(_) => Some(Err(AdapterError::internal(
            "Invalid cursor overlay child protocol marker",
        ))),
    }
}

fn run() -> Result<(), AdapterError> {
    let initial = read_control(std::io::stdin())?;
    if !super::lifecycle::session_active(initial.session_id(), initial.agent_id()) {
        return Ok(());
    }
    let socket = socket_path(&initial)?;
    prepare_socket(&socket)?;
    let mut state = OverlayState::default();
    let session = super::session_flow::is_session(initial.session_id());
    if !initial.is_enable() {
        state.at = super::session_flow::recall();
    }
    if !handle(&initial, &mut state)? {
        return cleanup(socket);
    }
    let listener = UnixListener::bind(&socket).map_err(|error| {
        AdapterError::internal("Could not bind the cursor overlay session socket")
            .with_platform_detail(error.to_string())
    })?;
    std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o600)).map_err(|error| {
        AdapterError::internal("Could not protect the cursor overlay session socket")
            .with_platform_detail(error.to_string())
    })?;
    listener.set_nonblocking(true).map_err(|error| {
        AdapterError::internal("Could not configure the cursor overlay session socket")
            .with_platform_detail(error.to_string())
    })?;
    let mut quiet_since = std::time::Instant::now();
    let mut checked_session = quiet_since;
    let build = super::lifecycle::BuildIdentity::current();
    let idle_exit = super::lifecycle::idle_exit_for(initial.session_id());
    loop {
        if checked_session.elapsed() >= Duration::from_secs(1) {
            let ended = !super::lifecycle::session_active(initial.session_id(), initial.agent_id());
            if ended
                || super::lifecycle::should_retire(
                    build.as_ref(),
                    quiet_since.elapsed(),
                    idle_exit,
                    bridge::drag_active(),
                )
            {
                if ended && session {
                    super::session_flow::farewell(state.at.as_ref());
                }
                return cleanup(socket);
            }
            checked_session = std::time::Instant::now();
        }
        match listener.accept() {
            Ok((mut stream, _)) => {
                if prepare_stream(&stream).is_err() {
                    continue;
                }
                let Ok(control) = read_control(&mut stream) else {
                    continue;
                };
                if control.session_id() != initial.session_id()
                    || (control.agent_id() != initial.agent_id()
                        && !(control.agent_id().is_none()
                            && (control.is_transient() || control.is_disable())))
                {
                    continue;
                }
                if control.is_disable() {
                    drop(listener);
                    if session {
                        super::session_flow::farewell(state.at.as_ref());
                    }
                    cleanup(socket)?;
                    let _ = stream.write_all(&[1]);
                    return Ok(());
                }
                if state.resting && control.is_caret_follow() {
                    let _ = stream.write_all(&[1]);
                    continue;
                }
                quiet_since = std::time::Instant::now();
                if std::mem::take(&mut state.resting) {
                    bridge::show();
                }
                if state.cues.stop_thinking() {
                    bridge::think(None);
                }
                let landed = state.at.clone();
                let outcome = handle(&control, &mut state);
                if state.at != landed {
                    super::session_flow::remember(state.at.as_ref());
                }
                if outcome.is_ok() && control.expects_acknowledgement() {
                    let _ = stream.write_all(&[1]);
                }
                if outcome.is_err() {
                    continue;
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                bridge::idle();
                pass_quiet_time(&mut state, session, quiet_since.elapsed());
                super::lifecycle::wait_for_connection(&listener, Duration::from_millis(8));
            }
            Err(error) => {
                let _ = cleanup(socket);
                return Err(
                    AdapterError::internal("Cursor overlay session socket failed")
                        .with_platform_detail(error.to_string()),
                );
            }
        }
    }
}

fn handle(control: &CursorOverlayControl, state: &mut OverlayState) -> Result<bool, AdapterError> {
    control.validate()?;
    if control.is_disable() {
        return Ok(false);
    }
    if let Some(image_changed) = absorb_settings(control, state) {
        bridge::apply_style(&state.style, image_changed);
    }
    if control.is_hide() || control.is_show() {
        if control.is_show() {
            bridge::show();
        } else if state.dragging {
            bridge::hide();
        }
        apply_landing_memory(control, state, None);
        return Ok(true);
    }
    let Some(sent) = control.instruction() else {
        return open(control, state);
    };
    let carried = state.cues.carry(sent);
    let instruction = carried.as_ref().unwrap_or(sent);
    render(instruction, state)?;
    state.cues.drawn(instruction, Instant::now());
    apply_landing_memory(control, state, Some(instruction));
    advance_moves(control, state);
    Ok(true)
}

/// A session's opening note, drawn where the cursor already is so it never
/// jumps, or at the centre of the main screen when it is new.
fn open(control: &CursorOverlayControl, state: &mut OverlayState) -> Result<bool, AdapterError> {
    let greeting_over_image = state.style.image().is_some();
    let label = control.label().filter(|_| !greeting_over_image);
    let fresh = state.at.is_none();
    let start = match state.at.clone() {
        Some(at) => at,
        None => bridge::initial_point()?,
    };
    let note = super::session_flow::still(&start, label)?;
    let session = super::session_flow::is_session(control.session_id());
    super::session_flow::open(&note, fresh, session)?;
    state.cues.opened();
    apply_landing_memory(control, state, Some(&note));
    Ok(true)
}

/// Between steps a session's cursor stays on screen and, once the agent has
/// been quiet a moment, shows the thinking spinner. An ambient cursor lets
/// its narration fade and then rests out of sight.
fn pass_quiet_time(state: &mut OverlayState, session: bool, quiet_for: Duration) {
    if bridge::drag_active() {
        return;
    }
    if session {
        if state.at.is_some() && state.cues.think_due(quiet_for) {
            bridge::think(state.at.as_ref());
            state.cues.start_thinking();
        }
        return;
    }
    if state.cues.narration_due(Instant::now()) {
        state.cues.narration_faded();
        bridge::fade_label();
    }
    if !state.resting && quiet_for.as_millis() >= u128::from(CURSOR_IDLE_REST_MS) {
        bridge::rest();
        state.resting = true;
    }
}

fn absorb_settings(control: &CursorOverlayControl, state: &mut OverlayState) -> Option<bool> {
    if let Some(motion) = control.motion() {
        state.motion = motion.clone();
    }
    if control.is_enable() {
        state.moves = 0;
    }
    absorb_style(control, state)
}

fn advance_moves(control: &CursorOverlayControl, state: &mut OverlayState) {
    if control
        .instruction()
        .is_some_and(|instruction| instruction.phase() == CursorPhase::Travel)
    {
        state.moves = state.moves.wrapping_add(1);
    }
}

fn absorb_style(control: &CursorOverlayControl, state: &mut OverlayState) -> Option<bool> {
    let style = control.style()?;
    let image_changed = state.style.images() != style.images();
    state.style = style.clone();
    Some(image_changed)
}

fn apply_landing_memory(
    control: &CursorOverlayControl,
    state: &mut OverlayState,
    instruction: Option<&CursorOverlayInstruction>,
) {
    if control.is_hide() {
        if std::mem::take(&mut state.dragging) {
            state.at = None;
        }
        return;
    }
    if control.is_show() {
        return;
    }
    let Some(instruction) = instruction else {
        return;
    };
    if bridge::is_caret(instruction) {
        return;
    }
    state.dragging = instruction.phase() == CursorPhase::Drag;
    state.at = Some(if state.dragging {
        instruction
            .drag_from()
            .unwrap_or(instruction.destination())
            .clone()
    } else {
        instruction.destination().clone()
    });
}

fn render(
    instruction: &CursorOverlayInstruction,
    state: &OverlayState,
) -> Result<(), AdapterError> {
    if bridge::apply_caret(instruction, state.at.as_ref()) {
        return Ok(());
    }
    if instruction.phase() == CursorPhase::Drag {
        let from = instruction.drag_from().unwrap_or(instruction.destination());
        let (screen, fps, reduce_motion) = bridge::screen_at(from)?;
        let pace = bridge::RenderPace {
            fps,
            reduce_motion,
            announce: false,
            fade_in: false,
        };
        bridge::run(
            &[CursorPose::still(from.clone())],
            pace,
            instruction,
            &screen,
        )?;
        bridge::begin_drag(from, state.style.ripple() && !reduce_motion);
        return Ok(());
    }
    if instruction.phase() == CursorPhase::Effect {
        bridge::end_drag(instruction.destination(), instruction.drag_from().is_some());
    }
    let (screen, fps, reduce_motion) = bridge::screen_at(instruction.destination())?;
    let shown = if state.style.highlight() {
        instruction.clone()
    } else {
        instruction.clone().with_target(None)
    };
    let frames = frames_for(state, &shown, &screen, fps, reduce_motion);
    let pace = bridge::RenderPace {
        fps,
        reduce_motion,
        announce: shown.phase() == CursorPhase::Travel && shown.is_narrated(),
        fade_in: false,
    };
    bridge::run(&frames, pace, &shown, &screen)
}

fn frames_for(
    state: &OverlayState,
    instruction: &CursorOverlayInstruction,
    screen: &agent_desktop_core::Rect,
    fps: u32,
    reduce_motion: bool,
) -> Vec<CursorPose> {
    if reduce_motion {
        vec![CursorPose::still(instruction.destination().clone())]
    } else {
        motion_frames(state, instruction, screen, fps)
    }
}

fn motion_frames(
    state: &OverlayState,
    instruction: &CursorOverlayInstruction,
    screen: &agent_desktop_core::Rect,
    fps: u32,
) -> Vec<CursorPose> {
    let destination = instruction.destination();
    if instruction.phase() == CursorPhase::Effect {
        let mut frames = vec![CursorPose::still(destination.clone())];
        if instruction.is_click() && state.style.ripple() {
            frames.push(CursorPose {
                point: destination.clone(),
                ripple: 1.0,
            });
        }
        return frames;
    }
    let start = state.at.clone().unwrap_or_else(|| Point {
        x: (destination.x - 180.0).clamp(screen.x, screen.x + screen.width),
        y: (destination.y + 108.0).clamp(screen.y, screen.y + screen.height),
    });
    let motion = CursorMotion::shaped(start, destination.clone(), &state.motion, state.moves)
        .with_impact(instruction.is_click())
        .with_ripple(state.style.ripple());
    let frame_ms = 1_000.0 / f64::from(fps);
    let frame_count = (motion.total_ms() as f64 / frame_ms).ceil() as u64;
    (0..=frame_count)
        .map(|frame| {
            let elapsed = ((frame as f64 * frame_ms).round() as u64).min(motion.total_ms());
            motion.pose(elapsed)
        })
        .collect()
}

#[cfg(test)]
#[path = "child_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "child_motion_tests.rs"]
mod motion_tests;

#[cfg(test)]
#[path = "child_image_tests.rs"]
mod image_tests;
