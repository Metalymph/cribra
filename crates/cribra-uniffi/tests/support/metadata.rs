//! Source-free direct-Core oracle projections; no adapter DTO conversions.
use serde_json::{Value, json};

pub fn explanation(value: cribra::Explanation) -> Value {
    match value {
        cribra::Explanation::Classified(mode) => json!({"classified": format!("{mode:?}")}),
        cribra::Explanation::Ambiguous(evidence) => json!({"ambiguous": format!("{evidence:?}")}),
        _ => panic!("unhandled Core explanation"),
    }
}

pub fn report(
    scanner: &cribra::Scanner,
    source_id: &str,
    source_bytes: usize,
    report: &cribra::ScanReport,
) -> Value {
    let findings: Vec<Value> = report.findings().iter().map(|finding| {
        let location = finding.location();
        json!({"ruleId":finding.rule_id().as_str(),"start":location.start(),"end":location.end(),
            "line":location.line(),"column":location.column(),"severity":finding.severity().to_string(),
            "confidence":finding.confidence().to_string(),
            "remediation":finding.remediation().map(|r| format!("{r:?}")),
            "explanation":finding.explanation(scanner).map(explanation)})
    }).collect();
    let candidates: Vec<Value> = report.candidates().iter().map(|candidate| {
        let location = candidate.location();
        json!({"kind":format!("{:?}", candidate.kind()),"start":location.start(),"end":location.end(),
            "line":location.line(),"column":location.column(),"evidence":format!("{:?}", candidate.evidence()),
            "explanation":explanation(candidate.explanation())})
    }).collect();
    json!({"sourceId":source_id,"sourceBytes":source_bytes,"findings":findings,"candidates":candidates,
        "candidateCount":report.candidate_len(),"needsReview":report.needs_review()})
}
