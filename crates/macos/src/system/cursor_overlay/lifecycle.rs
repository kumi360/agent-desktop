//! Keeps overlays from outliving their purpose: a running overlay retires
//! itself when the binary it was started from is replaced or it has been idle
//! too long, and a new overlay first retires every other overlay process of
//! this user that belongs to another session, whatever build or protocol it
//! came from. One session's cursor is on screen at a time, never an orphan.

use std::os::unix::fs::MetadataExt;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Which file this process was started from, so it can tell when a new
/// build has been installed over it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct BuildIdentity {
    path: PathBuf,
    device: u64,
    inode: u64,
    modified: i64,
}

impl BuildIdentity {
    pub(super) fn current() -> Option<Self> {
        Self::of(&std::env::current_exe().ok()?)
    }

    pub(super) fn of(path: &Path) -> Option<Self> {
        let metadata = std::fs::metadata(path).ok()?;
        Some(Self {
            path: path.to_path_buf(),
            device: metadata.dev(),
            inode: metadata.ino(),
            modified: metadata.mtime(),
        })
    }

    /// True when the file at the start path is no longer the one this
    /// process was started from (replaced, or removed).
    pub(super) fn replaced(&self) -> bool {
        Self::of(&self.path).is_none_or(|now| now != *self)
    }
}

/// Whether an overlay should leave now: its build was replaced, or it has
/// had nothing to show for `idle_exit` (never in the middle of a drag).
pub(super) fn should_retire(
    build: Option<&BuildIdentity>,
    quiet_for: Duration,
    idle_exit: Duration,
    dragging: bool,
) -> bool {
    if build.is_some_and(BuildIdentity::replaced) {
        return true;
    }
    !dragging && quiet_for >= idle_exit
}

/// Stops every overlay process of this user whose socket does not belong to
/// `session_id`: orphans of sessions that were never ended, older builds and
/// older protocols. Returns the pids it stopped.
pub(super) fn retire_other_overlays(session_id: &str) -> Vec<i32> {
    let (Ok(directories), Ok(keep)) = (
        super::endpoint::socket_directories(),
        super::endpoint::session_prefix(session_id),
    ) else {
        return Vec::new();
    };
    let mut stopped = Vec::new();
    for socket in overlay_sockets(&directories) {
        let name = socket
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        if name.starts_with(&keep) {
            continue;
        }
        let Some(pid) = overlay_owner(&socket) else {
            continue;
        };
        if unsafe { libc::kill(pid, libc::SIGTERM) } == 0 {
            tracing::info!(pid, socket = %name, "retired another cursor overlay");
            let _ = std::fs::remove_file(&socket);
            stopped.push(pid);
        }
    }
    stopped
}

pub(super) fn is_overlay_socket_name(name: &str) -> bool {
    name.strip_prefix(".cursor-overlay-")
        .and_then(|rest| rest.strip_suffix(".sock"))
        .is_some_and(|rest| {
            !rest.is_empty()
                && rest
                    .split('-')
                    .all(|part| part.len() == 16 && part.bytes().all(|b| b.is_ascii_hexdigit()))
        })
}

fn overlay_sockets(directories: &[PathBuf]) -> Vec<PathBuf> {
    let mut sockets = Vec::new();
    for directory in directories {
        let Ok(entries) = std::fs::read_dir(directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name();
            if is_overlay_socket_name(&name.to_string_lossy())
                && super::endpoint::is_private_socket(&path)
            {
                sockets.push(path);
            }
        }
    }
    sockets.sort();
    sockets
}

/// The pid listening on `socket`, when it is a live agent-desktop process of
/// this user other than this one.
fn overlay_owner(socket: &Path) -> Option<i32> {
    let stream = UnixStream::connect(socket).ok()?;
    let pid = peer_pid(&stream)?;
    drop(stream);
    if pid <= 1 || pid == std::process::id() as i32 {
        return None;
    }
    is_agent_desktop(pid).then_some(pid)
}

fn peer_pid(stream: &UnixStream) -> Option<i32> {
    let mut pid: libc::pid_t = 0;
    let mut len = std::mem::size_of::<libc::pid_t>() as libc::socklen_t;
    let status = unsafe {
        libc::getsockopt(
            std::os::fd::AsRawFd::as_raw_fd(stream),
            libc::SOL_LOCAL,
            libc::LOCAL_PEERPID,
            (&raw mut pid).cast(),
            &raw mut len,
        )
    };
    (status == 0 && pid > 0).then_some(pid)
}

fn is_agent_desktop(pid: i32) -> bool {
    let mut buffer = vec![0_u8; libc::PROC_PIDPATHINFO_MAXSIZE as usize];
    let written =
        unsafe { libc::proc_pidpath(pid, buffer.as_mut_ptr().cast(), buffer.len() as u32) };
    if written <= 0 {
        return false;
    }
    buffer.truncate(written as usize);
    let path = PathBuf::from(String::from_utf8_lossy(&buffer).into_owned());
    path.file_stem().and_then(|stem| stem.to_str()) == Some("agent-desktop")
}

/// An ambient overlay (commands run without a session) leaves the screen
/// after this long with nothing to show; the next action brings it back.
const AMBIENT_IDLE_EXIT: Duration = Duration::from_secs(120);
/// A session overlay whose agent has gone quiet this long (a harness that
/// was killed or never ran `session end`) leaves instead of lingering.
const SESSION_IDLE_EXIT: Duration = Duration::from_secs(600);

pub(super) fn idle_exit_for(session_id: &str) -> Duration {
    if session_id == agent_desktop_core::AMBIENT_CURSOR_ID {
        AMBIENT_IDLE_EXIT
    } else {
        SESSION_IDLE_EXIT
    }
}

pub(super) fn session_active(session_id: &str, agent_id: Option<&str>) -> bool {
    if session_id == agent_desktop_core::AMBIENT_CURSOR_ID {
        return agent_id.is_none() && agent_desktop_core::ambient_active();
    }
    agent_desktop_core::session::read_manifest(session_id)
        .ok()
        .flatten()
        .is_some_and(|manifest| {
            manifest.ended_at.is_none()
                && manifest.cursor_overlay.is_enabled()
                && !(manifest.cursor_overlay.is_multi_agent() && agent_id.is_none())
        })
}

#[cfg(test)]
#[path = "lifecycle_tests.rs"]
mod tests;
