use crate::AppError;
use crate::refs::refs_test_support::HomeGuard;
use crate::session::{ArtifactsMode, SessionTraceMode, StartSessionOptions, start_session};
use std::os::unix::fs::PermissionsExt;

fn options() -> StartSessionOptions {
    StartSessionOptions {
        name: None,
        trace: SessionTraceMode::On,
        artifacts: ArtifactsMode::Events,
    }
}

fn mode(path: &std::path::Path) -> u32 {
    std::fs::metadata(path).unwrap().permissions().mode() & 0o777
}

#[test]
fn a_session_does_not_start_in_a_state_root_that_group_users_can_write() {
    let guard = HomeGuard::new();
    let root = guard.path().join(".agent-desktop");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o775)).unwrap();
    let error = start_session(options()).expect_err("a shared root is refused");
    assert_eq!(error.code(), "PERM_DENIED");
    let AppError::Adapter(adapter) = &error else {
        panic!("expected an adapter error, got {error:?}");
    };
    assert!(adapter.message.contains("accessible by group or other"));
    let suggestion = adapter.suggestion.as_deref().unwrap_or_default();
    assert!(
        suggestion.contains(&format!("chmod 700 '{}'", root.display())),
        "the suggestion names the exact command: {suggestion}"
    );
    assert!(!root.join("sessions").exists());
    assert_eq!(
        mode(&root),
        0o775,
        "a writable root is never silently changed"
    );
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
    start_session(options()).expect("a private root starts a session");
}

#[test]
fn a_readable_state_root_is_made_private_and_the_session_starts() {
    let guard = HomeGuard::new();
    let root = guard.path().join(".agent-desktop");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o755)).unwrap();
    start_session(options()).expect("a 755 root is repaired, not refused");
    assert_eq!(mode(&root), 0o700);
    assert!(root.join("sessions").exists());
}

#[test]
fn a_read_only_for_others_root_like_750_is_also_repaired() {
    let guard = HomeGuard::new();
    let root = guard.path().join(".agent-desktop");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o750)).unwrap();
    start_session(options()).expect("a 750 root is repaired");
    assert_eq!(mode(&root), 0o700);
}
