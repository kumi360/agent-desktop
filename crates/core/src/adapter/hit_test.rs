use crate::Rect;
use serde::{Deserialize, Serialize};

/// Classifies whether a hit-tested point reaches the intended target. A hit
/// on the target itself or one of its descendants reaches it; a hit outside
/// the target's ancestor chain names a real occluder (modal, overlay,
/// sibling); a hit on the target's own ancestor is `Unknown` rather than a
/// false occlusion, since composited or custom-drawn views often expose no
/// distinct child node to hit-test. Probe failures are `Unknown` for the
/// same reason: unavailable evidence is never a false failure.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum HitTestResult {
    ReachesTarget,
    InterceptedBy {
        #[serde(skip_serializing_if = "Option::is_none")]
        role: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        bounds: Option<Rect>,
    },
    Unknown,
}

/// Where a resolved element can actually be seen right now, after every
/// clipping ancestor, the owning window's on-screen state and the attached
/// displays are applied. `Visible` carries the seen part of the element in
/// screen coordinates; it is the only rectangle a pointer or a drawn
/// highlight may aim at. `Hidden` is proof that nothing of the element is on
/// screen (clipped out, minimized, application hidden, off every display, or
/// zero sized). `Unknown` means the adapter could not establish either, so
/// callers keep their previous behaviour rather than fail on missing evidence.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "visibility", rename_all = "snake_case")]
pub enum TargetVisibility {
    Visible { bounds: Rect },
    Hidden { reason: String },
    Unknown,
}

#[cfg(test)]
#[path = "hit_test_tests.rs"]
mod tests;
