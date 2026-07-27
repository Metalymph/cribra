/// The severity level of a finding.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum Severity {
    Low,
    Medium,
    High,
    Critical,
}
