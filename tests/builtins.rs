//! End-to-end tests for the public built-in detection contracts.
//!
//! These tests exercise the complete public path:
//!
//! `RuleSpec -> ScannerBuilder -> compiled matcher -> capture projection ->
//! validator dispatch -> Finding`
//!
//! Every credential below is synthetic and intentionally unusable.

use std::collections::BTreeSet;

use silens_scan::{Confidence, Scanner, builtins};

fn scanner_for(rules: impl IntoIterator<Item = silens_scan::RuleSpec>) -> Scanner {
    Scanner::builder()
        .builtins(rules)
        .build()
        .expect("built-in rules must compile")
}

fn scan_one(scanner: &Scanner, source: &str) -> silens_scan::ScanResults<&'static str> {
    scanner.scan([("fixture", source)])
}

fn rule_ids(report: &silens_scan::ScanReport) -> BTreeSet<&str> {
    report
        .iter()
        .map(|finding| finding.rule_id().as_str())
        .collect()
}

fn matched<'a>(source: &'a str, finding: &silens_scan::Finding) -> &'a str {
    let location = finding.location();
    &source[location.start()..location.end()]
}

fn contextual_scanner() -> Scanner {
    scanner_for([
        builtins::AWS_SECRET_ACCESS_KEY,
        builtins::AZURE_CLIENT_SECRET,
        builtins::DATABASE_PASSWORD_FIELD,
        builtins::SENSITIVE_HASH,
        builtins::GENERIC_API_KEY,
    ])
}

fn expected_contextual_rule_ids() -> BTreeSet<&'static str> {
    BTreeSet::from([
        "aws.secret-access-key",
        "azure.client-secret",
        "generic.database-password-field",
        "generic.sensitive-hash",
        "generic.api-key",
    ])
}

#[test]
fn deterministic_builtins_detect_realistic_synthetic_values() {
    let mut source = include_str!("fixtures/deterministic.env").to_owned();

    source.push_str("SLACK_BOT_TOKEN=");
    source.push_str("xoxb-");
    source.push_str("1234567890-");
    source.push_str("1234567890-");
    source.push_str("AbCdEfGhIjKlMnOpQrStUvWx\n");

    let scanner = scanner_for([
        builtins::GITHUB_CLASSIC_PAT,
        builtins::STRIPE_LIVE_SECRET_KEY,
        builtins::CLOUDFLARE_USER_API_TOKEN,
        builtins::SLACK_BOT_TOKEN,
        builtins::TELEGRAM_BOT_TOKEN,
        builtins::SIGNED_JWT,
    ]);

    let results = scan_one(&scanner, &source);
    let report = results.single_report().expect("one fixture was scanned");
    let ids = rule_ids(report);

    assert_eq!(report.len(), 6);
    assert!(ids.contains("github.classic-pat"));
    assert!(ids.contains("stripe.live-secret-key"));
    assert!(ids.contains("cloudflare.user-api-token"));
    assert!(ids.contains("slack.bot-token"));
    assert!(ids.contains("telegram.bot-token"));
    assert!(ids.contains("jwt.signed-compact"));

    assert!(
        report
            .iter()
            .all(|finding| finding.confidence() >= Confidence::Medium)
    );
}

#[test]
fn contextual_builtins_project_only_the_secret_value() {
    let source = include_str!("fixtures/contextual.env");
    let scanner = scanner_for([
        builtins::AWS_SECRET_ACCESS_KEY,
        builtins::AZURE_CLIENT_SECRET,
        builtins::DATABASE_PASSWORD_FIELD,
        builtins::SENSITIVE_HASH,
    ]);

    let results = scan_one(&scanner, source);
    let report = results.single_report().expect("one fixture was scanned");
    let ids = rule_ids(report);

    assert_eq!(report.len(), 4);
    assert!(ids.contains("aws.secret-access-key"));
    assert!(ids.contains("azure.client-secret"));
    assert!(ids.contains("generic.database-password-field"));
    assert!(ids.contains("generic.sensitive-hash"));

    for finding in report {
        let value = matched(source, finding);

        assert!(!value.contains('='));
        assert!(!value.contains("AWS_SECRET_ACCESS_KEY"));
        assert!(!value.contains("AZURE_CLIENT_SECRET"));
        assert!(!value.contains("POSTGRES_PASSWORD"));
        assert!(!value.contains("password_hash"));
    }
}

#[test]
fn contextual_builtins_are_equivalent_across_common_config_syntaxes() {
    let scanner = contextual_scanner();
    let expected = expected_contextual_rule_ids();

    for (name, source) in [
        ("env", include_str!("fixtures/contextual.env")),
        ("yaml", include_str!("fixtures/contextual.yaml")),
        ("toml", include_str!("fixtures/contextual.toml")),
        ("json", include_str!("fixtures/contextual.json")),
    ] {
        let results = scan_one(&scanner, source);
        let report = results.single_report().expect("one fixture was scanned");
        let ids = rule_ids(report);

        assert_eq!(
            ids, expected,
            "{name} fixture did not preserve contextual detection semantics"
        );
        assert_eq!(
            report.len(),
            expected.len(),
            "{name} fixture produced an unexpected number of findings"
        );

        for finding in report {
            let value = matched(source, finding);
            assert!(
                !value.contains(['=', ':', '"', '\'']),
                "{name} fixture projected assignment syntax into the finding: {value:?}"
            );
        }
    }
}

