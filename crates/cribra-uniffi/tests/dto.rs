#![cfg(feature = "core")]

use cribra::{Rule, Scanner};
use cribra_uniffi::{Confidence, Finding, Location, RuleId, Severity};

#[test]
fn converts_every_severity_and_confidence_variant() {
    for (core, dto) in [
        (cribra::Severity::Info, Severity::Info),
        (cribra::Severity::Low, Severity::Low),
        (cribra::Severity::Medium, Severity::Medium),
        (cribra::Severity::High, Severity::High),
        (cribra::Severity::Critical, Severity::Critical),
    ] {
        assert_eq!(Severity::from(core), dto);
    }
    for (core, dto) in [
        (cribra::Confidence::Low, Confidence::Low),
        (cribra::Confidence::Medium, Confidence::Medium),
        (cribra::Confidence::High, Confidence::High),
    ] {
        assert_eq!(Confidence::from(core), dto);
    }
}

#[test]
fn rule_identifier_is_owned_and_preserved_verbatim() {
    let dto = {
        let core = cribra::RuleId::from("custom/clé-🔑");
        RuleId::from(&core)
    };
    assert_eq!(dto.value, "custom/clé-🔑");
}

#[test]
fn finding_preserves_core_metadata_after_source_and_report_are_dropped() {
    let dto = {
        let scanner = Scanner::builder()
            .rule(Rule::literal("example", "secret", cribra::Severity::High))
            .build()
            .unwrap();
        let source = String::from("secret");
        let results = scanner.scan([("memory", source.as_str())]);
        let finding = &results.single_report().unwrap().findings()[0];
        let dto = Finding::from(finding);
        assert_eq!(dto.confidence, Confidence::from(finding.confidence()));
        assert_eq!(dto.location, Location::from(finding.location()));
        dto
    };
    assert_eq!(dto.rule_id.value, "example");
    assert_eq!(dto.severity, Severity::High);
    assert_eq!(
        dto.location,
        Location {
            start: 0,
            end: 6,
            line: 1,
            column: 1
        }
    );
}

#[test]
fn coordinates_preserve_utf8_half_open_spans_and_scalar_columns() {
    let scanner = Scanner::builder()
        .rule(Rule::literal("unicode", "秘密🔑", cribra::Severity::Low))
        .build()
        .unwrap();

    // CRLF, combining marks and a non-BMP scalar distinguish byte offsets,
    // scalar columns, UTF-16 code units and grapheme-cluster columns.
    for (source, start, line, column) in [
        ("秘密🔑", 0, 1, 1),
        ("é🙂e\u{301}秘密🔑", 9, 1, 5),
        ("前\r\né🙂e\u{301}秘密🔑", 14, 2, 5),
        ("\n秘密🔑\n", 1, 2, 1),
    ] {
        let results = scanner.scan([("memory", source)]);
        let findings = results.single_report().unwrap().findings();
        assert_eq!(findings.len(), 1);
        let dto = Location::from(findings[0].location());
        assert_eq!(
            dto,
            Location {
                start,
                end: start + 10,
                line,
                column
            }
        );
        let start = usize::try_from(dto.start).unwrap();
        let end = usize::try_from(dto.end).unwrap();
        assert!(source.is_char_boundary(start));
        assert!(source.is_char_boundary(end));
        assert_eq!(&source[start..end], "秘密🔑");
    }
}
