//! The ambient cursor: what agent-desktop draws when it acts without a
//! session. A harness that never runs `session start` or `cursor-overlay
//! enable` still shows the person watching a purple cursor, so the agent's
//! actions never look like apps popping up by themselves.
//!
//! It is on by default. `AGENT_DESKTOP_CURSOR=off` turns it off for one
//! process tree, and `cursor-overlay disable` without a session turns it off
//! for the user until `cursor-overlay enable` turns it back on.

use super::CursorOverlayConfig;
use crate::AppError;
use std::path::PathBuf;

/// The overlay key session-less commands draw under. Real session ids are
/// `run-…` or user names that pass `validate_session_id`; the leading
/// underscore keeps this one apart from generated ids.
pub const AMBIENT_CURSOR_ID: &str = "_ambient";

/// Environment switch for the default-on cursor: `off`, `0`, `false`, `no`
/// or `none` turn it off; anything else (or unset) leaves it on.
pub const CURSOR_ENV: &str = "AGENT_DESKTOP_CURSOR";

const PREFERENCE_FILE: &str = "ambient-cursor.json";
const MAX_PREFERENCE_BYTES: u64 = 16 * 1024;
const DEFAULT_WORDS: usize = 6;

/// Whether the environment turns the default cursor off.
pub fn cursor_env_off() -> bool {
    std::env::var(CURSOR_ENV).is_ok_and(|value| env_value_is_off(&value))
}

pub(crate) fn env_value_is_off(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "off" | "0" | "false" | "no" | "none" | "disable" | "disabled"
    )
}

/// The cursor a session-less command draws with.
pub fn ambient_config() -> CursorOverlayConfig {
    if cursor_env_off() {
        return CursorOverlayConfig::default();
    }
    saved_preference().unwrap_or_else(default_on)
}

/// Whether a running ambient overlay should stay up. The overlay process
/// reads only the saved preference: the environment of whoever spawned it
/// already allowed it to start.
pub fn ambient_active() -> bool {
    saved_preference().is_none_or(|config| config.is_enabled())
}

/// Saves the ambient cursor for later session-less commands.
pub fn save_ambient_config(config: &CursorOverlayConfig) -> Result<(), AppError> {
    let config = config.clone().validated()?;
    let json = serde_json::to_vec_pretty(&config)?;
    let root = crate::session::agent_desktop_dir()?;
    crate::session::make_state_root_private(&root)?;
    crate::refs::write_private_file(&root.join(PREFERENCE_FILE), &json)
}

fn default_on() -> CursorOverlayConfig {
    CursorOverlayConfig::enabled(None, DEFAULT_WORDS).unwrap_or_default()
}

fn preference_path() -> Option<PathBuf> {
    crate::session::agent_desktop_dir()
        .ok()
        .map(|root| root.join(PREFERENCE_FILE))
}

fn saved_preference() -> Option<CursorOverlayConfig> {
    let path = preference_path()?;
    let bytes =
        crate::storage::private_file::read_private_bounded(&path, MAX_PREFERENCE_BYTES).ok()?;
    serde_json::from_slice::<CursorOverlayConfig>(&bytes)
        .ok()
        .and_then(|config| config.validated().ok())
        .map(|config| config.with_multi_agent(false))
}

#[cfg(test)]
#[path = "ambient_tests.rs"]
mod tests;
