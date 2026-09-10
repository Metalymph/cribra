//! End-to-end tests for the public built-in detection contracts.
//!
//! These tests exercise the complete public path:
//!
//! `RuleSpec -> ScannerBuilder -> compiled matcher -> capture projection ->
//! validator dispatch -> Finding`
//!
//! Every credential below is synthetic and intentionally unusable.

use std::collections::BTreeSet;

use cribra::{Confidence, Remediation, Scanner, Severity, builtins};

fn scanner_for(rules: impl IntoIterator<Item = cribra::RuleSpec>) -> Scanner {
    Scanner::builder()
        .builtins(rules)
        .build()
        .expect("built-in rules must compile")
}

fn scan_one(scanner: &Scanner, source: &str) -> cribra::ScanResults<&'static str> {
    scanner.scan([("fixture", source)])
}

fn rule_ids(report: &cribra::ScanReport) -> BTreeSet<&str> {
    report
        .iter()
        .map(|finding| finding.rule_id().as_str())
        .collect()
}

fn matched<'a>(source: &'a str, finding: &cribra::Finding) -> &'a str {
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
fn gitlab_builtins_detect_supported_standard_prefixes() {
    const BODY: &str = "AbCdEf0123456789_AbCdEf0123456789";

    let cases = [
        (
            builtins::GITLAB_ACCESS_TOKEN,
            "gitlab.access-token",
            "glpat-",
        ),
        (
            builtins::GITLAB_OAUTH_APPLICATION_SECRET,
            "gitlab.oauth-application-secret",
            "gloas-",
        ),
        (
            builtins::GITLAB_DEPLOY_TOKEN,
            "gitlab.deploy-token",
            "gldt-",
        ),
        (
            builtins::GITLAB_RUNNER_AUTH_TOKEN,
            "gitlab.runner-auth-token",
            "glrt-",
        ),
        (
            builtins::GITLAB_REGISTRATION_DERIVED_RUNNER_AUTH_TOKEN,
            "gitlab.registration-derived-runner-auth-token",
            "glrtr-",
        ),
        (
            builtins::GITLAB_CI_JOB_TOKEN,
            "gitlab.ci-job-token",
            "glcbt-",
        ),
        (
            builtins::GITLAB_TRIGGER_TOKEN,
            "gitlab.trigger-token",
            "glptt-",
        ),
        (builtins::GITLAB_FEED_TOKEN, "gitlab.feed-token", "glft-"),
        (
            builtins::GITLAB_INCOMING_MAIL_TOKEN,
            "gitlab.incoming-mail-token",
            "glimt-",
        ),
        (
            builtins::GITLAB_AGENT_TOKEN,
            "gitlab.agent-token",
            "glagent-",
        ),
        (
            builtins::GITLAB_WORKSPACE_TOKEN,
            "gitlab.workspace-token",
            "glwt-",
        ),
        (builtins::GITLAB_SCIM_TOKEN, "gitlab.scim-token", "glsoat-"),
        (
            builtins::GITLAB_FEATURE_FLAG_CLIENT_TOKEN,
            "gitlab.feature-flag-client-token",
            "glffct-",
        ),
    ];

    for (rule, expected_id, prefix) in cases {
        let token = format!("{prefix}{BODY}");
        let scanner = scanner_for([rule]);
        let results = scanner.scan([("fixture", token.as_str())]);
        let report = results.single_report().expect("one fixture was scanned");

        assert_eq!(report.len(), 1, "{expected_id} was not detected");

        let finding = &report.findings()[0];

        assert_eq!(finding.rule_id().as_str(), expected_id);
        assert_eq!(matched(&token, finding), token);
        assert_eq!(finding.severity(), Severity::Critical);
        assert_eq!(finding.confidence(), Confidence::High);
        assert_eq!(
            finding.remediation(),
            Some(Remediation::RevokeAndRotateCredential)
        );
    }
}

