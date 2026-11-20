#[derive(Debug)]
pub(super) struct SessionScope {
    pub(super) id: String,
    pub(super) lease: Option<crate::session::SessionLivenessLease>,
}

impl Clone for SessionScope {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            lease: self.lease.clone(),
        }
    }
}

impl SessionScope {
    pub(super) fn ensure_active(&self) -> Result<(), crate::AppError> {
        let manifest = crate::session::read_manifest(&self.id)?;
        if manifest.is_none() && was_started(&self.id) {
            return Err(expired(&self.id));
        }
        if manifest.is_some_and(|manifest| manifest.ended_at.is_some()) {
            return Err(crate::AdapterError::new(
                crate::ErrorCode::InvalidArgs,
                format!("Session '{}' has ended", self.id),
            )
            .with_suggestion("Unset AGENT_DESKTOP_SESSION and omit --session, then run the command or start a new session")
            .with_details(serde_json::json!({ "kind": "session_ended", "session_id": self.id }))
            .with_disposition(crate::DeliverySemantics::not_delivered())
            .into());
        }
        Ok(())
    }
}

/// Whether `id` has the shape `session start` gives every session it opens,
/// as opposed to a bare `--session` name an agent chose for a namespace.
pub(super) fn was_started(id: &str) -> bool {
    let mut parts = id.split('-');
    parts.next() == Some("run")
        && parts.clone().count() == 3
        && parts.all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
}

/// A started session whose state is gone has expired or was removed, so the
/// command is refused rather than run in a fresh namespace with the same id.
fn expired(id: &str) -> crate::AppError {
    crate::AdapterError::new(
        crate::ErrorCode::InvalidArgs,
        format!("Session '{id}' has expired or was removed"),
    )
    .with_suggestion(
        "Start a new session with `agent-desktop session start`, then pass its id with --session",
    )
    .with_details(serde_json::json!({ "kind": "session_expired", "session_id": id }))
    .with_disposition(crate::DeliverySemantics::not_delivered())
    .into()
}

impl super::CommandContext {
    /// Allows repeatable session teardown while ordinary commands remain refused.
    pub fn session_end_scope(&self) -> Result<super::CommandScope<'_>, crate::AppError> {
        self.command_scope_with_disposition(
            "session",
            crate::DeliverySemantics::delivered_unverified(),
            true,
        )
    }
}

#[cfg(test)]
#[path = "session_scope_tests.rs"]
mod tests;
