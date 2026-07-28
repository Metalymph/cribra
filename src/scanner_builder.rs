use crate::{
    compiled_rule::CompiledRule,
    rule::{Rule, RuleError, RuleSpec},
    scanner::Scanner,
};

#[derive(Debug, Default)]
pub struct ScannerBuilder {
    rules: Vec<Rule>,
    builtin_rules: Vec<RuleSpec>,
}

impl ScannerBuilder {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            rules: Vec::new(),
            builtin_rules: Vec::new(),
        }
    }

    #[must_use]
    pub fn builtin(mut self, rule: RuleSpec) -> Self {
        self.builtin_rules.push(rule);
        self
    }

    #[must_use]
    pub fn builtins<I>(mut self, rules: I) -> Self
    where
        I: IntoIterator<Item = RuleSpec>,
    {
        self.builtin_rules.extend(rules);
        self
    }

    #[must_use]
    pub fn rule(mut self, rule: Rule) -> Self {
        self.rules.push(rule);
        self
    }

    #[must_use]
    pub fn rules<I>(mut self, rules: I) -> Self
    where
        I: IntoIterator<Item = Rule>,
    {
        self.rules.extend(rules);
        self
    }

    pub fn build(mut self) -> Result<Scanner, RuleError> {
        self.rules.reserve(self.builtin_rules.len());

        for spec in self.builtin_rules {
            self.rules.push(spec.to_rule()?);
        }

        let compiled_rules = self.rules.into_iter().map(CompiledRule::compile).collect();

        Ok(Scanner::new(compiled_rules))
    }
}
