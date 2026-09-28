use std::sync::Arc;

use crate::compiled_rule::CompiledRuleSet;

/// Mutable execution state owned by one logical source.
///
/// Streaming execution state belongs here rather than in the shared compiled
/// scanner configuration. Later streaming slices extend this boundary with the
/// concrete matcher, validator, candidate, normalization, and location state
/// they require.
#[derive(Debug, Default)]
struct SourceExecutionState {
    accepted_bytes: usize,
}

/// Mutable execution state for one logical source.
///
/// A source session owns all state that may change while processing a single
/// logical source. The compiled rule set is shared immutable configuration and
/// must never contain source-local execution state.
#[derive(Debug)]
pub(crate) struct SourceSession {
    rules: Arc<CompiledRuleSet>,
    lifecycle: SourceLifecycle,
    state: SourceExecutionState,
}

/// Lifecycle of one logical-source execution.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) enum SourceLifecycle {
    Active,
    Completed,
    Failed,
}

/// Invalid mutation of a source session that is no longer active.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) enum SourceSessionStateError {
    NotActive,
}

impl SourceSession {
    pub(crate) fn new(rules: Arc<CompiledRuleSet>) -> Self {
        Self {
            rules,
            lifecycle: SourceLifecycle::Active,
            state: SourceExecutionState::default(),
        }
    }

    pub(crate) fn accepted_bytes(&self) -> usize {
        self.state.accepted_bytes
    }

    pub(crate) fn lifecycle(&self) -> SourceLifecycle {
        self.lifecycle
    }

    pub(crate) fn accept_bytes(&mut self, count: usize) -> Result<(), SourceSessionStateError> {
        self.ensure_active()?;
        self.state.accepted_bytes += count;
        Ok(())
    }

    pub(crate) fn complete(&mut self) -> Result<(), SourceSessionStateError> {
        self.ensure_active()?;
        self.lifecycle = SourceLifecycle::Completed;
        Ok(())
    }

    pub(crate) fn fail(&mut self) -> Result<(), SourceSessionStateError> {
        self.ensure_active()?;
        self.lifecycle = SourceLifecycle::Failed;
        Ok(())
    }

    fn ensure_active(&self) -> Result<(), SourceSessionStateError> {
        if self.lifecycle == SourceLifecycle::Active {
            Ok(())
        } else {
            Err(SourceSessionStateError::NotActive)
        }
    }

    #[cfg(test)]
    pub(crate) fn shares_rules_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.rules, &other.rules)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Scanner;

    #[test]
    fn sessions_share_immutable_compiled_configuration() {
        let scanner = Scanner::default();

        let first = scanner.source_session();
        let second = scanner.source_session();

        assert!(first.shares_rules_with(&second));
    }

    #[test]
    fn sessions_keep_mutable_state_independent() {
        let scanner = Scanner::default();

        let mut first = scanner.source_session();
        let second = scanner.source_session();

        first.accept_bytes(17).unwrap();

        assert_eq!(first.accepted_bytes(), 17);
        assert_eq!(second.accepted_bytes(), 0);
        assert_eq!(first.lifecycle(), SourceLifecycle::Active);
        assert_eq!(second.lifecycle(), SourceLifecycle::Active);
    }

    #[test]
    fn abandoned_session_does_not_affect_later_session() {
        let scanner = Scanner::default();

        {
            let mut abandoned = scanner.source_session();
            abandoned.accept_bytes(23).unwrap();
        }

        let later = scanner.source_session();

        assert_eq!(later.accepted_bytes(), 0);
        assert_eq!(later.lifecycle(), SourceLifecycle::Active);
    }

    #[test]
    fn session_can_reach_each_terminal_lifecycle() {
        let scanner = Scanner::default();

        let mut completed = scanner.source_session();
        completed.complete().unwrap();

        let mut failed = scanner.source_session();
        failed.fail().unwrap();

        assert_eq!(completed.lifecycle(), SourceLifecycle::Completed);
        assert_eq!(failed.lifecycle(), SourceLifecycle::Failed);
    }

    #[test]
    fn completed_session_rejects_further_mutation() {
        let scanner = Scanner::default();
        let mut session = scanner.source_session();

        session.accept_bytes(11).unwrap();
        session.complete().unwrap();

        assert_eq!(
            session.accept_bytes(7),
            Err(SourceSessionStateError::NotActive)
        );
        assert_eq!(session.complete(), Err(SourceSessionStateError::NotActive));
        assert_eq!(session.fail(), Err(SourceSessionStateError::NotActive));
        assert_eq!(session.accepted_bytes(), 11);
        assert_eq!(session.lifecycle(), SourceLifecycle::Completed);
    }

    #[test]
    fn failed_session_rejects_further_mutation() {
        let scanner = Scanner::default();
        let mut session = scanner.source_session();

        session.accept_bytes(13).unwrap();
        session.fail().unwrap();

        assert_eq!(
            session.accept_bytes(5),
            Err(SourceSessionStateError::NotActive)
        );
        assert_eq!(session.complete(), Err(SourceSessionStateError::NotActive));
        assert_eq!(session.fail(), Err(SourceSessionStateError::NotActive));
        assert_eq!(session.accepted_bytes(), 13);
        assert_eq!(session.lifecycle(), SourceLifecycle::Failed);
    }

    #[test]
    fn completing_one_session_does_not_affect_another_active_session() {
        let scanner = Scanner::default();

        let mut first = scanner.source_session();
        let mut second = scanner.source_session();

        first.accept_bytes(11).unwrap();
        second.accept_bytes(7).unwrap();
        first.accept_bytes(13).unwrap();
        second.accept_bytes(5).unwrap();

        assert_eq!(first.accepted_bytes(), 24);
        assert_eq!(second.accepted_bytes(), 12);

        first.complete().unwrap();

        assert_eq!(first.lifecycle(), SourceLifecycle::Completed);
        assert_eq!(second.lifecycle(), SourceLifecycle::Active);

        second.accept_bytes(3).unwrap();

        assert_eq!(first.accepted_bytes(), 24);
        assert_eq!(second.accepted_bytes(), 15);
    }

    #[test]
    fn failing_one_session_does_not_affect_another_active_session() {
        let scanner = Scanner::default();

        let mut first = scanner.source_session();
        let mut second = scanner.source_session();

        first.accept_bytes(19).unwrap();
        second.accept_bytes(5).unwrap();

        first.fail().unwrap();

        assert_eq!(first.lifecycle(), SourceLifecycle::Failed);
        assert_eq!(second.lifecycle(), SourceLifecycle::Active);

        second.accept_bytes(7).unwrap();

        assert_eq!(first.accepted_bytes(), 19);
        assert_eq!(second.accepted_bytes(), 12);
    }
}
