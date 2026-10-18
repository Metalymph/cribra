#![cfg(feature = "core")]

use cribra_uniffi::{Finding, cribra_version, smoke_scan_text};

#[test]
fn version_is_from_core_manifest_not_adapter_package() {
    let core: toml::Table = include_str!("../../../Cargo.toml").parse().unwrap();
    assert_eq!(
        cribra_version(),
        core["package"]["version"].as_str().unwrap()
    );
    assert_ne!(cribra_version(), env!("CARGO_PKG_VERSION"));
}

#[test]
fn smoke_scan_projects_the_default_core_scanner() {
    let token = "ghp_AbCdEf0123456789_AbCdEf0123456789";
    let source = format!("前\r\né🙂e\u{301} GITHUB_TOKEN={token}\n");
    let scanner = cribra::Scanner::default();
    let results = scanner.scan([("smoke", source.as_str())]);
    let expected: Vec<Finding> = results
        .single_report()
        .unwrap()
        .findings()
        .iter()
        .map(Finding::from)
        .collect();
    let actual = smoke_scan_text(source);
    assert_eq!(actual, expected);
    assert_eq!(actual.len(), 1);
    assert_eq!(actual[0].rule_id.value, "github.classic-pat");
    assert_eq!(actual[0].location.start, 28);
    assert_eq!(actual[0].location.end, 28 + token.len() as u64);
    assert_eq!(actual[0].location.line, 2);
    assert_eq!(actual[0].location.column, 19);
}

#[test]
fn smoke_scan_accepts_empty_and_clean_text() {
    assert!(smoke_scan_text(String::new()).is_empty());
    assert!(smoke_scan_text("ordinary text: é🙂".to_owned()).is_empty());
}
