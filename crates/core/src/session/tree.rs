use std::path::Path;

use crate::{AdapterError, AppError, ErrorCode};

/// Creates the session directory and its trace directory. The state root
/// holds every session's files, so it must be private before a session is
/// written into it. A root that is ours and only readable by others (a
/// `mkdir` under a 022 umask gives 755) is tightened to 700 in place, since
/// nobody else could have written into it. A root that others can write to,
/// or that someone else owns, is refused with the exact command to fix it.
pub(super) fn create(dir: &Path) -> Result<(), AppError> {
    let root = crate::storage::state_root::resolve_configured_state_root()?;
    make_root_private(&root)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(dir)?;
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(dir.join("trace"))?;
    }
    #[cfg(not(unix))]
    {
        crate::storage::private_file_parent::ensure_private(&dir.join("trace"))?;
    }
    Ok(())
}

/// Makes the state root private, repairing a merely readable root and
/// refusing anything else with a message that says exactly what to run.
pub(crate) fn make_root_private(root: &Path) -> Result<(), AppError> {
    #[cfg(unix)]
    tighten_readable_root(root)?;
    crate::storage::private_file_parent::ensure_private(root)
        .map_err(|error| friendly_root_error(root, &error))
}

#[cfg(unix)]
fn tighten_readable_root(root: &Path) -> Result<(), AppError> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let Ok(metadata) = std::fs::symlink_metadata(root) else {
        return Ok(());
    };
    let ours = metadata.uid() == unsafe { libc::geteuid() };
    let mode = metadata.mode();
    if !metadata.is_dir() || !ours || mode & 0o077 == 0 || mode & 0o022 != 0 {
        return Ok(());
    }
    if crate::refs::home_dir().is_some_and(|home| same_directory(&home, root)) {
        return Ok(());
    }
    std::fs::set_permissions(root, std::fs::Permissions::from_mode(0o700))
        .map_err(|error| friendly_root_error(root, &error))?;
    tracing::info!(
        path = %root.display(),
        from = format!("{:o}", mode & 0o777),
        "made the agent-desktop state folder private (700)"
    );
    Ok(())
}

#[cfg(unix)]
fn same_directory(a: &Path, b: &Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

fn friendly_root_error(root: &Path, error: &std::io::Error) -> AppError {
    let path = root.display();
    let detail = error.to_string();
    let suggestion = if detail.contains("not owned") {
        format!(
            "This folder belongs to another user. Run `sudo chown -R $(id -un) '{path}' && chmod 700 '{path}'`, or point AGENT_DESKTOP_HOME at a folder you own."
        )
    } else if detail.contains("accessible by group or other") {
        format!(
            "Other users can write to this folder, so agent-desktop will not keep sessions there. Run `chmod 700 '{path}'`, then try again."
        )
    } else {
        format!(
            "Make sure '{path}' is a real folder you own with mode 700 (`chmod 700 '{path}'`), or set AGENT_DESKTOP_HOME to one."
        )
    };
    AdapterError::new(
        ErrorCode::PermDenied,
        format!("The agent-desktop state folder '{path}' is not private: {detail}"),
    )
    .with_suggestion(suggestion)
    .into()
}

#[cfg(all(test, unix))]
#[path = "tree_tests.rs"]
mod tests;