#[test]
fn contextual_placeholders_and_unrelated_fields_are_rejected_across_syntaxes() {
    let source = include_str!("fixtures/contextual-false-positives.txt");
    let scanner = scanner_for([
        builtins::AWS_SECRET_ACCESS_KEY,
        builtins::AZURE_CLIENT_SECRET,
        builtins::PASSWORD_FIELD,
        builtins::DATABASE_PASSWORD_FIELD,
        builtins::SENSITIVE_HASH,
        builtins::GENERIC_API_KEY,
        builtins::GENERIC_AUTH_TOKEN,
        builtins::GENERIC_SECRET,
    ]);

    let results = scan_one(&scanner, source);
    let report = results.single_report().expect("one fixture was scanned");

    assert!(
        report.is_empty(),
        "contextual false-positive corpus produced: {:?}",
        report
            .iter()
            .map(|finding| finding.rule_id().as_str())
            .collect::<Vec<_>>()
    );
}

#[test]
fn gcp_json_builtins_detect_projected_fields() {
    let source = include_str!("fixtures/service-account.json");
    let scanner = scanner_for([
        builtins::GCP_PRIVATE_KEY_ID,
        builtins::GCP_CLIENT_SECRET,
        builtins::GCP_PRIVATE_KEY,
    ]);

    let results = scan_one(&scanner, source);
    let report = results.single_report().expect("one fixture was scanned");
    let ids = rule_ids(report);

    assert_eq!(report.len(), 3);
    assert!(ids.contains("gcp.private-key-id"));
    assert!(ids.contains("gcp.client-secret"));
    assert!(ids.contains("gcp.private-key"));

    let private_key = report
        .iter()
        .find(|finding| finding.rule_id().as_str() == "gcp.private-key")
        .expect("private key finding must exist");

    let value = matched(source, private_key);
    assert!(value.starts_with("-----BEGIN PRIVATE KEY-----"));
    assert!(value.ends_with("-----END PRIVATE KEY-----"));
}

#[test]
fn placeholders_and_unrelated_hashes_are_rejected() {
    let source = include_str!("fixtures/false-positives.txt");
    let scanner = scanner_for([
        builtins::GITHUB_CLASSIC_PAT,
        builtins::STRIPE_LIVE_SECRET_KEY,
        builtins::SLACK_BOT_TOKEN,
        builtins::GENERIC_API_KEY,
        builtins::PASSWORD_FIELD,
        builtins::SENSITIVE_HASH,
    ]);

    let results = scan_one(&scanner, source);
    let report = results.single_report().expect("one fixture was scanned");

    assert!(
        report.is_empty(),
        "false-positive fixture produced: {:?}",
        report
            .iter()
            .map(|finding| finding.rule_id().as_str())
            .collect::<Vec<_>>()
    );
}

#[test]
fn findings_are_sorted_and_unicode_locations_remain_correct() {
    let source = concat!(
        "αβγ before\n",
        "STRIPE_SECRET_KEY=sk_live_AbCdEf0123456789_AbCdEf0123456789\n",
        "GITHUB_TOKEN=ghp_AbCdEf0123456789_AbCdEf0123456789\n",
    );

    let scanner = scanner_for([
        builtins::STRIPE_LIVE_SECRET_KEY,
        builtins::GITHUB_CLASSIC_PAT,
    ]);

    let results = scan_one(&scanner, source);
    let report = results.single_report().expect("one fixture was scanned");

    assert_eq!(report.len(), 2);
    assert!(report.findings()[0].location().start() < report.findings()[1].location().start());

    let first = &report.findings()[0];
    assert_eq!(first.rule_id().as_str(), "stripe.live-secret-key");
    assert_eq!(first.location().line(), 2);
    assert_eq!(first.location().column(), 19);
}

#[test]
fn current_pack_is_public_and_compiles_as_one_scanner() {
    let scanner = scanner_for(builtins::CURRENT.iter().copied());

    assert_eq!(scanner.rules_count(), builtins::CURRENT.len());
}

#[test]
fn full_pack_detects_expected_provider_specific_rules() {
    let source = include_str!("fixtures/mixed-config.txt");
    let scanner = scanner_for(builtins::CURRENT.iter().copied());

    let results = scan_one(&scanner, source);
    let report = results.single_report().expect("one fixture was scanned");
    let ids = rule_ids(report);

    // Exact total count is deliberately deferred to the overlap/deduplication
    // milestone. These assertions ensure the provider-specific contracts are
    // present before generic overlaps are normalized.
    assert!(ids.contains("github.classic-pat"));
    assert!(ids.contains("stripe.live-secret-key"));
    assert!(ids.contains("aws.secret-access-key"));
    assert!(ids.contains("azure.client-secret"));
    assert!(ids.contains("generic.database-password-field"));
}
