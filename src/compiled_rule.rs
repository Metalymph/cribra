use regex::Regex;

use crate::{
    rule::{Matcher, Rule, RuleId},
    severity::Severity,
};

#[derive(Debug)]
pub(crate) struct CompiledRule {
    id: RuleId,
    severity: Severity,
    matcher: CompiledMatcher,
}

#[derive(Debug)]
pub(crate) enum CompiledMatcher {
    Literal(Box<str>),
    Prefix(Box<str>),
    Suffix(Box<str>),
    Pattern(Regex),
}

impl CompiledRule {
    #[must_use]
    pub(crate) fn compile(rule: Rule) -> Self {
        let Rule {
            id,
            severity,
            matcher,
        } = rule;

        let matcher = match matcher {
            Matcher::Literal(value) => CompiledMatcher::Literal(value),
            Matcher::Prefix(value) => CompiledMatcher::Prefix(value),
            Matcher::Suffix(value) => CompiledMatcher::Suffix(value),
            Matcher::Pattern(pattern) => CompiledMatcher::Pattern(pattern),
        };

        Self {
            id,
            severity,
            matcher,
        }
    }

    #[must_use]
    pub(crate) const fn id(&self) -> &RuleId {
        &self.id
    }

    #[must_use]
    pub(crate) const fn severity(&self) -> Severity {
        self.severity
    }

    #[must_use]
    pub(crate) const fn matcher(&self) -> &CompiledMatcher {
        &self.matcher
    }
}
