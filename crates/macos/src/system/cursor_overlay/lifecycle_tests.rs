use super::*;

#[test]
fn a_replaced_binary_retires_the_overlay() {
    let dir = std::env::temp_dir().join(format!("ad-lifecycle-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let exe = dir.join("agent-desktop");
    std::fs::write(&exe, b"old build").unwrap();
    let build = BuildIdentity::of(&exe).unwrap();
    assert!(!build.replaced());
    assert!(!should_retire(
        Some(&build),
        Duration::from_secs(1),
        Duration::from_secs(60),
        false
    ));
    let staged = dir.join("agent-desktop.new");
    std::fs::write(&staged, b"new build").unwrap();
    std::fs::rename(&staged, &exe).unwrap();
    assert!(build.replaced(), "an atomic swap is a new build");
    assert!(should_retire(
        Some(&build),
        Duration::ZERO,
        Duration::from_secs(60),
        true
    ));
    std::fs::remove_file(&exe).unwrap();
    assert!(build.replaced(), "a removed binary is a replaced one");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_idle_overlay_retires_but_never_mid_drag() {
    let idle = Duration::from_secs(120);
    assert!(!should_retire(None, Duration::from_secs(119), idle, false));
    assert!(should_retire(None, Duration::from_secs(120), idle, false));
    assert!(!should_retire(None, Duration::from_secs(900), idle, true));
}

#[test]
fn overlay_socket_names_of_every_protocol_are_recognised() {
    assert!(is_overlay_socket_name(
        ".cursor-overlay-2e411796a1574282.sock"
    ));
    assert!(is_overlay_socket_name(
        ".cursor-overlay-2e411796a1574282-00000000000000ff.sock"
    ));
    assert!(!is_overlay_socket_name(".cursor-overlay-start.lock"));
    assert!(!is_overlay_socket_name(".cursor-overlay-xyz.sock"));
    assert!(!is_overlay_socket_name(".cursor-overlay-.sock"));
    assert!(!is_overlay_socket_name("refstore.lock"));
}

#[test]
fn a_socket_held_by_this_process_or_a_non_agent_desktop_process_is_never_stopped() {
    let dir = std::env::temp_dir().join(format!("ad-sweep-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let socket = dir.join(".cursor-overlay-0123456789abcdef.sock");
    let _ = std::fs::remove_file(&socket);
    let _listener = std::os::unix::net::UnixListener::bind(&socket).unwrap();
    assert_eq!(
        overlay_owner(&socket),
        Owner::Other,
        "our own socket is skipped"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_socket_nobody_listens_on_is_dead_and_only_removed_once_old() {
    let dir = std::env::temp_dir().join(format!("ad-dead-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let socket = dir.join(".cursor-overlay-fedcba9876543210.sock");
    let _ = std::fs::remove_file(&socket);
    drop(std::os::unix::net::UnixListener::bind(&socket).unwrap());
    assert!(socket.exists());
    assert_eq!(overlay_owner(&socket), Owner::Dead);
    assert!(
        !left_behind(&socket),
        "a fresh socket may be about to listen"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_peer_pid_of_a_connected_socket_is_read_from_the_kernel() {
    let (a, _b) = UnixStream::pair().unwrap();
    assert_eq!(peer_pid(&a), Some(std::process::id() as i32));
}

#[test]
fn only_a_session_ended_on_purpose_counts_as_completed() {
    assert!(!session_completed(agent_desktop_core::AMBIENT_CURSOR_ID));
    assert!(!session_completed("no-such-session-for-this-test"));
}
