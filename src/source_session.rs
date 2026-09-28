use std::sync::Arc;

use crate::compiled_rule::CompiledRuleSet;

/// Mutable execution state for one logical source.
///
/// A source session owns all state that may change while processing a single
/// logical source. The compiled rule set is shared immutable configuration and
/// must never contain source-local execution state.
#[derive(Debug)]
pub(crate) struct SourceSession {
    rules: Arc<CompiledRuleSet>,
    lifecycle: SourceLifecycle,
    accepted_bytes: usize,
}

/// Lifecycle of one logical-source execution.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) enum SourceLifecycle {
    Active,
    Completed,
    Failed,
}

impl SourceSession {
    pub(crate) fn new(rules: Arc<CompiledRuleSet>) -> Self {
        Self {
            rules,
            lifecycle: SourceLifecycle::Active,
            accepted_bytes: 0,
        }
    }

    pub(crate) fn accepted_bytes(&self) -> usize {
        self.accepted_bytes
    }

    pub(crate) fn lifecycle(&self) -> SourceLifecycle {
        self.lifecycle
    }

    pub(crate) fn accept_bytes(&mut self, count: usize) {
        debug_assert_eq!(self.lifecycle, SourceLifecycle::Active);
        self.accepted_bytes += count;
    }

    pub(crate) fn complete(&mut self) {
        debug_assert_eq!(self.lifecycle, SourceLifecycle::Active);
        self.lifecycle = SourceLifecycle::Completed;
    }

    pub(crate) fn fail(&mut self) {
        debug_assert_eq!(self.lifecycle, SourceLifecycle::Active);
        self.lifecycle = SourceLifecycle::Failed;
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

        first.accept_bytes(17);

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
            abandoned.accept_bytes(23);
        }

        let later = scanner.source_session();

        assert_eq!(later.accepted_bytes(), 0);
        assert_eq!(later.lifecycle(), SourceLifecycle::Active);
    }

    #[test]
    fn session_can_reach_each_terminal_lifecycle() {
        let scanner = Scanner::default();

        let mut completed = scanner.source_session();
        completed.complete();

        let mut failed = scanner.source_session();
        failed.fail();

        assert_eq!(completed.lifecycle(), SourceLifecycle::Completed);
        assert_eq!(failed.lifecycle(), SourceLifecycle::Failed);
    }
}