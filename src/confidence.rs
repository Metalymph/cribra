/// The confidence level of a finding.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum Confidence {
    Low,
    Medium,
    High,
}
