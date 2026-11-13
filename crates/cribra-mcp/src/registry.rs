//! Bounded request lifecycle tracking.
//!
//! An identifier stays registered from acceptance until its response has been
//! fully written or skipped because of cancellation. This prevents premature
//! identifier reuse while a response is pending in either queue.

use std::collections::HashMap;

use crate::jsonrpc::RequestId;

/// Why a request identifier could not be accepted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AcceptError {
    /// The identifier is still pending.
    Duplicate,
    /// The registry has no capacity for another pending request.
    Full,
}

/// The worker's decision when it picks up a queued request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StartDecision {
    /// Execute the request.
    Execute,
    /// The request was cancelled before execution; no result will be produced.
    Cancelled,
}

/// The writer's decision at the emission boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EmitDecision {
    /// The response may be emitted.
    Emit,
    /// The request was cancelled while queued; no response may be emitted.
    Skip,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EntryState {
    Pending,
    Processing,
    Emitting,
    Cancelled,
}

/// A bounded map from request identifiers to lifecycle state.
#[derive(Debug)]
pub struct Registry {
    entries: HashMap<RequestId, EntryState>,
    capacity: usize,
}

impl Registry {
    /// Creates an empty registry with the supplied capacity.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: HashMap::new(),
            capacity,
        }
    }

    /// Registers an accepted request identifier.
    pub fn accept(&mut self, id: RequestId) -> Result<(), AcceptError> {
        if self.entries.contains_key(&id) {
            return Err(AcceptError::Duplicate);
        }
        if self.entries.len() >= self.capacity {
            return Err(AcceptError::Full);
        }
        self.entries.insert(id, EntryState::Pending);
        Ok(())
    }

    /// Marks a request as being executed.
    ///
    /// A cancellation observed before this point means no result will ever be
    /// produced for the request, so the identifier is released immediately and
    /// execution is skipped.
    pub fn start(&mut self, id: &RequestId) -> StartDecision {
        match self.entries.get_mut(id) {
            Some(EntryState::Cancelled) => {
                self.entries.remove(id);
                StartDecision::Cancelled
            }
            Some(EntryState::Pending) => {
                self.entries.insert(id.clone(), EntryState::Processing);
                StartDecision::Execute
            }
            _ => StartDecision::Execute,
        }
    }

    /// Cancels a request that has not crossed the emission boundary.
    ///
    /// Returns `true` when the cancellation changed the state. Stale, duplicate,
    /// and already-emitting identifiers are ignored as permitted by MCP.
    pub fn cancel(&mut self, id: &RequestId) -> bool {
        match self.entries.get_mut(id) {
            Some(state @ (EntryState::Pending | EntryState::Processing)) => {
                *state = EntryState::Cancelled;
                true
            }
            _ => false,
        }
    }

    /// Claims the emission boundary for a queued response.
    ///
    /// The lock serializes this decision with [`Self::cancel`]. After this
    /// returns [`EmitDecision::Emit`], cancellation is intentionally ignored
    /// because a partially written JSON-RPC line cannot be retracted.
    pub fn begin_emit(&mut self, id: &RequestId) -> EmitDecision {
        match self.entries.get_mut(id) {
            Some(EntryState::Cancelled) => {
                self.entries.remove(id);
                EmitDecision::Skip
            }
            Some(EntryState::Pending | EntryState::Processing) => {
                self.entries.insert(id.clone(), EntryState::Emitting);
                EmitDecision::Emit
            }
            Some(EntryState::Emitting) => EmitDecision::Emit,
            None => EmitDecision::Emit,
        }
    }

    /// Removes a request after its response has been fully written.
    pub fn complete(&mut self, id: &RequestId) {
        self.entries.remove(id);
    }

    /// Removes a request that never entered either response queue.
    pub fn abandon(&mut self, id: &RequestId) {
        self.entries.remove(id);
    }

    /// Returns the number of tracked identifiers.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns whether no identifiers are tracked.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id() -> RequestId {
        RequestId::Number(1)
    }

    #[test]
    fn rejects_duplicate_until_completion() {
        let mut registry = Registry::new(2);
        registry.accept(id()).expect("first acceptance");
        assert_eq!(registry.accept(id()), Err(AcceptError::Duplicate));
        assert_eq!(registry.begin_emit(&id()), EmitDecision::Emit);
        assert_eq!(registry.accept(id()), Err(AcceptError::Duplicate));
        registry.complete(&id());
        registry.accept(id()).expect("reuse after completion");
    }

    #[test]
    fn enforces_capacity() {
        let mut registry = Registry::new(1);
        registry.accept(id()).expect("first acceptance");
        assert_eq!(
            registry.accept(RequestId::Number(2)),
            Err(AcceptError::Full)
        );
    }

    #[test]
    fn cancellation_before_execution_skips_work_and_releases_id() {
        let mut registry = Registry::new(2);
        registry.accept(id()).expect("acceptance");
        assert!(registry.cancel(&id()));
        assert_eq!(registry.start(&id()), StartDecision::Cancelled);
        assert!(registry.is_empty());
        registry
            .accept(id())
            .expect("id reusable because no result exists");
    }

    #[test]
    fn cancellation_during_execution_skips_emission() {
        let mut registry = Registry::new(2);
        registry.accept(id()).expect("acceptance");
        assert_eq!(registry.start(&id()), StartDecision::Execute);
        assert!(registry.cancel(&id()));
        assert_eq!(registry.begin_emit(&id()), EmitDecision::Skip);
    }

    #[test]
    fn cancellation_after_result_enqueue_skips_emission() {
        let mut registry = Registry::new(2);
        registry.accept(id()).expect("acceptance");
        assert_eq!(registry.start(&id()), StartDecision::Execute);
        // The result is already queued here; the writer has not claimed it yet.
        assert!(registry.cancel(&id()));
        assert_eq!(registry.begin_emit(&id()), EmitDecision::Skip);
    }

    #[test]
    fn cancellation_at_emission_boundary_is_ignored() {
        let mut registry = Registry::new(2);
        registry.accept(id()).expect("acceptance");
        assert_eq!(registry.start(&id()), StartDecision::Execute);
        assert_eq!(registry.begin_emit(&id()), EmitDecision::Emit);
        assert!(!registry.cancel(&id()));
        registry.complete(&id());
        assert!(registry.is_empty());
    }

    #[test]
    fn stale_and_duplicate_cancellation_are_ignored() {
        let mut registry = Registry::new(2);
        assert!(!registry.cancel(&RequestId::Number(9)));
        registry.accept(id()).expect("acceptance");
        assert!(registry.cancel(&id()));
        assert!(!registry.cancel(&id()));
    }

    #[test]
    fn stale_cancellation_does_not_poison_a_reused_id() {
        let mut registry = Registry::new(2);
        registry.accept(id()).expect("first acceptance");
        assert_eq!(registry.begin_emit(&id()), EmitDecision::Emit);
        registry.complete(&id());

        // A cancellation that arrives after the response completed is stale.
        assert!(!registry.cancel(&id()));

        registry.accept(id()).expect("reuse after completion");
        assert_eq!(registry.start(&id()), StartDecision::Execute);
        assert_eq!(registry.begin_emit(&id()), EmitDecision::Emit);
    }
}
