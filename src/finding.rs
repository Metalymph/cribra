use crate::{confidence::Confidence, location::Location, rule::RuleId, severity::Severity};

/// A validated finding produced by a scanner rule.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Finding {
    rule_id: RuleId,
    location: Location,
    severity: Severity,
    confidence: Confidence,
}

impl Finding {
    pub(crate) const fn new(
        rule_id: RuleId,
        location: Location,
        severity: Severity,
        confidence: Confidence,
    ) -> Self {
        Self {
            rule_id,
            location,
            severity,
            confidence,
        }
    }

    #[must_use]
    pub const fn rule_id(&self) -> &RuleId {
        &self.rule_id
    }

    #[must_use]
    pub const fn location(&self) -> &Location {
        &self.location
    }

    pub(crate) const fn location_mut(&mut self) -> &mut Location {
        &mut self.location
    }

    #[must_use]
    pub const fn severity(&self) -> Severity {
        self.severity
    }

    #[must_use]
    pub const fn confidence(&self) -> Confidence {
        self.confidence
    }
}