#[test]
fn gitlab_builtins_reject_placeholders_short_values_and_unknown_prefixes() {
    let scanner = scanner_for([
        builtins::GITLAB_ACCESS_TOKEN,
        builtins::GITLAB_OAUTH_APPLICATION_SECRET,
        builtins::GITLAB_DEPLOY_TOKEN,
        builtins::GITLAB_RUNNER_AUTH_TOKEN,
        builtins::GITLAB_REGISTRATION_DERIVED_RUNNER_AUTH_TOKEN,
        builtins::GITLAB_CI_JOB_TOKEN,
        builtins::GITLAB_TRIGGER_TOKEN,
        builtins::GITLAB_FEED_TOKEN,
        builtins::GITLAB_INCOMING_MAIL_TOKEN,
        builtins::GITLAB_AGENT_TOKEN,
        builtins::GITLAB_WORKSPACE_TOKEN,
        builtins::GITLAB_SCIM_TOKEN,
        builtins::GITLAB_FEATURE_FLAG_CLIENT_TOKEN,
    ]);

    for source in [
        "glpat-your_token_here",
        "gloas-example",
        "gldt-too",
        "glrt-placeholder",
        "glrtr-token_here",
        "glcbt-example_token",
        "glptt-test_token",
        "glft-placeholder",
        "glimt-placeholder",
        "glagent-example",
        "glwt-token_here",
        "glsoat-placeholder",
        "glffct-example",
        "glxyz-AbCdEf0123456789_AbCdEf0123456789",
        "company_pat_AbCdEf0123456789_AbCdEf0123456789",
    ] {
        let results = scan_one(&scanner, source);
        let report = results.single_report().expect("one fixture was scanned");

        assert!(
            report.is_empty(),
            "invalid GitLab candidate unexpectedly produced findings for {source:?}: {:?}",
            report
                .iter()
                .map(|finding| finding.rule_id().as_str())
                .collect::<Vec<_>>()
        );
    }
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
    let scanner = scanner_for([builtins::GCP_PRIVATE_KEY_ID, builtins::GCP_PRIVATE_KEY]);

    let results = scan_one(&scanner, source);
    let report = results.single_report().expect("one fixture was scanned");
    let ids = rule_ids(report);

    assert_eq!(report.len(), 2);
    assert!(ids.contains("gcp.private-key-id"));
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
fn bare_google_style_client_secret_is_detected_generically() {
    let source = r#"{
        "type": "authorized_user",
        "client_id": "example.apps.googleusercontent.com",
        "client_secret": "AbCdEfGhIjKlMnOpQrStUvWxYz012345"
    }"#;

    let scanner = Scanner::default();
    let results = scanner.scan([("fixture", source)]);
    let report = results.single_report().expect("one fixture was scanned");

    let matching = report
        .findings()
        .iter()
        .filter(|finding| {
            let location = finding.location();
            &source[location.start()..location.end()] == "AbCdEfGhIjKlMnOpQrStUvWxYz012345"
        })
        .collect::<Vec<_>>();

    assert_eq!(matching.len(), 1);
    assert_eq!(matching[0].rule_id().as_str(), "generic.secret");
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

#[test]
fn private_key_builtins_detect_complete_pem_blocks() {
    let scanner = scanner_for([
        builtins::PKCS8_PRIVATE_KEY,
        builtins::ENCRYPTED_PRIVATE_KEY,
        builtins::RSA_PRIVATE_KEY,
        builtins::EC_PRIVATE_KEY,
        builtins::OPENSSH_PRIVATE_KEY,
    ]);

    let source = concat!(
        "-----BEGIN PRIVATE KEY-----\n",
        "MIIEvQIBADANBgkqhkiG9w0BAQEFAASC0123456789ABCDEF\n",
        "-----END PRIVATE KEY-----\n",
        "\n",
        "-----BEGIN ENCRYPTED PRIVATE KEY-----\n",
        "MIIE6TAbBgkqhkiG9w0BBQMwDgQI0123456789ABCDEFGH\n",
        "-----END ENCRYPTED PRIVATE KEY-----\n",
        "\n",
        "-----BEGIN RSA PRIVATE KEY-----\n",
        "MIIEowIBAAKCAQEA0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ\n",
        "-----END RSA PRIVATE KEY-----\n",
        "\n",
        "-----BEGIN EC PRIVATE KEY-----\n",
        "MHQCAQEEIB0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ\n",
        "-----END EC PRIVATE KEY-----\n",
        "\n",
        "-----BEGIN OPENSSH PRIVATE KEY-----\n",
        "b3BlbnNzaC1rZXktdjEAAAAA0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ\n",
        "-----END OPENSSH PRIVATE KEY-----",
    );

    let results = scanner.scan([("pem", source)]);
    let report = results.single_report().expect("one source");
    let ids = report
        .findings()
        .iter()
        .map(|finding| finding.rule_id().as_str())
        .collect::<Vec<_>>();

    assert_eq!(report.len(), 5);
    assert!(ids.contains(&"generic.pkcs8-private-key"));
    assert!(ids.contains(&"generic.encrypted-private-key"));
    assert!(ids.contains(&"generic.rsa-private-key"));
    assert!(ids.contains(&"generic.ec-private-key"));
    assert!(ids.contains(&"generic.openssh-private-key"));

    assert!(
        report
            .findings()
            .iter()
            .all(|finding| finding.severity() == Severity::Critical)
    );
    assert!(
        report
            .findings()
            .iter()
            .all(|finding| finding.remediation() == Some(Remediation::ReplacePrivateKey))
    );
}

#[test]
fn private_key_builtins_reject_incomplete_or_mismatched_pem_blocks() {
    let scanner = scanner_for([
        builtins::PKCS8_PRIVATE_KEY,
        builtins::ENCRYPTED_PRIVATE_KEY,
        builtins::RSA_PRIVATE_KEY,
        builtins::EC_PRIVATE_KEY,
        builtins::OPENSSH_PRIVATE_KEY,
    ]);

    for source in [
        "-----BEGIN PRIVATE KEY-----",
        "-----BEGIN PRIVATE KEY-----\nshort\n-----END PRIVATE KEY-----",
        "-----BEGIN PRIVATE KEY-----\n0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ\n-----END RSA PRIVATE KEY-----",
        "-----BEGIN PUBLIC KEY-----\n0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ\n-----END PUBLIC KEY-----",
        "-----BEGIN CERTIFICATE-----\n0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ\n-----END CERTIFICATE-----",
    ] {
        let results = scanner.scan([("near-miss", source)]);
        assert!(
            results.single_report().expect("one source").is_empty(),
            "unexpected private-key finding for {source:?}",
        );
    }
}

#[test]
fn gcp_private_key_outranks_generic_pkcs8_for_the_same_span() {
    let scanner = scanner_for([builtins::PKCS8_PRIVATE_KEY, builtins::GCP_PRIVATE_KEY]);

    let pem = concat!(
        "-----BEGIN PRIVATE KEY-----\n",
        "MIIEvQIBADANBgkqhkiG9w0BAQEFAASC0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ\n",
        "-----END PRIVATE KEY-----",
    );
    let source = format!(r#""private_key": "{pem}""#);

    let results = scanner.scan([("gcp", source.as_str())]);
    let report = results.single_report().expect("one source");

    assert_eq!(report.len(), 1);
    assert_eq!(report.findings()[0].rule_id().as_str(), "gcp.private-key");
}

#[test]
fn gcp_json_builtin_detects_escaped_private_key() {
    let scanner = scanner_for([builtins::GCP_ESCAPED_PRIVATE_KEY]);

    let value = concat!(
        "-----BEGIN PRIVATE KEY-----\\n",
        "MIIEvQIBADANBgkqhkiG9w0BAQEFAASC0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ\\n",
        "-----END PRIVATE KEY-----\\n",
    );

    let source = format!(r#"{{"type":"service_account","private_key":"{value}"}}"#);

    let results = scanner.scan([("gcp", source.as_str())]);
    let report = results.single_report().expect("one source");

    assert_eq!(report.len(), 1);

    let finding = &report.findings()[0];
    let location = finding.location();

    assert_eq!(finding.rule_id().as_str(), "gcp.escaped-private-key");
    assert_eq!(&source[location.start()..location.end()], value);
    assert_eq!(finding.severity(), Severity::Critical);
    assert_eq!(finding.remediation(), Some(Remediation::ReplacePrivateKey),);
}

#[test]
fn gcp_private_key_rules_preserve_raw_and_escaped_source_semantics() {
    let scanner = scanner_for([builtins::GCP_PRIVATE_KEY, builtins::GCP_ESCAPED_PRIVATE_KEY]);

    let raw_value = concat!(
        "-----BEGIN PRIVATE KEY-----\n",
        "MIIEvQIBADANBgkqhkiG9w0BAQEFAASC0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ\n",
        "-----END PRIVATE KEY-----",
    );
    let raw_source = format!(r#""private_key": "{raw_value}""#);

    let escaped_value = concat!(
        "-----BEGIN PRIVATE KEY-----\\n",
        "MIIEvQIBADANBgkqhkiG9w0BAQEFAASC0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ\\n",
        "-----END PRIVATE KEY-----\\n",
    );
    let escaped_source = format!(r#""private_key": "{escaped_value}""#);

    let raw_results = scanner.scan([("raw", raw_source.as_str())]);
    let raw_report = raw_results.single_report().expect("one raw source");

    assert_eq!(raw_report.len(), 1);
    assert_eq!(
        raw_report.findings()[0].rule_id().as_str(),
        "gcp.private-key",
    );

    let escaped_results = scanner.scan([("escaped", escaped_source.as_str())]);
    let escaped_report = escaped_results.single_report().expect("one escaped source");

    assert_eq!(escaped_report.len(), 1);
    assert_eq!(
        escaped_report.findings()[0].rule_id().as_str(),
        "gcp.escaped-private-key",
    );
}

#[test]
fn gcp_escaped_private_key_rejects_incomplete_or_unrelated_values() {
    let scanner = scanner_for([builtins::GCP_ESCAPED_PRIVATE_KEY]);

    for source in [
        r#""private_key": "-----BEGIN PRIVATE KEY-----\\nshort""#,
        r#""private_key": "-----BEGIN PRIVATE KEY-----\\n0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ\\n-----END RSA PRIVATE KEY-----\\n""#,
        r#""other_key": "-----BEGIN PRIVATE KEY-----\\n0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ\\n-----END PRIVATE KEY-----\\n""#,
        r#""private_key": "-----BEGIN PUBLIC KEY-----\\n0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ\\n-----END PUBLIC KEY-----\\n""#,
    ] {
        let results = scanner.scan([("near-miss", source)]);
        let report = results.single_report().expect("one source");

        assert!(
            report.is_empty(),
            "unexpected escaped GCP private-key finding for {source:?}",
        );
    }
}

#[test]
fn database_connection_password_detects_supported_uri_schemes() {
    let scanner = scanner_for([builtins::DATABASE_CONNECTION_PASSWORD]);

    for (source, expected) in [
        (
            "DATABASE_URL=postgres://alice:CorrectHorseBatteryStaple@localhost/app",
            "CorrectHorseBatteryStaple",
        ),
        (
            "DATABASE_URL=postgresql://alice:CorrectHorseBatteryStaple@localhost/app",
            "CorrectHorseBatteryStaple",
        ),
        (
            "DATABASE_URL=mysql://alice:CorrectHorseBatteryStaple@localhost/app",
            "CorrectHorseBatteryStaple",
        ),
        (
            "DATABASE_URL=mariadb://alice:CorrectHorseBatteryStaple@localhost/app",
            "CorrectHorseBatteryStaple",
        ),
        (
            "DATABASE_URL=mongodb://alice:CorrectHorseBatteryStaple@localhost/app",
            "CorrectHorseBatteryStaple",
        ),
        (
            "DATABASE_URL=mongodb+srv://alice:CorrectHorseBatteryStaple@cluster.example/app",
            "CorrectHorseBatteryStaple",
        ),
        (
            "REDIS_URL=redis://alice:CorrectHorseBatteryStaple@localhost:6379",
            "CorrectHorseBatteryStaple",
        ),
        (
            "REDIS_URL=rediss://alice:CorrectHorseBatteryStaple@cache.example:6379",
            "CorrectHorseBatteryStaple",
        ),
    ] {
        let results = scan_one(&scanner, source);
        let report = results.single_report().expect("one fixture was scanned");

        assert_eq!(
            report.len(),
            1,
            "database connection credential was not detected in {source:?}"
        );

        let finding = &report.findings()[0];

        assert_eq!(
            finding.rule_id().as_str(),
            "generic.database-connection-password"
        );
        assert_eq!(matched(source, finding), expected);
        assert_eq!(finding.severity(), Severity::Critical);
        assert_eq!(finding.confidence(), Confidence::High);
        assert_eq!(finding.remediation(), Some(Remediation::RotatePassword));
    }
}

#[test]
fn database_connection_password_preserves_percent_encoded_source_value() {
    let source = "DATABASE_URL=postgresql://alice:Correct%40Horse%2FBattery@localhost/app";

    let scanner = scanner_for([builtins::DATABASE_CONNECTION_PASSWORD]);
    let results = scan_one(&scanner, source);
    let report = results.single_report().expect("one fixture was scanned");

    assert_eq!(report.len(), 1);

    let finding = &report.findings()[0];

    assert_eq!(
        finding.rule_id().as_str(),
        "generic.database-connection-password"
    );
    assert_eq!(matched(source, finding), "Correct%40Horse%2FBattery");
}

#[test]
fn database_connection_password_rejects_non_credential_uris_and_near_misses() {
    let scanner = scanner_for([builtins::DATABASE_CONNECTION_PASSWORD]);

    for source in [
        "postgres://localhost/app",
        "postgres://alice@localhost/app",
        "postgres://alice:@localhost/app",
        "postgres://localhost:5432/app",
        "postgresql://localhost/app",
        "mysql://localhost/app",
        "mariadb://localhost/app",
        "mongodb://localhost/app",
        "mongodb+srv://cluster.example/app",
        "redis://localhost:6379",
        "rediss://cache.example:6379",
        "https://alice:CorrectHorseBatteryStaple@example.com",
        "example://alice:CorrectHorseBatteryStaple@host",
    ] {
        let results = scan_one(&scanner, source);
        let report = results.single_report().expect("one fixture was scanned");

        assert!(
            report.is_empty(),
            "non-credential URI unexpectedly produced findings for {source:?}: {:?}",
            report
                .iter()
                .map(|finding| finding.rule_id().as_str())
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn database_connection_password_rejects_malformed_percent_encoding() {
    let scanner = scanner_for([builtins::DATABASE_CONNECTION_PASSWORD]);

    for source in [
        "postgres://alice:secret%2@localhost/app",
        "postgres://alice:secret%XX@localhost/app",
        "mysql://alice:secret%@localhost/app",
    ] {
        let results = scan_one(&scanner, source);
        let report = results.single_report().expect("one fixture was scanned");

        assert!(
            report.is_empty(),
            "malformed database password unexpectedly produced a finding: {source:?}"
        );
    }
}

#[test]
fn database_connection_password_preserves_encoded_delimiters_and_exact_span() {
    let scanner = scanner_for([builtins::DATABASE_CONNECTION_PASSWORD]);

    for (source, expected) in [
        (
            "postgres://alice:p%40ss%3Aword@localhost/app",
            "p%40ss%3Aword",
        ),
        (
            r#""postgres://alice:CorrectHorseBatteryStaple@localhost/app""#,
            "CorrectHorseBatteryStaple",
        ),
        (
            "(postgres://alice:CorrectHorseBatteryStaple@localhost/app)",
            "CorrectHorseBatteryStaple",
        ),
    ] {
        let results = scan_one(&scanner, source);
        let report = results.single_report().expect("one fixture was scanned");

        assert_eq!(report.len(), 1);

        let finding = &report.findings()[0];

        assert_eq!(
            finding.rule_id().as_str(),
            "generic.database-connection-password"
        );
        assert_eq!(matched(source, finding), expected);
    }
}

#[test]
fn password_fields_support_quoted_values_with_internal_whitespace() {
    for (rule, source, expected_rule, expected) in [
        (
            builtins::PASSWORD_FIELD,
            r#"password="Correct Horse Battery Staple""#,
            "generic.password-field",
            "Correct Horse Battery Staple",
        ),
        (
            builtins::PASSWORD_FIELD,
            "password='Correct Horse Battery Staple'",
            "generic.password-field",
            "Correct Horse Battery Staple",
        ),
        (
            builtins::DATABASE_PASSWORD_FIELD,
            r#"db_password="Database Horse Battery Staple""#,
            "generic.database-password-field",
            "Database Horse Battery Staple",
        ),
        (
            builtins::PASSPHRASE_FIELD,
            r#"private_key_passphrase="Private Key Horse Battery Staple""#,
            "generic.passphrase-field",
            "Private Key Horse Battery Staple",
        ),
    ] {
        let scanner = scanner_for([rule]);
        let results = scan_one(&scanner, source);
        let report = results.single_report().expect("one fixture was scanned");

        assert_eq!(report.len(), 1);

        let finding = &report.findings()[0];

        assert_eq!(finding.rule_id().as_str(), expected_rule);
        assert_eq!(matched(source, finding), expected);
    }
}

#[test]
fn password_fields_do_not_include_trailing_whitespace_in_the_finding() {
    let scanner = scanner_for([builtins::PASSWORD_FIELD]);

    let source = "password=CorrectHorseBatteryStaple   ";
    let results = scan_one(&scanner, source);
    let report = results.single_report().expect("one fixture was scanned");

    assert_eq!(report.len(), 1);
    assert_eq!(
        matched(source, &report.findings()[0]),
        "CorrectHorseBatteryStaple"
    );
}

#[test]
fn password_fields_stop_at_newlines() {
    let scanner = scanner_for([builtins::PASSWORD_FIELD]);

    let source = "password=\"Correct Horse\nBattery Staple\"";
    let results = scan_one(&scanner, source);
    let report = results.single_report().expect("one fixture was scanned");

    assert_eq!(report.len(), 1);
    assert_eq!(matched(source, &report.findings()[0]), "Correct Horse");
}

#[test]
fn password_fields_do_not_require_matching_surrounding_quotes() {
    let scanner = scanner_for([builtins::PASSWORD_FIELD]);

    let source = "password=\"Correct Horse Battery Staple'";
    let results = scan_one(&scanner, source);
    let report = results.single_report().expect("one fixture was scanned");

    assert_eq!(report.len(), 1);
    assert_eq!(
        matched(source, &report.findings()[0]),
        "Correct Horse Battery Staple"
    );
}
