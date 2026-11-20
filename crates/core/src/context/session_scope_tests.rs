use super::was_started;

#[test]
fn an_id_from_session_start_is_recognised() {
    assert!(was_started("run-1795192181592-78259-0"));
}

#[test]
fn a_chosen_namespace_name_is_not_mistaken_for_a_started_session() {
    for name in [
        "dev",
        "run",
        "run-1",
        "run-a-b-c",
        "run-1-2-3-4",
        "agent-run-1-2-3",
        "run--2-3",
    ] {
        assert!(!was_started(name), "{name}");
    }
}
