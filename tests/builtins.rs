//! End-to-end tests for the public built-in detection contracts.
//!
//! These tests exercise the complete public path:
//!
//! `RuleSpec -> ScannerBuilder -> compiled matcher -> capture projection ->
//! validator dispatch -> Finding`
//!
//! Every credential below is synthetic and intentionally unusable.

use std::collections::BTreeSet;

use cribra::{
    Confidence, DetectionMode, Remediation, Scanner, Severity, builtins,
    transform::{SynthesisOptions, synthesize},
};

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
fn nuget_cleartext_password_projects_only_the_value() {
    let scanner = scanner_for([builtins::NUGET_PACKAGE_SOURCE_CLEARTEXT_PASSWORD]);
    let source = r#"<configuration>
  <packageSourceCredentials>
    <PrivateFeed>
      <add
        key='ClearTextPassword'
        value='NuGetSecretValue_1234' />
    </PrivateFeed>
  </packageSourceCredentials>
</configuration>"#;

    let results = scan_one(&scanner, source);
    let report = results.single_report().expect("one source was scanned");

    assert_eq!(report.len(), 1);
    let finding = &report.findings()[0];
    assert_eq!(
        finding.rule_id().as_str(),
        "nuget.package-source-cleartext-password"
    );
    assert_eq!(matched(source, finding), "NuGetSecretValue_1234");
    assert_eq!(finding.severity(), Severity::Critical);
    assert_eq!(finding.confidence(), Confidence::High);
    assert_eq!(
        finding.remediation(),
        Some(Remediation::RevokeAndRotateCredential)
    );
}

#[test]
fn nuget_cleartext_password_rejects_protected_outside_and_malformed_values() {
    let scanner = scanner_for([builtins::NUGET_PACKAGE_SOURCE_CLEARTEXT_PASSWORD]);
    let source = concat!(
        r#"<packageSourceCredentials><Feed><add key="Password" value="ProtectedValue_1234" /></Feed></packageSourceCredentials>"#,
        r#"<packageSourceCredentials><Feed><add key="Username" value="alice-user" /></Feed></packageSourceCredentials>"#,
        r#"<Feed><add key="ClearTextPassword" value="OutsideValue_1234" /></Feed>"#,
        r#"<packageSourceCredentials><Feed><add value="ReversedValue_1234" key="ClearTextPassword" /></Feed></packageSourceCredentials>"#,
        r#"<packageSourceCredentials><Feed><add key="ClearTextPassword" value="placeholder_value" /></Feed>"#,
    );

    let results = scan_one(&scanner, source);
    let report = results.single_report().expect("one source was scanned");

    assert!(report.is_empty());
}

#[test]
fn nuget_context_does_not_leak_across_closed_regions_or_comments() {
    let scanner = scanner_for([builtins::NUGET_PACKAGE_SOURCE_CLEARTEXT_PASSWORD]);
    let source = concat!(
        r#"<packageSourceCredentials></packageSourceCredentials><Feed><add key="ClearTextPassword" value="LeakedValue_1234" /></Feed>"#,
        r#"<!-- <packageSourceCredentials><Feed><add key="ClearTextPassword" value="CommentValue_1234" /></Feed></packageSourceCredentials> -->"#,
    );

    let results = scan_one(&scanner, source);
    let report = results.single_report().expect("one source was scanned");

    assert!(report.is_empty());
}

#[test]
fn nuget_environment_credentials_are_explicitly_not_classified_as_nuget() {
    let scanner = scanner_for([builtins::NUGET_PACKAGE_SOURCE_CLEARTEXT_PASSWORD]);
    let source =
        "NuGetPackageSourceCredentials_PrivateFeed=Username=alice;Password=NuGetEnvSecret_1234";

    let results = scan_one(&scanner, source);
    let report = results.single_report().expect("one source was scanned");

    assert!(
        report
            .findings()
            .iter()
            .all(|finding| finding.rule_id().as_str() != "nuget.package-source-cleartext-password")
    );
}

#[test]
fn nuget_synthesis_is_invalid_under_normal_validation_on_rescan() {
    let scanner = scanner_for([builtins::NUGET_PACKAGE_SOURCE_CLEARTEXT_PASSWORD]);
    let source = r#"<packageSourceCredentials><PrivateFeed><add key="ClearTextPassword" value="NuGetSecretValue_1234" /></PrivateFeed></packageSourceCredentials>"#;
    let results = scan_one(&scanner, source);
    let report = results.single_report().expect("one source was scanned");

    let synthesized = synthesize(source, report, &SynthesisOptions::new([91; 32]))
        .expect("NuGet synthesis should succeed");

    assert_eq!(synthesized.len(), source.len());
    assert!(!synthesized.contains("NuGetSecretValue_1234"));
    assert!(synthesized.as_bytes().contains(&0));

    let rescanned_results = scan_one(&scanner, &synthesized);
    let rescanned = rescanned_results
        .single_report()
        .expect("one synthesized source was scanned");
    assert!(
        rescanned
            .findings()
            .iter()
            .all(|finding| finding.rule_id().as_str() != "nuget.package-source-cleartext-password")
    );
}

#[test]
fn maven_synthesis_is_invalid_under_normal_validation_on_rescan() {
    let scanner = scanner_for([builtins::MAVEN_SERVER_PASSWORD]);
    let source = r#"<settings><servers><server><id>private</id><username>alice</username><password>MavenSecretValue_1234</password></server></servers></settings>"#;

    let results = scan_one(&scanner, source);
    let report = results.single_report().expect("one source was scanned");

    assert_eq!(report.len(), 1);
    assert_eq!(
        report.findings()[0].rule_id().as_str(),
        "maven.server-password"
    );

    let synthesized = synthesize(source, report, &SynthesisOptions::new([92; 32]))
        .expect("Maven synthesis should succeed");

    assert_eq!(synthesized.len(), source.len());
    assert!(!synthesized.contains("MavenSecretValue_1234"));
    assert!(synthesized.as_bytes().contains(&0));

    let rescanned_results = scan_one(&scanner, &synthesized);
    let rescanned = rescanned_results
        .single_report()
        .expect("one synthesized source was scanned");

    assert!(
        rescanned
            .findings()
            .iter()
            .all(|finding| finding.rule_id().as_str() != "maven.server-password"),
        "synthetic Maven password must not validate as a real Maven credential",
    );
}

#[test]
fn maven_server_password_wins_generic_password_collision() {
    let scanner = scanner_for([builtins::MAVEN_SERVER_PASSWORD, builtins::PASSWORD_FIELD]);
    let password = "CorrectHorseBatteryStaple";
    let source = format!(
        "<settings><servers><server><id>private</id><username>alice</username>\
         <password>{password}</password></server></servers></settings>"
    );

    let results = scan_one(&scanner, &source);
    let report = results.single_report().expect("one source was scanned");

    assert_eq!(report.len(), 1);

    let finding = &report.findings()[0];

    assert_eq!(finding.rule_id().as_str(), "maven.server-password");
    assert_eq!(matched(&source, finding), password);
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

#[test]
fn pgp_private_key_detects_complete_ascii_armored_block() {
    let scanner = scanner_for([builtins::PGP_PRIVATE_KEY]);

    let source = concat!(
        "-----BEGIN PGP PRIVATE KEY BLOCK-----\n",
        "\n",
        "lQOYBGsynthetic0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ\n",
        "0123456789abcdefghijklmnopqrstuvwxyzABCDEFGH\n",
        "=ABCD\n",
        "-----END PGP PRIVATE KEY BLOCK-----",
    );

    let results = scan_one(&scanner, source);
    let report = results.single_report().expect("one source");

    assert_eq!(report.len(), 1);

    let finding = &report.findings()[0];

    assert_eq!(finding.rule_id().as_str(), "generic.pgp-private-key");
    assert_eq!(matched(source, finding), source);
    assert_eq!(finding.severity(), Severity::Critical);
    assert_eq!(finding.remediation(), Some(Remediation::ReplacePrivateKey));
}

#[test]
fn pgp_private_key_rejects_public_incomplete_short_and_mismatched_blocks() {
    let scanner = scanner_for([builtins::PGP_PRIVATE_KEY]);

    for source in [
        concat!(
            "-----BEGIN PGP PUBLIC KEY BLOCK-----\n",
            "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ\n",
            "-----END PGP PUBLIC KEY BLOCK-----",
        ),
        "-----BEGIN PGP PRIVATE KEY BLOCK-----",
        concat!(
            "-----BEGIN PGP PRIVATE KEY BLOCK-----\n",
            "short\n",
            "-----END PGP PRIVATE KEY BLOCK-----",
        ),
        concat!(
            "-----BEGIN PGP PRIVATE KEY BLOCK-----\n",
            "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ\n",
            "-----END PGP PUBLIC KEY BLOCK-----",
        ),
        concat!(
            "-----BEGIN PGP PUBLIC KEY BLOCK-----\n",
            "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ\n",
            "-----END PGP PRIVATE KEY BLOCK-----",
        ),
    ] {
        let results = scan_one(&scanner, source);
        let report = results.single_report().expect("one source");

        assert!(
            report.is_empty(),
            "unexpected PGP private-key finding for {source:?}"
        );
    }
}

#[test]
fn pgp_private_key_supports_crlf_armored_blocks() {
    let scanner = scanner_for([builtins::PGP_PRIVATE_KEY]);

    let source = concat!(
        "-----BEGIN PGP PRIVATE KEY BLOCK-----\r\n",
        "\r\n",
        "lQOYBGsynthetic0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ\r\n",
        "=ABCD\r\n",
        "-----END PGP PRIVATE KEY BLOCK-----",
    );

    let results = scan_one(&scanner, source);
    let report = results.single_report().expect("one source");

    assert_eq!(report.len(), 1);
    assert_eq!(matched(source, &report.findings()[0]), source);
}

#[test]
fn pgp_private_key_remains_independent_from_other_private_key_families() {
    let scanner = scanner_for([
        builtins::PKCS8_PRIVATE_KEY,
        builtins::ENCRYPTED_PRIVATE_KEY,
        builtins::RSA_PRIVATE_KEY,
        builtins::EC_PRIVATE_KEY,
        builtins::OPENSSH_PRIVATE_KEY,
        builtins::PGP_PRIVATE_KEY,
    ]);

    let source = concat!(
        "-----BEGIN PGP PRIVATE KEY BLOCK-----\n",
        "lQOYBGsynthetic0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ\n",
        "0123456789abcdefghijklmnopqrstuvwxyzABCDEFGH\n",
        "-----END PGP PRIVATE KEY BLOCK-----",
    );

    let results = scan_one(&scanner, source);
    let report = results.single_report().expect("one source");

    assert_eq!(report.len(), 1);
    assert_eq!(
        report.findings()[0].rule_id().as_str(),
        "generic.pgp-private-key"
    );
}

#[test]
fn pgp_private_key_redaction_replaces_the_entire_armored_block() {
    let scanner = scanner_for([builtins::PGP_PRIVATE_KEY]);

    let source = concat!(
        "before\n",
        "-----BEGIN PGP PRIVATE KEY BLOCK-----\n",
        "lQOYBGsynthetic0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ\n",
        "0123456789abcdefghijklmnopqrstuvwxyzABCDEFGH\n",
        "-----END PGP PRIVATE KEY BLOCK-----\n",
        "after",
    );

    let results = scan_one(&scanner, source);
    let report = results.single_report().expect("one source");

    assert_eq!(report.len(), 1);

    let transformed =
        cribra::transform::redact(source, report).expect("PGP private-key redaction must succeed");

    assert!(!transformed.contains("BEGIN PGP PRIVATE KEY BLOCK"));
    assert!(!transformed.contains("END PGP PRIVATE KEY BLOCK"));
    assert!(!transformed.contains("lQOYBGsynthetic"));
    assert!(transformed.starts_with("before\n"));
    assert!(transformed.ends_with("\nafter"));
}

#[test]
fn wireguard_credentials_detect_realistic_configuration_fields() {
    let scanner = scanner_for([
        builtins::WIREGUARD_PRIVATE_KEY,
        builtins::WIREGUARD_PRESHARED_KEY,
    ]);

    let private_key = "ERERERERERERERERERERERERERERERERERERERERERE=";
    let peer_public_key = "MzMzMzMzMzMzMzMzMzMzMzMzMzMzMzMzMzMzMzMzMzM=";
    let preshared_key = "IiIiIiIiIiIiIiIiIiIiIiIiIiIiIiIiIiIiIiIiIiI=";

    let source = format!(
        "\
[Interface]
Address = 10.0.0.2/32
PrivateKey = {private_key}

[Peer]
PublicKey = {peer_public_key}
PresharedKey = {preshared_key}
Endpoint = vpn.example.com:51820
"
    );

    let results = scan_one(&scanner, &source);
    let report = results.single_report().expect("one source");

    assert_eq!(report.len(), 2);

    let private = report
        .findings()
        .iter()
        .find(|finding| finding.rule_id().as_str() == "wireguard.private-key")
        .expect("WireGuard private key");

    let preshared = report
        .findings()
        .iter()
        .find(|finding| finding.rule_id().as_str() == "wireguard.preshared-key")
        .expect("WireGuard preshared key");

    assert_eq!(matched(&source, private), private_key);
    assert_eq!(matched(&source, preshared), preshared_key);

    assert_eq!(private.severity(), Severity::Critical);
    assert_eq!(preshared.severity(), Severity::Critical);

    assert_eq!(private.remediation(), Some(Remediation::ReplacePrivateKey));
    assert_eq!(
        preshared.remediation(),
        Some(Remediation::ReplacePrivateKey)
    );

    assert_eq!(private.confidence(), Confidence::High);
    assert_eq!(preshared.confidence(), Confidence::High);
}

#[test]
fn wireguard_credentials_reject_base64_without_correct_wireguard_context() {
    let scanner = scanner_for([
        builtins::WIREGUARD_PRIVATE_KEY,
        builtins::WIREGUARD_PRESHARED_KEY,
    ]);

    let value = "ERERERERERERERERERERERERERERERERERERERERERE=";

    for source in [
        format!("PrivateKey = {value}"),
        format!("PresharedKey = {value}"),
        format!("[Peer]\nPrivateKey = {value}"),
        format!("[Interface]\nPresharedKey = {value}"),
        format!("[Other]\nPrivateKey = {value}"),
        format!("blob = {value}"),
        format!("[Interface]\nPublicKey = {value}"),
    ] {
        let results = scan_one(&scanner, &source);
        let report = results.single_report().expect("one source");

        assert!(
            report.is_empty(),
            "unexpected WireGuard credential finding for {source:?}",
        );
    }
}

#[test]
fn docker_registry_auth_detects_config_json_credentials() {
    let scanner = scanner_for([builtins::DOCKER_REGISTRY_AUTH]);

    let encoded = "Y3JpYnJhOkNvcnJlY3RIb3JzZUJhdHRlcnlTdGFwbGU=";

    let source = format!(
        r#"{{
  "auths": {{
    "https://index.docker.io/v1/": {{
      "auth": "{encoded}"
    }}
  }},
  "credsStore": "desktop"
}}"#
    );

    let results = scan_one(&scanner, &source);
    let report = results.single_report().expect("one source");

    assert_eq!(report.len(), 1);

    let finding = &report.findings()[0];

    assert_eq!(finding.rule_id().as_str(), "docker.registry-auth");
    assert_eq!(matched(&source, finding), encoded);
    assert_eq!(finding.severity(), Severity::Critical);
    assert_eq!(finding.remediation(), Some(Remediation::RotatePassword));
    assert_eq!(finding.confidence(), Confidence::High);
}

#[test]
fn docker_registry_auth_rejects_unrelated_auth_and_credential_helpers() {
    let scanner = scanner_for([builtins::DOCKER_REGISTRY_AUTH]);

    let encoded = "Y3JpYnJhOkNvcnJlY3RIb3JzZUJhdHRlcnlTdGFwbGU=";

    for source in [
        format!(r#"{{"auth": "{encoded}"}}"#),
        format!(r#"{{"service":{{"auth":"{encoded}"}}}}"#),
        format!(r#"{{"auths":{{"registry.example.com":{{}}}},"other":{{"auth":"{encoded}"}}}}"#),
        r#"{"credsStore":"desktop"}"#.to_owned(),
        r#"{"credHelpers":{"registry.example.com":"pass"}}"#.to_owned(),
        r#"{"auths":{"registry.example.com":{}}}"#.to_owned(),
    ] {
        let results = scan_one(&scanner, &source);
        let report = results.single_report().expect("one source");

        assert!(
            report.is_empty(),
            "unexpected Docker registry credential finding for {source:?}",
        );
    }
}

#[test]
fn docker_registry_auth_detects_multiple_registry_entries() {
    let scanner = scanner_for([builtins::DOCKER_REGISTRY_AUTH]);

    let first = "YWxpY2U6Q29ycmVjdEhvcnNlQmF0dGVyeVN0YXBsZQ==";
    let second = "Ym9iOkFub3RoZXJTdHJvbmdSZWdpc3RyeVBhc3N3b3Jk";

    let source = format!(
        r#"{{
  "auths": {{
    "registry-one.example.com": {{
      "auth": "{first}"
    }},
    "registry-two.example.com": {{
      "auth": "{second}"
    }}
  }}
}}"#
    );

    let results = scan_one(&scanner, &source);
    let report = results.single_report().expect("one source");

    assert_eq!(report.len(), 2);

    let values = report
        .iter()
        .map(|finding| matched(&source, finding))
        .collect::<Vec<_>>();

    assert_eq!(values, [first, second]);
}

#[test]
fn npm_registry_auth_token_detects_scoped_registry_credentials() {
    let scanner = scanner_for([builtins::NPM_REGISTRY_AUTH_TOKEN]);

    let token = "npm_AbCdEfGhIjKlMnOpQrStUvWxYz012345";

    let source = format!(
        "//registry.npmjs.org/:_authToken={token}\n\
         //npm.pkg.example.com/team/:_authToken={token}"
    );

    let results = scan_one(&scanner, &source);
    let report = results.single_report().expect("one source");

    assert_eq!(report.len(), 2);

    for finding in report {
        assert_eq!(finding.rule_id().as_str(), "npm.registry-auth-token");
        assert_eq!(matched(&source, finding), token);
        assert_eq!(finding.severity(), Severity::Critical);
        assert_eq!(
            finding.remediation(),
            Some(Remediation::RevokeAndRotateCredential)
        );
        assert_eq!(finding.confidence(), Confidence::High);
    }
}

#[test]
fn npm_registry_credentials_reject_unscoped_and_non_secret_configuration() {
    let scanner = scanner_for([
        builtins::NPM_REGISTRY_AUTH_TOKEN,
        builtins::NPM_REGISTRY_AUTH,
        builtins::NPM_REGISTRY_PASSWORD,
    ]);

    for source in [
        "_authToken=npm_AbCdEfGhIjKlMnOpQrStUvWxYz012345",
        "text _authToken=npm_AbCdEfGhIjKlMnOpQrStUvWxYz012345",
        "registry=https://registry.npmjs.org/",
        "always-auth=true",
        "certfile=/home/user/npm-client.pem",
        "keyfile=/home/user/npm-client-key.pem",
        "//registry.npmjs.org/:certfile=/home/user/npm-client.pem",
        "//registry.npmjs.org/:keyfile=/home/user/npm-client-key.pem",
    ] {
        let results = scan_one(&scanner, source);
        let report = results.single_report().expect("one source");

        assert!(
            report.is_empty(),
            "unexpected npm registry credential finding for {source:?}",
        );
    }
}

#[test]
fn npm_registry_legacy_auth_and_password_are_structurally_validated() {
    let scanner = scanner_for([builtins::NPM_REGISTRY_AUTH, builtins::NPM_REGISTRY_PASSWORD]);

    let auth = "Y3JpYnJhOkNvcnJlY3RIb3JzZUJhdHRlcnlTdGFwbGU=";
    let password = "Q29ycmVjdEhvcnNlQmF0dGVyeVN0YXBsZQ==";

    let source = format!(
        "//registry.example.com/:_auth={auth}\n\
         //registry.example.com/:_password={password}"
    );

    let results = scan_one(&scanner, &source);
    let report = results.single_report().expect("one source");

    assert_eq!(report.len(), 2);

    assert_eq!(matched(&source, &report.findings()[0]), auth);
    assert_eq!(matched(&source, &report.findings()[1]), password);
}

#[test]
fn composer_http_basic_password_detects_repository_credential() {
    let scanner = scanner_for([builtins::COMPOSER_HTTP_BASIC_PASSWORD]);

    let password = "ComposerRepositorySecret_123456";
    let source = format!(
        r#"{{
  "http-basic": {{
    "repo.example": {{
      "username": "alice",
      "password": "{password}"
    }}
  }}
}}"#
    );

    let results = scan_one(&scanner, &source);
    let report = results.single_report().expect("one source");

    assert_eq!(report.len(), 1);

    let finding = &report.findings()[0];
    assert_eq!(finding.rule_id().as_str(), "composer.http-basic-password");
    assert_eq!(matched(&source, finding), password);
    assert_eq!(finding.severity(), Severity::Critical);
    assert_eq!(finding.confidence(), Confidence::High);
    assert_eq!(finding.remediation(), Some(Remediation::RotatePassword),);
}

#[test]
fn composer_bearer_token_detects_repository_credential() {
    let scanner = scanner_for([builtins::COMPOSER_BEARER_TOKEN]);

    let token = "ComposerBearerToken_123456";
    let source = format!(
        r#"{{
  "bearer": {{
    "repo.example": "{token}"
  }}
}}"#
    );

    let results = scan_one(&scanner, &source);
    let report = results.single_report().expect("one source");

    assert_eq!(report.len(), 1);

    let finding = &report.findings()[0];
    assert_eq!(finding.rule_id().as_str(), "composer.bearer-token");
    assert_eq!(matched(&source, finding), token);
    assert_eq!(finding.severity(), Severity::Critical);
    assert_eq!(finding.confidence(), Confidence::High);
    assert_eq!(
        finding.remediation(),
        Some(Remediation::RevokeAndRotateCredential),
    );
}

#[test]
fn composer_bearer_token_rejects_unattributed_values() {
    let scanner = scanner_for([builtins::COMPOSER_BEARER_TOKEN]);
    let token = "ComposerBearerToken_123456";

    for source in [
        format!(r#"{{"repo.example":"{token}"}}"#),
        format!(r#"{{"application":{{"repo.example":"{token}"}}}}"#),
        format!(r#"{{"bearer":{{"":"{token}"}}}}"#),
    ] {
        let results = scan_one(&scanner, &source);
        let report = results.single_report().expect("one source");

        assert!(
            report.is_empty(),
            "unattributed bearer value unexpectedly detected as Composer: {source}",
        );
    }
}

#[test]
fn composer_bitbucket_consumer_secret_detects_repository_credential() {
    let scanner = scanner_for([builtins::COMPOSER_BITBUCKET_CONSUMER_SECRET]);

    let secret = "BitbucketConsumerSecret_123456";
    let source = format!(
        r#"{{
  "bitbucket-oauth": {{
    "bitbucket.org": {{
      "consumer-key": "consumer-key",
      "consumer-secret": "{secret}"
    }}
  }}
}}"#
    );

    let results = scan_one(&scanner, &source);
    let report = results.single_report().expect("one source");

    assert_eq!(report.len(), 1);

    let finding = &report.findings()[0];
    assert_eq!(
        finding.rule_id().as_str(),
        "composer.bitbucket-consumer-secret"
    );
    assert_eq!(matched(&source, finding), secret);
    assert_eq!(finding.severity(), Severity::Critical);
    assert_eq!(finding.confidence(), Confidence::High);
    assert_eq!(
        finding.remediation(),
        Some(Remediation::RevokeAndRotateCredential),
    );
}

#[test]
fn composer_forgejo_token_detects_repository_credential() {
    let scanner = scanner_for([builtins::COMPOSER_FORGEJO_TOKEN]);

    let token = "ForgejoAccessToken_123456";
    let source = format!(
        r#"{{
  "forgejo-token": {{
    "forgejo.example.org": {{
      "username": "alice",
      "token": "{token}"
    }}
  }}
}}"#
    );

    let results = scan_one(&scanner, &source);
    let report = results.single_report().expect("one source");

    assert_eq!(report.len(), 1);

    let finding = &report.findings()[0];
    assert_eq!(finding.rule_id().as_str(), "composer.forgejo-token");
    assert_eq!(matched(&source, finding), token);
    assert_eq!(finding.severity(), Severity::Critical);
    assert_eq!(finding.confidence(), Confidence::High);
    assert_eq!(
        finding.remediation(),
        Some(Remediation::RevokeAndRotateCredential),
    );
}

#[test]
fn developer_credentials_are_in_current_pack() {
    let ids: std::collections::HashSet<_> =
        builtins::CURRENT.iter().map(|rule| rule.id()).collect();

    assert!(ids.contains("cargo.registry-token"));
    assert!(ids.contains("cargo.registry-env-token"));

    assert!(ids.contains("swiftpm.registry-token"));
    assert!(ids.contains("swiftpm.registry-password"));
    assert!(ids.contains("swiftpm.source-control-token"));
    assert!(ids.contains("swiftpm.netrc-password"));
    assert!(ids.contains("gradle.repository-password"));
    assert!(ids.contains("gradle.repository-password"));
    assert!(ids.contains("gradle.repository-auth-header-value"));
    assert!(ids.contains("composer.http-basic-password"));
    assert!(ids.contains("composer.bearer-token"));
    assert!(ids.contains("composer.bitbucket-consumer-secret"));
    assert!(ids.contains("composer.forgejo-token"));
}

#[test]
fn cargo_registry_token_detects_default_registry() {
    let scanner = Scanner::default();
    let token = "cargo-secret-token-0123456789";
    let source = format!(
        "[registry]\n\
         token = \"{token}\"\n"
    );

    let results = scanner.scan([("credentials.toml", source.as_str())]);
    let report = results.single_report().expect("one report");

    assert_eq!(report.findings().len(), 1);

    let finding = &report.findings()[0];

    assert_eq!(finding.rule_id().as_str(), "cargo.registry-token");
    assert_eq!(matched(&source, finding), token);
    assert_eq!(finding.severity(), Severity::Critical);
    assert_eq!(finding.confidence(), Confidence::High);
    assert_eq!(
        finding.remediation(),
        Some(Remediation::RevokeAndRotateCredential),
    );
}

#[test]
fn cargo_registry_token_detects_named_registry() {
    let scanner = Scanner::default();
    let token = "private-registry-token-0123456789";
    let source = format!(
        "[registries.internal]\n\
         index = \"sparse+https://packages.example.invalid/index/\"\n\
         token = \"{token}\"\n"
    );

    let results = scanner.scan([("credentials.toml", source.as_str())]);
    let report = results.single_report().expect("one report");

    assert_eq!(report.findings().len(), 1);

    let finding = &report.findings()[0];

    assert_eq!(finding.rule_id().as_str(), "cargo.registry-token");
    assert_eq!(matched(&source, finding), token);
}

#[test]
fn cargo_registry_environment_tokens_are_detected() {
    let scanner = Scanner::default();

    for (source, token) in [
        (
            "CARGO_REGISTRY_TOKEN=cargo-default-token-0123456789",
            "cargo-default-token-0123456789",
        ),
        (
            "CARGO_REGISTRIES_INTERNAL_TOKEN=cargo-private-token-0123456789",
            "cargo-private-token-0123456789",
        ),
    ] {
        let results = scanner.scan([("environment", source)]);
        let report = results.single_report().expect("one report");

        assert_eq!(report.findings().len(), 1, "unexpected Cargo finding count",);

        let finding = &report.findings()[0];

        assert_eq!(finding.rule_id().as_str(), "cargo.registry-env-token",);
        assert_eq!(matched(source, finding), token);
        assert_eq!(finding.severity(), Severity::Critical);
        assert_eq!(finding.confidence(), Confidence::High);
        assert_eq!(
            finding.remediation(),
            Some(Remediation::RevokeAndRotateCredential),
        );
    }
}

#[test]
fn cargo_registry_tokens_project_only_the_secret_value() {
    let scanner = Scanner::default();
    let token = "cargo-project-only-this-value-0123456789";
    let source = format!(
        "[registries.private]\n\
         token = \"{token}\"\n"
    );

    let results = scanner.scan([("credentials.toml", source.as_str())]);
    let report = results.single_report().expect("one report");
    let finding = &report.findings()[0];

    assert_eq!(matched(&source, finding), token);
    assert_eq!(
        finding.location().end() - finding.location().start(),
        token.len(),
    );
}

#[test]
fn cargo_registry_rejects_unrelated_token_fields() {
    let scanner = Scanner::default();
    let token = "ordinary-token-value-0123456789";

    for source in [
        format!("token = \"{token}\""),
        format!("[package]\ntoken = \"{token}\""),
        format!("[profile.release]\ntoken = \"{token}\""),
        format!("[workspace.metadata]\ntoken = \"{token}\""),
    ] {
        let results = scanner.scan([("fixture.toml", source.as_str())]);
        let report = results.single_report().expect("one report");

        assert!(
            report
                .findings()
                .iter()
                .all(|finding| !finding.rule_id().as_str().starts_with("cargo.")),
            "unrelated token field unexpectedly produced Cargo finding",
        );
    }
}

#[test]
fn cargo_registry_rejects_placeholders() {
    let scanner = Scanner::default();

    for token in [
        "token",
        "your_token",
        "your_token_here",
        "example_token",
        "example_token_here",
    ] {
        let source = format!(
            "[registry]\n\
             token = \"{token}\"\n"
        );

        let results = scanner.scan([("credentials.toml", source.as_str())]);
        let report = results.single_report().expect("one report");

        assert!(
            report
                .findings()
                .iter()
                .all(|finding| !finding.rule_id().as_str().starts_with("cargo.")),
            "placeholder unexpectedly produced Cargo finding",
        );
    }
}

#[test]
fn cargo_registry_rule_wins_generic_token_collision() {
    let scanner = Scanner::default();
    let token = "cargo-collision-token-0123456789";
    let source = format!("CARGO_REGISTRY_TOKEN={token}");

    let results = scanner.scan([("environment", source.as_str())]);
    let report = results.single_report().expect("one report");

    assert_eq!(report.findings().len(), 1);

    let finding = &report.findings()[0];

    assert_eq!(finding.rule_id().as_str(), "cargo.registry-env-token",);
    assert_eq!(matched(&source, finding), token);
}

#[test]
fn pypi_repository_token_is_in_current_pack() {
    let ids: std::collections::HashSet<_> =
        builtins::CURRENT.iter().map(|rule| rule.id()).collect();

    assert!(ids.contains("pypi.repository-token"));
}

#[test]
fn pypi_repository_token_detects_default_and_custom_sections() {
    let scanner = Scanner::default();

    for (source, token) in [
        (
            "[pypi]\nusername = __token__\npassword = pypi-AbCdEfGhIjKlMnOpQrStUvWxYz012345",
            "pypi-AbCdEfGhIjKlMnOpQrStUvWxYz012345",
        ),
        (
            "[internal]\nrepository = https://packages.example.invalid/legacy/\nusername = __token__\npassword = private-package-token-0123456789",
            "private-package-token-0123456789",
        ),
    ] {
        let results = scanner.scan([(".pypirc", source)]);
        let report = results.single_report().expect("one report");

        assert_eq!(report.findings().len(), 1);

        let finding = &report.findings()[0];

        assert_eq!(finding.rule_id().as_str(), "pypi.repository-token");
        assert_eq!(matched(source, finding), token);
        assert_eq!(finding.severity(), Severity::Critical);
        assert_eq!(finding.confidence(), Confidence::High);
        assert_eq!(
            finding.remediation(),
            Some(Remediation::RevokeAndRotateCredential),
        );
    }
}

#[test]
fn pypi_repository_token_projects_only_token_value() {
    let scanner = Scanner::default();
    let token = "pypi-AbCdEfGhIjKlMnOpQrStUvWxYz012345";
    let source = format!(
        "[pypi]\n\
         username = __token__\n\
         password = {token}\n"
    );

    let results = scanner.scan([(".pypirc", source.as_str())]);
    let report = results.single_report().expect("one report");
    let finding = &report.findings()[0];

    assert_eq!(matched(&source, finding), token);
    assert_eq!(
        finding.location().end() - finding.location().start(),
        token.len(),
    );
}

#[test]
fn pypi_repository_token_does_not_cross_sections() {
    let scanner = Scanner::default();
    let token = "private-package-token-0123456789";
    let source = format!(
        "[first]\n\
         repository = https://packages.example.invalid/\n\
         username = __token__\n\
         [second]\n\
         password = {token}\n"
    );

    let results = scanner.scan([(".pypirc", source.as_str())]);
    let report = results.single_report().expect("one report");

    assert!(
        report
            .findings()
            .iter()
            .all(|finding| finding.rule_id().as_str() != "pypi.repository-token")
    );
}

#[test]
fn pypi_repository_token_rejects_ordinary_repository_passwords() {
    let scanner = Scanner::default();
    let password = "CorrectHorseBatteryStaple";
    let source = format!(
        "[internal]\n\
         repository = https://packages.example.invalid/\n\
         username = alice\n\
         password = {password}\n"
    );

    let results = scanner.scan([(".pypirc", source.as_str())]);
    let report = results.single_report().expect("one report");

    assert!(
        report
            .findings()
            .iter()
            .all(|finding| finding.rule_id().as_str() != "pypi.repository-token")
    );
}

#[test]
fn pypi_repository_rule_wins_generic_password_collision() {
    let scanner = Scanner::default();
    let token = "pypi-AbCdEfGhIjKlMnOpQrStUvWxYz012345";
    let source = format!(
        "[pypi]\n\
         username = __token__\n\
         password = {token}\n"
    );

    let results = scanner.scan([(".pypirc", source.as_str())]);
    let report = results.single_report().expect("one report");

    assert_eq!(report.findings().len(), 1);
    assert_eq!(
        report.findings()[0].rule_id().as_str(),
        "pypi.repository-token",
    );
}

#[test]
fn rubygems_credentials_are_in_current_pack() {
    let ids: std::collections::HashSet<_> =
        builtins::CURRENT.iter().map(|rule| rule.id()).collect();

    assert!(ids.contains("rubygems.api-key"));
    assert!(ids.contains("rubygems.host-api-key"));
}

#[test]
fn rubygems_prefixed_api_key_is_detected_without_context() {
    let scanner = Scanner::default();
    let key = format!("rubygems_{}", "a".repeat(32));

    let results = scanner.scan([("fixture", key.as_str())]);
    let report = results.single_report().expect("one report");

    assert_eq!(report.findings().len(), 1);
    assert_eq!(
        report.findings()[0].rule_id().to_string(),
        "rubygems.api-key",
    );
    assert_eq!(matched(&key, &report.findings()[0]), key);
}

#[test]
fn rubygems_credentials_file_key_is_detected() {
    let scanner = Scanner::default();
    let key = format!("rubygems_{}", "a".repeat(32));
    let source = format!(":rubygems_api_key: {key}\n");

    let results = scanner.scan([("credentials", source.as_str())]);
    let report = results.single_report().expect("one report");

    assert_eq!(report.findings().len(), 1);
    assert_eq!(
        report.findings()[0].rule_id().to_string(),
        "rubygems.api-key"
    );
    assert_eq!(matched(&source, &report.findings()[0]), key);
}

#[test]
fn rubygems_host_api_key_detects_custom_server_key() {
    let scanner = Scanner::default();
    let key = "custom-gem-server-credential-0123456789";
    let source = format!("GEM_HOST_API_KEY={key}");

    let results = scanner.scan([("environment", source.as_str())]);
    let report = results.single_report().expect("one report");

    assert_eq!(report.findings().len(), 1);
    assert_eq!(
        report.findings()[0].rule_id().to_string(),
        "rubygems.host-api-key"
    );
    assert_eq!(matched(&source, &report.findings()[0]), key);
}

#[test]
fn rubygems_specific_rule_wins_collision() {
    let scanner = Scanner::default();
    let key = format!("rubygems_{}", "a".repeat(32));
    let source = format!("GEM_HOST_API_KEY={key}");

    let results = scanner.scan([("environment", source.as_str())]);
    let report = results.single_report().expect("one report");

    assert_eq!(report.findings().len(), 1);

    // Prefer the structurally authoritative RubyGems.org rule when the key
    // itself proves the provider family.
    assert_eq!(
        report.findings()[0].rule_id().to_string(),
        "rubygems.api-key"
    );
}

#[test]
fn developer_credential_rules_expose_expected_public_metadata() {
    let scanner = Scanner::default();

    for (source, expected_rule, expected_remediation) in [
        (
            r#"{"http-basic":{"repo.example":{"username":"alice","password":"ComposerRepositorySecret_123456"}}}"#,
            "composer.http-basic-password",
            Remediation::RotatePassword,
        ),
        (
            r#"{"bearer":{"repo.example":"ComposerBearerToken_123456"}}"#,
            "composer.bearer-token",
            Remediation::RevokeAndRotateCredential,
        ),
        (
            r#"{"bitbucket-oauth":{"bitbucket.org":{"consumer-key":"key","consumer-secret":"BitbucketConsumerSecret_123456"}}}"#,
            "composer.bitbucket-consumer-secret",
            Remediation::RevokeAndRotateCredential,
        ),
        (
            r#"{"forgejo-token":{"forgejo.example.org":{"username":"alice","token":"ForgejoAccessToken_123456"}}}"#,
            "composer.forgejo-token",
            Remediation::RevokeAndRotateCredential,
        ),
        (
            "[registry]\ntoken = \"cargo-secret-token-0123456789\"",
            "cargo.registry-token",
            Remediation::RevokeAndRotateCredential,
        ),
        (
            "CARGO_REGISTRY_TOKEN=cargo-default-token-0123456789",
            "cargo.registry-env-token",
            Remediation::RevokeAndRotateCredential,
        ),
        (
            "[pypi]\nusername = __token__\npassword = pypi-AbCdEfGhIjKlMnOpQrStUvWxYz012345",
            "pypi.repository-token",
            Remediation::RevokeAndRotateCredential,
        ),
        (
            r#"<packageSourceCredentials><Feed><add key="ClearTextPassword" value="NuGetSecretValue_1234" /></Feed></packageSourceCredentials>"#,
            "nuget.package-source-cleartext-password",
            Remediation::RevokeAndRotateCredential,
        ),
        (
            "<settings><servers><server><password>MavenSecretValue_1234</password></server></servers></settings>",
            "maven.server-password",
            Remediation::RevokeAndRotateCredential,
        ),
        (
            "rubygems_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "rubygems.api-key",
            Remediation::RevokeAndRotateCredential,
        ),
        (
            "GEM_HOST_API_KEY=custom-gem-server-credential-0123456789",
            "rubygems.host-api-key",
            Remediation::RevokeAndRotateCredential,
        ),
        (
            "SWIFTPM_REGISTRY_TOKEN=swiftpm-registry-token-0123456789",
            "swiftpm.registry-token",
            Remediation::RevokeAndRotateCredential,
        ),
        (
            "SWIFTPM_REGISTRY_PASSWORD=SwiftPMRegistryPassword_123456",
            "swiftpm.registry-password",
            Remediation::RotatePassword,
        ),
        (
            "SWIFTPM_SOURCE_CONTROL_TOKEN=swiftpm-source-control-token-0123456789",
            "swiftpm.source-control-token",
            Remediation::RevokeAndRotateCredential,
        ),
        (
            r#"SWIFTPM_NETRC_DATA="machine registry.example.com login alice password SwiftPMNetrcSecret_123456""#,
            "swiftpm.netrc-password",
            Remediation::RotatePassword,
        ),
        (
            "ORG_GRADLE_PROJECT_internalRepositoryPassword=GradleRepositorySecret_123456",
            "gradle.repository-password",
            Remediation::RotatePassword,
        ),
        (
            "ORG_GRADLE_PROJECT_internalRepositoryAuthHeaderValue=Bearer-GradleRepositoryToken_123456",
            "gradle.repository-auth-header-value",
            Remediation::RevokeAndRotateCredential,
        ),
    ] {
        let results = scanner.scan([("fixture", source)]);
        let report = results.single_report().expect("one report");

        let finding = report
            .findings()
            .iter()
            .find(|finding| finding.rule_id().as_str() == expected_rule)
            .expect("expected developer credential finding");

        assert_eq!(finding.severity(), Severity::Critical, "{expected_rule}");
        assert_eq!(finding.confidence(), Confidence::High, "{expected_rule}");
        assert_eq!(
            finding.remediation(),
            Some(expected_remediation),
            "{expected_rule}",
        );
    }
}

#[test]
fn netrc_password_detects_complete_machine_credentials() {
    let scanner = scanner_for([builtins::NETRC_PASSWORD]);

    let source = concat!(
        "machine api.example.com\n",
        "  login alice\n",
        "  password CorrectHorseBatteryStaple\n",
        "\n",
        "machine registry.example.com login bob password AnotherStrongSecret42\n",
    );

    let results = scan_one(&scanner, source);
    let report = results.single_report().expect("one source");

    assert_eq!(report.len(), 2);

    let values = report
        .iter()
        .map(|finding| matched(source, finding))
        .collect::<Vec<_>>();

    assert_eq!(
        values,
        ["CorrectHorseBatteryStaple", "AnotherStrongSecret42"]
    );

    for finding in report {
        assert_eq!(finding.rule_id().as_str(), "netrc.password");
        assert_eq!(finding.severity(), Severity::Critical);
        assert_eq!(finding.confidence(), Confidence::High);
        assert_eq!(finding.remediation(), Some(Remediation::RotatePassword));
    }
}

#[test]
fn netrc_password_projects_only_the_password_value() {
    let scanner = scanner_for([builtins::NETRC_PASSWORD]);

    let password = "CorrectHorseBatteryStaple";
    let source = format!("machine api.example.com login alice password {password}");

    let results = scan_one(&scanner, &source);
    let report = results.single_report().expect("one source");

    assert_eq!(report.len(), 1);
    assert_eq!(matched(&source, &report.findings()[0]), password);
}

#[test]
fn netrc_password_rejects_incomplete_and_unrelated_password_text() {
    let scanner = scanner_for([builtins::NETRC_PASSWORD]);

    let password = "CorrectHorseBatteryStaple";

    for source in [
        format!("password {password}"),
        format!("login alice password {password}"),
        format!("machine api.example.com password {password}"),
        format!("PASSWORD={password}"),
        format!("password={password}"),
        format!("service password {password}"),
    ] {
        let results = scan_one(&scanner, &source);
        let report = results.single_report().expect("one source");

        assert!(
            report.is_empty(),
            "unexpected .netrc password finding for invalid fixture",
        );
    }
}

#[test]
fn netrc_password_does_not_cross_machine_record_boundaries() {
    let scanner = scanner_for([builtins::NETRC_PASSWORD]);

    let password = "CorrectHorseBatteryStaple";

    for source in [
        format!(
            "machine first.example.com login alice\n\
             machine second.example.com password {password}"
        ),
        format!(
            "machine first.example.com login alice\n\
             default password {password}"
        ),
        format!(
            "machine first.example.com login alice\n\
             macdef init\n\
             password {password}"
        ),
    ] {
        let results = scan_one(&scanner, &source);
        let report = results.single_report().expect("one source");

        assert!(
            report.is_empty(),
            "cross-record .netrc password unexpectedly detected",
        );
    }
}

#[test]
fn netrc_password_rejects_placeholders_and_documentation_values() {
    let scanner = scanner_for([builtins::NETRC_PASSWORD]);

    for password in [
        "changeme",
        "password",
        "your_password",
        "your_password_here",
        "example_password",
        "replace_me",
    ] {
        let source = format!("machine api.example.com login alice password {password}");

        let results = scan_one(&scanner, &source);
        let report = results.single_report().expect("one source");

        assert!(
            report.is_empty(),
            "documentation .netrc password unexpectedly detected",
        );
    }
}

#[test]
fn swiftpm_registry_token_detects_environment_credential() {
    let scanner = scanner_for([builtins::SWIFTPM_REGISTRY_TOKEN]);

    let token = "swiftpm-registry-token-0123456789";
    let source = format!("SWIFTPM_REGISTRY_TOKEN={token}");

    let results = scan_one(&scanner, &source);
    let report = results.single_report().expect("one source");

    assert_eq!(report.len(), 1);

    let finding = &report.findings()[0];
    assert_eq!(finding.rule_id().as_str(), "swiftpm.registry-token");
    assert_eq!(matched(&source, finding), token);
    assert_eq!(finding.severity(), Severity::Critical);
    assert_eq!(finding.confidence(), Confidence::High);
    assert_eq!(
        finding.remediation(),
        Some(Remediation::RevokeAndRotateCredential)
    );
}

#[test]
fn swiftpm_registry_password_detects_environment_credential() {
    let scanner = scanner_for([builtins::SWIFTPM_REGISTRY_PASSWORD]);

    let password = "SwiftPMRegistryPassword_123456";
    let source = format!("SWIFTPM_REGISTRY_PASSWORD={password}");

    let results = scan_one(&scanner, &source);
    let report = results.single_report().expect("one source");

    assert_eq!(report.len(), 1);

    let finding = &report.findings()[0];
    assert_eq!(finding.rule_id().as_str(), "swiftpm.registry-password");
    assert_eq!(matched(&source, finding), password);
    assert_eq!(finding.severity(), Severity::Critical);
    assert_eq!(finding.confidence(), Confidence::High);
    assert_eq!(finding.remediation(), Some(Remediation::RotatePassword));
}

#[test]
fn swiftpm_source_control_token_detects_environment_credential() {
    let scanner = scanner_for([builtins::SWIFTPM_SOURCE_CONTROL_TOKEN]);

    let token = "swiftpm-source-control-token-0123456789";
    let source = format!("SWIFTPM_SOURCE_CONTROL_TOKEN={token}");

    let results = scan_one(&scanner, &source);
    let report = results.single_report().expect("one source");

    assert_eq!(report.len(), 1);

    let finding = &report.findings()[0];
    assert_eq!(finding.rule_id().as_str(), "swiftpm.source-control-token");
    assert_eq!(matched(&source, finding), token);
    assert_eq!(finding.severity(), Severity::Critical);
    assert_eq!(finding.confidence(), Confidence::High);
    assert_eq!(
        finding.remediation(),
        Some(Remediation::RevokeAndRotateCredential)
    );
}

#[test]
fn swiftpm_registry_login_is_not_a_credential_rule() {
    let scanner = Scanner::default();
    let source = "SWIFTPM_REGISTRY_LOGIN=swift-user";

    let results = scan_one(&scanner, source);
    let report = results.single_report().expect("one source");

    assert!(
        report
            .iter()
            .all(|finding| !finding.rule_id().as_str().starts_with("swiftpm.")),
        "SwiftPM registry login unexpectedly classified as a credential",
    );
}

#[test]
fn swiftpm_credentials_support_quoted_values_and_project_only_the_secret() {
    for (rule, source, expected) in [
        (
            builtins::SWIFTPM_REGISTRY_TOKEN,
            r#"SWIFTPM_REGISTRY_TOKEN="swiftpm-registry-token-0123456789""#,
            "swiftpm-registry-token-0123456789",
        ),
        (
            builtins::SWIFTPM_REGISTRY_PASSWORD,
            "SWIFTPM_REGISTRY_PASSWORD='SwiftPMRegistryPassword_123456'",
            "SwiftPMRegistryPassword_123456",
        ),
        (
            builtins::SWIFTPM_SOURCE_CONTROL_TOKEN,
            r#"SWIFTPM_SOURCE_CONTROL_TOKEN="swiftpm-source-control-token-0123456789""#,
            "swiftpm-source-control-token-0123456789",
        ),
    ] {
        let scanner = scanner_for([rule]);
        let results = scan_one(&scanner, source);
        let report = results.single_report().expect("one source");

        assert_eq!(report.len(), 1);
        assert_eq!(matched(source, &report.findings()[0]), expected);
    }
}

#[test]
fn swiftpm_credentials_reject_obvious_placeholders() {
    for (rule, name) in [
        (builtins::SWIFTPM_REGISTRY_TOKEN, "SWIFTPM_REGISTRY_TOKEN"),
        (
            builtins::SWIFTPM_REGISTRY_PASSWORD,
            "SWIFTPM_REGISTRY_PASSWORD",
        ),
        (
            builtins::SWIFTPM_SOURCE_CONTROL_TOKEN,
            "SWIFTPM_SOURCE_CONTROL_TOKEN",
        ),
    ] {
        for value in [
            "changeme",
            "replace_me",
            "your_token_here",
            "your_password_here",
        ] {
            let source = format!("{name}={value}");
            let scanner = scanner_for([rule]);
            let results = scan_one(&scanner, &source);
            let report = results.single_report().expect("one source");

            assert!(
                report.is_empty(),
                "unexpected SwiftPM finding for placeholder {value:?}",
            );
        }
    }
}

#[test]
fn swiftpm_credentials_reject_unrelated_and_lookalike_variables() {
    for (rule, sources) in [
        (
            builtins::SWIFTPM_REGISTRY_TOKEN,
            [
                "SWIFTPM_REGISTRY_TOKEN_SUFFIX=swiftpm-secret-0123456789",
                "MY_SWIFTPM_REGISTRY_TOKEN=swiftpm-secret-0123456789",
            ],
        ),
        (
            builtins::SWIFTPM_REGISTRY_PASSWORD,
            [
                "SWIFTPM_REGISTRY_PASSWORD_SUFFIX=SwiftPMSecret_123456",
                "MY_SWIFTPM_REGISTRY_PASSWORD=SwiftPMSecret_123456",
            ],
        ),
        (
            builtins::SWIFTPM_SOURCE_CONTROL_TOKEN,
            [
                "SWIFTPM_SOURCE_CONTROL_TOKEN_SUFFIX=swiftpm-secret-0123456789",
                "MY_SWIFTPM_SOURCE_CONTROL_TOKEN=swiftpm-secret-0123456789",
            ],
        ),
    ] {
        let scanner = scanner_for([rule]);

        for source in sources {
            let results = scan_one(&scanner, source);
            let report = results.single_report().expect("one source");

            assert!(
                report.is_empty(),
                "unexpected SwiftPM finding for lookalike variable: {source}",
            );
        }
    }
}

#[test]
fn swiftpm_netrc_data_detects_each_machine_password() {
    let source = concat!(
        "SWIFTPM_NETRC_DATA=\"",
        "machine registry1.example.com login alice password SwiftPMNetrcSecretOne_123456\n",
        "machine registry2.example.com login bob password SwiftPMNetrcSecretTwo_123456",
        "\"",
    );

    let scanner = scanner_for([builtins::SWIFTPM_NETRC_PASSWORD]);
    let results = scan_one(&scanner, source);
    let report = results.single_report().expect("one source");

    assert_eq!(report.len(), 2);

    let mut values = report
        .findings()
        .iter()
        .map(|finding| matched(source, finding))
        .collect::<Vec<_>>();
    values.sort_unstable();

    assert_eq!(
        values,
        [
            "SwiftPMNetrcSecretOne_123456",
            "SwiftPMNetrcSecretTwo_123456",
        ]
    );

    for finding in report.findings() {
        assert_eq!(finding.rule_id().as_str(), "swiftpm.netrc-password");
        assert_eq!(finding.severity(), Severity::Critical);
        assert_eq!(finding.confidence(), Confidence::High);
        assert_eq!(finding.remediation(), Some(Remediation::RotatePassword),);
    }
}

#[test]
fn swiftpm_netrc_password_does_not_claim_plain_netrc() {
    let source = "machine registry.example.com login alice password SwiftPMNetrcSecret_123456";

    let scanner = scanner_for([builtins::SWIFTPM_NETRC_PASSWORD]);
    let results = scan_one(&scanner, source);
    let report = results.single_report().expect("one source");

    assert!(report.is_empty());
}

#[test]
fn swiftpm_netrc_password_rejects_incomplete_machine_records() {
    for source in [
        r#"SWIFTPM_NETRC_DATA="password SwiftPMNetrcSecret_123456""#,
        r#"SWIFTPM_NETRC_DATA="machine registry.example.com password SwiftPMNetrcSecret_123456""#,
        r#"SWIFTPM_NETRC_DATA="login alice password SwiftPMNetrcSecret_123456""#,
        r#"SWIFTPM_NETRC_DATA="machine registry.example.com login password SwiftPMNetrcSecret_123456""#,
    ] {
        let scanner = scanner_for([builtins::SWIFTPM_NETRC_PASSWORD]);
        let results = scan_one(&scanner, source);
        let report = results.single_report().expect("one source");

        assert!(
            report.is_empty(),
            "unexpected SwiftPM netrc finding for incomplete record: {source}",
        );
    }
}

#[test]
fn swiftpm_netrc_password_does_not_cross_record_boundaries() {
    for source in [
        concat!(
            r#"SWIFTPM_NETRC_DATA="machine first.example.com login alice "#,
            "machine second.example.com password SwiftPMNetrcSecret_123456\"",
        ),
        concat!(
            r#"SWIFTPM_NETRC_DATA="machine first.example.com login alice "#,
            "default password SwiftPMNetrcSecret_123456\"",
        ),
        concat!(
            r#"SWIFTPM_NETRC_DATA="machine first.example.com login alice "#,
            "macdef init password SwiftPMNetrcSecret_123456\"",
        ),
    ] {
        let scanner = scanner_for([builtins::SWIFTPM_NETRC_PASSWORD]);
        let results = scan_one(&scanner, source);
        let report = results.single_report().expect("one source");

        assert!(
            report.is_empty(),
            "unexpected cross-record SwiftPM netrc finding: {source}",
        );
    }
}

#[test]
fn swiftpm_netrc_password_rejects_placeholders() {
    for password in [
        "changeme",
        "password",
        "your_password",
        "your_password_here",
        "example_password",
        "replace_me",
    ] {
        let source = format!(
            r#"SWIFTPM_NETRC_DATA="machine registry.example.com login alice password {password}""#
        );

        let scanner = scanner_for([builtins::SWIFTPM_NETRC_PASSWORD]);
        let results = scan_one(&scanner, &source);
        let report = results.single_report().expect("one source");

        assert!(
            report.is_empty(),
            "unexpected SwiftPM netrc finding for placeholder password",
        );
    }
}

#[test]
fn swiftpm_netrc_password_rejects_lookalike_container_names() {
    for name in ["MY_SWIFTPM_NETRC_DATA", "SWIFTPM_NETRC_DATA_SUFFIX"] {
        let source = format!(
            r#"{name}="machine registry.example.com login alice password SwiftPMNetrcSecret_123456""#
        );

        let scanner = scanner_for([builtins::SWIFTPM_NETRC_PASSWORD]);
        let results = scan_one(&scanner, &source);
        let report = results.single_report().expect("one source");

        assert!(
            report.is_empty(),
            "unexpected SwiftPM netrc finding for lookalike container {name}",
        );
    }
}

#[test]
fn gradle_repository_password_has_expected_public_semantics() {
    let scanner = scanner_for([builtins::GRADLE_REPOSITORY_PASSWORD]);

    let password = "GradleRepositorySecret_123456";
    let source = format!("ORG_GRADLE_PROJECT_internalRepositoryPassword={password}");

    let results = scan_one(&scanner, &source);
    let report = results.single_report().expect("one source");

    assert_eq!(report.len(), 1);

    let finding = &report.findings()[0];

    assert_eq!(finding.rule_id().as_str(), "gradle.repository-password");
    assert_eq!(matched(&source, finding), password);
    assert_eq!(finding.severity(), Severity::Critical);
    assert_eq!(finding.confidence(), Confidence::High);
    assert_eq!(finding.remediation(), Some(Remediation::RotatePassword),);
}

#[test]
fn gradle_repository_credentials_reject_unattributed_properties() {
    let scanner = scanner_for([
        builtins::GRADLE_REPOSITORY_PASSWORD,
        builtins::GRADLE_REPOSITORY_AUTH_HEADER_VALUE,
    ]);
    let password = "GradleRepositorySecret_123456";

    for source in [
        format!("internalRepositoryPassword={password}"),
        format!("MY_ORG_GRADLE_PROJECT_internalRepositoryPassword={password}"),
        format!("ORG_GRADLE_PROJECT_Password={password}"),
        format!("ORG_GRADLE_PROJECT_internalRepositoryPasswordSuffix={password}"),
        "internalRepositoryAuthHeaderValue=Bearer GradleRepositoryToken_123456".to_owned(),
    ] {
        let results = scan_one(&scanner, &source);
        let report = results.single_report().expect("one source");

        assert!(
            report.is_empty(),
            "unattributed Gradle property unexpectedly detected",
        );
    }
}

#[test]
fn gradle_repository_auth_header_value_detects_environment_credential() {
    let scanner = scanner_for([builtins::GRADLE_REPOSITORY_AUTH_HEADER_VALUE]);

    let credential = "Bearer GradleRepositoryToken_123456";
    let source = format!("ORG_GRADLE_PROJECT_internalRepositoryAuthHeaderValue={credential}");

    let results = scan_one(&scanner, &source);
    let report = results.single_report().expect("one source");

    assert_eq!(report.len(), 1);

    let finding = &report.findings()[0];

    assert_eq!(
        finding.rule_id().as_str(),
        "gradle.repository-auth-header-value"
    );
    assert_eq!(matched(&source, finding), credential);
    assert_eq!(finding.severity(), Severity::Critical);
    assert_eq!(finding.confidence(), Confidence::High);
    assert_eq!(
        finding.remediation(),
        Some(Remediation::RevokeAndRotateCredential),
    );
}

#[test]
fn gradle_repository_password_wins_generic_password_collision() {
    let scanner = Scanner::default();
    let password = "GradleRepositorySecret_123456";
    let source = format!("ORG_GRADLE_PROJECT_internalRepositoryPassword={password}");

    let results = scan_one(&scanner, &source);
    let report = results.single_report().expect("one source");

    assert_eq!(report.findings().len(), 1);

    let finding = &report.findings()[0];
    assert_eq!(finding.rule_id().as_str(), "gradle.repository-password");
    assert_eq!(matched(&source, finding), password);
}

#[test]
fn shadow_password_verifier_detects_supported_crypt_families() {
    let scanner = scanner_for([builtins::SHADOW_PASSWORD_VERIFIER]);

    let sha256 = "$5$rounds=5000$abcdefghijklmnop$0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdef";
    let sha512 = "$6$abcdefghijklmnop$0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let yescrypt = "$y$j9T$abcdefghijklmnop$0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnop";

    let source = format!(
        "alice:{sha256}:20000:0:99999:7:::\n\
         bob:{sha512}:20001:0:99999:7:::\n\
         carol:{yescrypt}:20002:0:99999:7:::\n"
    );

    let results = scan_one(&scanner, &source);
    let report = results.single_report().expect("one source");

    assert_eq!(report.len(), 3);

    let values = report
        .iter()
        .map(|finding| matched(&source, finding))
        .collect::<Vec<_>>();

    assert_eq!(values, [sha256, sha512, yescrypt]);

    for finding in report {
        assert_eq!(
            finding.rule_id().as_str(),
            "system.shadow-password-verifier"
        );
        assert_eq!(finding.severity(), Severity::High);
        assert_eq!(finding.confidence(), Confidence::High);
        assert_eq!(
            finding.remediation(),
            Some(Remediation::ReviewPasswordVerifier)
        );
    }
}

#[test]
fn shadow_password_verifier_projects_only_the_verifier_field() {
    let scanner = scanner_for([builtins::SHADOW_PASSWORD_VERIFIER]);

    let verifier = "$6$abcdefghijklmnop$0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let source = format!("alice:{verifier}:20000:0:99999:7:::\n");

    let results = scan_one(&scanner, &source);
    let report = results.single_report().expect("one source");

    assert_eq!(report.len(), 1);
    assert_eq!(matched(&source, &report.findings()[0]), verifier);
}

#[test]
fn shadow_password_verifier_rejects_non_shadow_context() {
    let scanner = scanner_for([builtins::SHADOW_PASSWORD_VERIFIER]);

    let verifier = "$6$abcdefghijklmnop$0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";

    for source in [
        verifier.to_owned(),
        format!("password_hash={verifier}"),
        format!("alice:{verifier}"),
        format!("alice:{verifier}:20000"),
        format!("checksum:{verifier}:sha256:package:release:artifact:linux:x86_64:stable"),
    ] {
        let results = scan_one(&scanner, &source);
        let report = results.single_report().expect("one source");

        assert!(
            report.is_empty(),
            "unexpected shadow verifier detection for {source:?}",
        );
    }
}

#[test]
fn shadow_password_verifier_rejects_locked_and_unsupported_fields() {
    let scanner = scanner_for([builtins::SHADOW_PASSWORD_VERIFIER]);

    for source in [
        "root:!:20000:0:99999:7:::\n",
        "daemon:*:20000:0:99999:7:::\n",
        "service:!!:20000:0:99999:7:::\n",
        "alice:!$6$abcdefghijklmnop$0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ:20000:0:99999:7:::\n",
        "bob:$1$salt$abcdefghijklmnopqrstuv:20000:0:99999:7:::\n",
        "carol:$2b$12$abcdefghijklmnopqrstuuABCDEFGHIJKLMNOPQRSTUVWXYZ12345:20000:0:99999:7:::\n",
    ] {
        let results = scan_one(&scanner, source);
        let report = results.single_report().expect("one source");

        assert!(
            report.is_empty(),
            "unexpected shadow verifier detection for {source:?}",
        );
    }
}

#[test]
fn htpasswd_password_verifier_detects_supported_families() {
    let scanner = scanner_for([builtins::HTPASSWD_PASSWORD_VERIFIER]);

    let apr1 = "$apr1$hfT7jp2q$2VbDVlM1QY3wP1uQYxJYB/";
    let bcrypt_a = "$2a$12$123456789012345678901u1234567890123456789012345678901";
    let bcrypt_b = "$2b$12$123456789012345678901u1234567890123456789012345678901";
    let bcrypt_y = "$2y$12$123456789012345678901u1234567890123456789012345678901";

    let source = format!(
        "alice:{apr1}\n\
         bob:{bcrypt_a}\n\
         carol:{bcrypt_b}\n\
         dave:{bcrypt_y}\n"
    );

    let results = scan_one(&scanner, &source);
    let report = results.single_report().expect("one source");

    assert_eq!(report.len(), 4);

    let values = report
        .iter()
        .map(|finding| matched(&source, finding))
        .collect::<Vec<_>>();

    assert_eq!(values, [apr1, bcrypt_a, bcrypt_b, bcrypt_y]);

    for finding in report {
        assert_eq!(
            finding.rule_id().as_str(),
            "system.htpasswd-password-verifier"
        );
        assert_eq!(finding.severity(), Severity::High);
        assert_eq!(finding.confidence(), Confidence::High);
        assert_eq!(
            finding.remediation(),
            Some(Remediation::ReviewPasswordVerifier)
        );
    }
}

#[test]
fn htpasswd_password_verifier_projects_only_the_verifier() {
    let scanner = scanner_for([builtins::HTPASSWD_PASSWORD_VERIFIER]);

    let verifier = "$apr1$hfT7jp2q$2VbDVlM1QY3wP1uQYxJYB/";
    let source = format!("alice:{verifier}");

    let results = scan_one(&scanner, &source);
    let report = results.single_report().expect("one source");

    assert_eq!(report.len(), 1);
    assert_eq!(matched(&source, &report.findings()[0]), verifier);
}

#[test]
fn htpasswd_password_verifier_rejects_standalone_and_malformed_values() {
    let scanner = scanner_for([builtins::HTPASSWD_PASSWORD_VERIFIER]);

    for source in [
        "$apr1$hfT7jp2q$2VbDVlM1QY3wP1uQYxJYB/".to_owned(),
        "$2b$12$123456789012345678901u1234567890123456789012345678901".to_owned(),
        "alice:$apr1$$2VbDVlM1QY3wP1uQYxJYB/".to_owned(),
        "alice:$apr1$toolongsalt$2VbDVlM1QY3wP1uQYxJYB/".to_owned(),
        "alice:$apr1$hfT7jp2q$short".to_owned(),
        "alice:$2b$03$123456789012345678901u1234567890123456789012345678901".to_owned(),
        "alice:$2b$32$123456789012345678901u1234567890123456789012345678901".to_owned(),
        "alice:$2x$12$123456789012345678901u1234567890123456789012345678901".to_owned(),
    ] {
        let results = scan_one(&scanner, &source);
        let report = results.single_report().expect("one source");

        assert!(
            report.is_empty(),
            "unexpected htpasswd verifier detection for {source:?}",
        );
    }
}

#[test]
fn system_password_verifier_rules_do_not_cross_record_formats() {
    let scanner = scanner_for([
        builtins::SHADOW_PASSWORD_VERIFIER,
        builtins::HTPASSWD_PASSWORD_VERIFIER,
    ]);

    let shadow = "$6$abcdefghijklmnop$0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let htpasswd = "$apr1$hfT7jp2q$2VbDVlM1QY3wP1uQYxJYB/";

    let source = format!(
        "alice:{shadow}:20000:0:99999:7:::\n\
         bob:{htpasswd}\n"
    );

    let results = scan_one(&scanner, &source);
    let report = results.single_report().expect("one source");

    assert_eq!(report.len(), 2);

    assert_eq!(
        report.findings()[0].rule_id().as_str(),
        "system.shadow-password-verifier"
    );
    assert_eq!(matched(&source, &report.findings()[0]), shadow);

    assert_eq!(
        report.findings()[1].rule_id().as_str(),
        "system.htpasswd-password-verifier"
    );
    assert_eq!(matched(&source, &report.findings()[1]), htpasswd);
}

#[test]
fn financial_iban_is_opt_in_and_not_part_of_default_scanner() {
    let source = "IT60X0542811101000000123456";

    let default_results = scan_one(&Scanner::default(), source);
    let default_report = default_results
        .single_report()
        .expect("one fixture was scanned");

    assert!(
        default_report
            .findings()
            .iter()
            .all(|finding| finding.rule_id().as_str() != "financial.iban")
    );

    let scanner = scanner_for(builtins::financial::CURRENT.iter().copied());
    let results = scan_one(&scanner, source);
    let report = results.single_report().expect("one fixture was scanned");

    assert_eq!(report.len(), 1);
    assert_eq!(report.findings()[0].rule_id().as_str(), "financial.iban");
}

#[test]
fn financial_iban_reports_exact_electronic_span() {
    let iban = "IT60X0542811101000000123456";
    let source = format!("before {iban} after");
    let scanner = scanner_for([builtins::financial::IBAN]);

    let results = scan_one(&scanner, &source);
    let report = results.single_report().expect("one fixture was scanned");

    assert_eq!(report.len(), 1);

    let finding = &report.findings()[0];

    assert_eq!(finding.rule_id().as_str(), "financial.iban");
    assert_eq!(matched(&source, finding), iban);
    assert_eq!(finding.severity(), Severity::High);
    assert_eq!(finding.confidence(), Confidence::High);
    assert_eq!(
        finding.remediation(),
        Some(Remediation::RemoveSensitiveValue)
    );
}

#[test]
fn financial_iban_print_representation_is_not_discovered_yet() {
    let source = "IT60 X054 2811 1010 0000 0123 456";
    let scanner = scanner_for([builtins::financial::IBAN]);

    let results = scan_one(&scanner, source);
    let report = results.single_report().expect("one fixture was scanned");

    assert!(report.is_empty());
}

#[test]
fn financial_iban_accepts_punctuation_boundaries_without_including_them() {
    let iban = "IT60X0542811101000000123456";
    let source = format!("({iban})");
    let scanner = scanner_for([builtins::financial::IBAN]);

    let results = scan_one(&scanner, &source);
    let report = results.single_report().expect("one fixture was scanned");

    assert_eq!(report.len(), 1);
    assert_eq!(matched(&source, &report.findings()[0]), iban);
}

#[test]
fn financial_iban_rejects_alphanumeric_adjacency() {
    let iban = "IT60X0542811101000000123456";
    let scanner = scanner_for([builtins::financial::IBAN]);

    for source in [format!("x{iban}"), format!("{iban}x")] {
        let results = scan_one(&scanner, &source);
        let report = results.single_report().expect("one fixture was scanned");

        assert!(
            report.is_empty(),
            "IBAN embedded in an alphanumeric token must not be detected"
        );
    }
}

#[test]
fn financial_iban_rejects_invalid_checksum() {
    let scanner = scanner_for([builtins::financial::IBAN]);
    let source = "IT00X0542811101000000123456";

    let results = scan_one(&scanner, source);
    let report = results.single_report().expect("one fixture was scanned");

    assert!(report.is_empty());
}

#[test]
fn financial_iban_rejects_unknown_country() {
    let scanner = scanner_for([builtins::financial::IBAN]);
    let source = "ZZ60X0542811101000000123456";

    let results = scan_one(&scanner, source);
    let report = results.single_report().expect("one fixture was scanned");

    assert!(report.is_empty());
}

#[test]
fn financial_iban_is_detected_before_following_prose() {
    let iban = "IT60X0542811101000000123456";
    let source = format!("IBAN {iban} account");
    let scanner = scanner_for([builtins::financial::IBAN]);

    let results = scan_one(&scanner, &source);
    let report = results.single_report().expect("one fixture was scanned");

    assert_eq!(report.len(), 1);
    assert_eq!(matched(&source, &report.findings()[0]), iban);
}

#[test]
fn financial_pan_is_opt_in_and_not_part_of_default_scanner() {
    let source = "card_number=4111111111111111";

    let default_results = scan_one(&Scanner::default(), source);
    let default_report = default_results
        .single_report()
        .expect("one fixture was scanned");

    assert!(
        default_report
            .findings()
            .iter()
            .all(|finding| finding.rule_id().as_str() != "financial.pan")
    );

    let scanner = scanner_for(builtins::financial::CURRENT.iter().copied());
    let results = scan_one(&scanner, source);
    let report = results.single_report().expect("one fixture was scanned");

    assert_eq!(report.len(), 1);
    assert_eq!(report.findings()[0].rule_id().as_str(), "financial.pan");
}

#[test]
fn financial_pan_reports_exact_compact_span_and_metadata() {
    let pan = "4111111111111111";
    let source = format!("card_number=({pan})");
    let scanner = scanner_for([builtins::financial::PAN]);

    let results = scan_one(&scanner, &source);
    let report = results.single_report().expect("one fixture was scanned");

    assert_eq!(report.len(), 1);

    let finding = &report.findings()[0];

    assert_eq!(finding.rule_id().as_str(), "financial.pan");
    assert_eq!(matched(&source, finding), pan);
    assert_eq!(finding.severity(), Severity::High);
    assert_eq!(finding.confidence(), Confidence::High);
    assert_eq!(
        finding.remediation(),
        Some(Remediation::RemoveSensitiveValue)
    );
}

#[test]
fn financial_pan_requires_explicit_card_context() {
    let pan = "4111111111111111";
    let scanner = scanner_for([builtins::financial::PAN]);

    for key in [
        "number",
        "account",
        "account_number",
        "payment",
        "card",
        "reference_number",
    ] {
        let source = format!("{key}={pan}");
        let results = scan_one(&scanner, &source);
        let report = results.single_report().expect("one fixture was scanned");

        assert!(
            report.is_empty(),
            "weak context {key:?} must not establish PAN authority"
        );
    }
}

#[test]
fn financial_pan_accepts_explicit_normalized_card_contexts() {
    let pan = "4111111111111111";
    let scanner = scanner_for([builtins::financial::PAN]);

    for key in [
        "pan",
        "primary_account_number",
        "CARD-NUMBER",
        "card.no",
        "card_num",
        "payment-card-number",
        "credit.card.number",
        "debit_card_number",
    ] {
        let source = format!("{key}={pan}");
        let results = scan_one(&scanner, &source);
        let report = results.single_report().expect("one fixture was scanned");

        assert_eq!(
            report.len(),
            1,
            "explicit PAN context {key:?} should be accepted"
        );
        assert_eq!(matched(&source, &report.findings()[0]), pan);
    }
}

#[test]
fn financial_pan_rejects_invalid_luhn_checksum() {
    let source = "card_number=4111111111111112";
    let scanner = scanner_for([builtins::financial::PAN]);

    let results = scan_one(&scanner, source);
    let report = results.single_report().expect("one fixture was scanned");

    assert!(report.is_empty());
}

#[test]
fn financial_pan_does_not_extract_from_longer_numeric_tokens() {
    let scanner = scanner_for([builtins::financial::PAN]);

    for source in [
        "card_number=94111111111111111999",
        "card_number=94111111111111111111",
    ] {
        let results = scan_one(&scanner, source);
        let report = results.single_report().expect("one fixture was scanned");

        assert!(
            report.is_empty(),
            "PAN matcher must not extract a valid-looking substring from a longer numeric token"
        );
    }
}

#[test]
fn financial_pan_formatted_representations_are_not_discovered_yet() {
    let scanner = scanner_for([builtins::financial::PAN]);

    for source in [
        "card_number=4111 1111 1111 1111",
        "card_number=4111-1111-1111-1111",
    ] {
        let results = scan_one(&scanner, source);
        let report = results.single_report().expect("one fixture was scanned");

        assert!(
            report.is_empty(),
            "formatted PAN representations are outside the current discovery contract"
        );
    }
}

#[test]
fn financial_pan_accepts_punctuation_boundaries_without_including_them() {
    let pan = "4111111111111111";
    let source = format!("card_number=({pan}),");
    let scanner = scanner_for([builtins::financial::PAN]);

    let results = scan_one(&scanner, &source);
    let report = results.single_report().expect("one fixture was scanned");

    assert_eq!(report.len(), 1);
    assert_eq!(matched(&source, &report.findings()[0]), pan);
}

#[test]
fn financial_pack_composes_with_default_builtins_without_joining_default_pack() {
    let scanner = Scanner::builder()
        .builtins(builtins::CURRENT)
        .builtins(builtins::financial::CURRENT)
        .build()
        .expect("default and financial packs must compose");

    assert_eq!(
        scanner.rules_count(),
        builtins::CURRENT.len() + builtins::financial::CURRENT.len()
    );
}

#[test]
fn financial_pack_preserves_financial_rule_ownership_when_composed_with_default_builtins() {
    let scanner = Scanner::builder()
        .builtins(builtins::CURRENT)
        .builtins(builtins::financial::CURRENT)
        .build()
        .expect("default and financial packs must compose");

    let source = concat!(
        "iban=IT60X0542811101000000123456\n",
        "card_number=1234567890123452"
    );

    let results = scan_one(&scanner, source);
    let report = results.single_report().expect("one fixture was scanned");

    assert_eq!(report.len(), 2);

    let ids: Vec<_> = report
        .findings()
        .iter()
        .map(|finding| finding.rule_id().as_str())
        .collect();

    assert_eq!(ids, ["financial.iban", "financial.pan"]);
}

#[test]
fn financial_pan_is_network_agnostic() {
    // Synthetic Luhn-valid value used only to prove that PAN validation does
    // not depend on card-network, issuer or live BIN/IIN attribution.
    let pan = "1234567890123452";
    let source = format!("card_number={pan}");
    let scanner = scanner_for([builtins::financial::PAN]);

    let results = scan_one(&scanner, &source);
    let report = results.single_report().expect("one fixture was scanned");

    assert_eq!(report.len(), 1);
    assert_eq!(report.findings()[0].rule_id().as_str(), "financial.pan");
    assert_eq!(matched(&source, &report.findings()[0]), pan);
    assert_eq!(report.findings()[0].confidence(), Confidence::High);
}

#[test]
fn financial_pan_rejects_values_outside_supported_length_range() {
    let scanner = scanner_for([builtins::financial::PAN]);

    for source in ["card_number=123456789", "card_number=12345678901234567890"] {
        let results = scan_one(&scanner, source);
        let report = results.single_report().expect("one fixture was scanned");

        assert!(
            report.is_empty(),
            "PAN outside the supported 10..=19 digit range must not be detected"
        );
    }
}

#[test]
fn personal_codice_fiscale_is_opt_in_and_not_part_of_default_scanner() {
    let codice_fiscale = "RSSMRA85T10A562S";

    let default_results = scan_one(&Scanner::default(), codice_fiscale);
    let default_report = default_results
        .single_report()
        .expect("one fixture was scanned");

    assert!(
        default_report
            .findings()
            .iter()
            .all(|finding| finding.rule_id().as_str() != "personal.it-codice-fiscale")
    );

    let scanner = scanner_for(builtins::personal::CURRENT.iter().copied());
    let results = scan_one(&scanner, codice_fiscale);
    let report = results.single_report().expect("one fixture was scanned");

    assert_eq!(report.len(), 1);
    assert_eq!(
        report.findings()[0].rule_id().as_str(),
        "personal.it-codice-fiscale"
    );
}

#[test]
fn personal_codice_fiscale_reports_exact_span_and_metadata() {
    let codice_fiscale = "RSSMRA85T10A562S";
    let source = format!("codice_fiscale=({codice_fiscale})");
    let scanner = scanner_for([builtins::personal::CODICE_FISCALE]);

    let results = scan_one(&scanner, &source);
    let report = results.single_report().expect("one fixture was scanned");

    assert_eq!(report.len(), 1);

    let finding = &report.findings()[0];

    assert_eq!(finding.rule_id().as_str(), "personal.it-codice-fiscale");
    assert_eq!(matched(&source, finding), codice_fiscale);
    assert_eq!(finding.severity(), Severity::High);
    assert_eq!(finding.confidence(), Confidence::High);
    assert_eq!(
        finding.remediation(),
        Some(Remediation::RemoveSensitiveValue)
    );
}

#[test]
fn personal_codice_fiscale_accepts_valid_omocodic_representation() {
    let codice_fiscale = "RSSMRA85T10A56NH";
    let scanner = scanner_for([builtins::personal::CODICE_FISCALE]);

    let results = scan_one(&scanner, codice_fiscale);
    let report = results.single_report().expect("one fixture was scanned");

    assert_eq!(report.len(), 1);
    assert_eq!(
        report.findings()[0].rule_id().as_str(),
        "personal.it-codice-fiscale"
    );
    assert_eq!(
        matched(codice_fiscale, &report.findings()[0]),
        codice_fiscale
    );
}

#[test]
fn personal_codice_fiscale_accepts_punctuation_boundaries_without_including_them() {
    let codice_fiscale = "RSSMRA85T10A562S";
    let source = format!("({codice_fiscale}),");
    let scanner = scanner_for([builtins::personal::CODICE_FISCALE]);

    let results = scan_one(&scanner, &source);
    let report = results.single_report().expect("one fixture was scanned");

    assert_eq!(report.len(), 1);
    assert_eq!(matched(&source, &report.findings()[0]), codice_fiscale);
}

#[test]
fn personal_codice_fiscale_rejects_alphanumeric_adjacency() {
    let codice_fiscale = "RSSMRA85T10A562S";
    let scanner = scanner_for([builtins::personal::CODICE_FISCALE]);

    for source in [format!("X{codice_fiscale}"), format!("{codice_fiscale}X")] {
        let results = scan_one(&scanner, &source);
        let report = results.single_report().expect("one fixture was scanned");

        assert!(
            report.is_empty(),
            "Codice Fiscale embedded in a larger alphanumeric token must not be detected"
        );
    }
}

#[test]
fn personal_codice_fiscale_rejects_structurally_invalid_candidates() {
    let scanner = scanner_for([builtins::personal::CODICE_FISCALE]);

    for source in [
        "RSSMRA85Z10A562S", // invalid month
        "RSSMRA85T00A562S", // invalid day
        "RSSMRA85T32A562S", // invalid male day
        "RSSMRA85T40A562S", // invalid encoded day
        "RSSMRA85T72A562S", // invalid female day
        "RSSMRAW5T10A562S", // invalid omocodia letter in numeric position
        "RSSMRA85T101562S", // birthplace must start with a letter
        "RSSMRA85T10AA62S", // birthplace numeric position is malformed
        "RSSMRA85T10A562A", // invalid control character
    ] {
        let results = scan_one(&scanner, source);
        let report = results.single_report().expect("one fixture was scanned");

        assert!(
            report.is_empty(),
            "structurally invalid Codice Fiscale must not be detected: {source}"
        );
    }
}

#[test]
fn personal_codice_fiscale_does_not_claim_eleven_digit_tax_identifiers() {
    let scanner = scanner_for([builtins::personal::CODICE_FISCALE]);

    for source in ["12345678901", "codice_fiscale=12345678901"] {
        let results = scan_one(&scanner, source);
        let report = results.single_report().expect("one fixture was scanned");

        assert!(
            report.is_empty(),
            "11-digit numeric tax identifiers are outside this rule contract"
        );
    }
}

#[test]
fn personal_pack_composes_with_default_builtins_without_joining_default_pack() {
    let scanner = Scanner::builder()
        .builtins(builtins::CURRENT)
        .builtins(builtins::personal::CURRENT)
        .build()
        .expect("default and personal packs must compose");

    assert_eq!(
        scanner.rules_count(),
        builtins::CURRENT.len() + builtins::personal::CURRENT.len()
    );
}

#[test]
fn personal_pack_preserves_codice_fiscale_ownership_when_composed_with_default_builtins() {
    let codice_fiscale = "RSSMRA85T10A562S";

    let scanner = Scanner::builder()
        .builtins(builtins::CURRENT)
        .builtins(builtins::personal::CURRENT)
        .build()
        .expect("default and personal packs must compose");

    let results = scan_one(&scanner, codice_fiscale);
    let report = results.single_report().expect("one fixture was scanned");

    assert_eq!(report.len(), 1);
    assert_eq!(
        report.findings()[0].rule_id().as_str(),
        "personal.it-codice-fiscale"
    );
    assert_eq!(
        matched(codice_fiscale, &report.findings()[0]),
        codice_fiscale
    );
}

#[test]
fn personal_codice_fiscale_exposes_deterministic_detection_metadata() {
    let scanner = scanner_for([builtins::personal::CODICE_FISCALE]);

    let metadata = scanner
        .rule_metadata()
        .find(|metadata| metadata.id() == "personal.it-codice-fiscale")
        .expect("Codice Fiscale metadata must be exposed");

    assert_eq!(metadata.detection_mode(), DetectionMode::Deterministic);
}

#[test]
fn personal_pesel_is_opt_in_and_not_part_of_default_scanner() {
    let pesel = "02070803628";

    let default_results = scan_one(&Scanner::default(), pesel);
    let default_report = default_results
        .single_report()
        .expect("one fixture was scanned");

    assert!(
        default_report
            .findings()
            .iter()
            .all(|finding| finding.rule_id().as_str() != "personal.pl-pesel")
    );

    let scanner = scanner_for(builtins::personal::CURRENT.iter().copied());
    let results = scan_one(&scanner, pesel);
    let report = results.single_report().expect("one fixture was scanned");

    assert_eq!(report.len(), 1);
    assert_eq!(report.findings()[0].rule_id().as_str(), "personal.pl-pesel");
}

#[test]
fn personal_pesel_reports_exact_span_and_metadata() {
    let pesel = "02070803628";
    let source = format!("pesel=({pesel})");
    let scanner = scanner_for([builtins::personal::PESEL]);

    let results = scan_one(&scanner, &source);
    let report = results.single_report().expect("one fixture was scanned");

    assert_eq!(report.len(), 1);

    let finding = &report.findings()[0];

    assert_eq!(finding.rule_id().as_str(), "personal.pl-pesel");
    assert_eq!(matched(&source, finding), pesel);
    assert_eq!(finding.severity(), Severity::High);
    assert_eq!(finding.confidence(), Confidence::High);
    assert_eq!(
        finding.remediation(),
        Some(Remediation::RemoveSensitiveValue)
    );
}

#[test]
fn personal_pesel_exposes_deterministic_detection_metadata() {
    let scanner = scanner_for([builtins::personal::PESEL]);

    let metadata = scanner
        .rule_metadata()
        .find(|metadata| metadata.id() == "personal.pl-pesel")
        .expect("PESEL metadata must be exposed");

    assert_eq!(metadata.detection_mode(), DetectionMode::Deterministic);
}

#[test]
fn personal_pesel_accepts_punctuation_boundaries_without_including_them() {
    let pesel = "02070803628";
    let source = format!("({pesel}),");
    let scanner = scanner_for([builtins::personal::PESEL]);

    let results = scan_one(&scanner, &source);
    let report = results.single_report().expect("one fixture was scanned");

    assert_eq!(report.len(), 1);
    assert_eq!(matched(&source, &report.findings()[0]), pesel);
}

#[test]
fn personal_pesel_rejects_alphanumeric_adjacency() {
    let pesel = "02070803628";
    let scanner = scanner_for([builtins::personal::PESEL]);

    for source in [format!("X{pesel}"), format!("{pesel}X")] {
        let results = scan_one(&scanner, &source);
        let report = results.single_report().expect("one fixture was scanned");

        assert!(
            report.is_empty(),
            "PESEL embedded in a larger alphanumeric token must not be detected"
        );
    }
}

#[test]
fn personal_pesel_rejects_structurally_invalid_candidates() {
    let scanner = scanner_for([builtins::personal::PESEL]);

    for source in [
        "02070803629", // wrong checksum
        "02130803628", // invalid encoded month
        "02223003628", // impossible February date
        "02070003628", // day zero
    ] {
        let results = scan_one(&scanner, source);
        let report = results.single_report().expect("one fixture was scanned");

        assert!(
            report.is_empty(),
            "structurally invalid PESEL must not be detected: {source}"
        );
    }
}

#[test]
fn personal_pack_preserves_pesel_ownership_when_composed_with_default_builtins() {
    let pesel = "02070803628";

    let scanner = Scanner::builder()
        .builtins(builtins::CURRENT)
        .builtins(builtins::personal::CURRENT)
        .build()
        .expect("default and personal packs must compose");

    let results = scan_one(&scanner, pesel);
    let report = results.single_report().expect("one fixture was scanned");

    assert_eq!(report.len(), 1);
    assert_eq!(report.findings()[0].rule_id().as_str(), "personal.pl-pesel");
    assert_eq!(matched(pesel, &report.findings()[0]), pesel);
}

#[test]
fn personal_nhs_number_is_opt_in_and_not_part_of_default_scanner() {
    let source = "nhs_number=9434765919";

    let default_results = scan_one(&Scanner::default(), source);
    let default_report = default_results
        .single_report()
        .expect("one fixture was scanned");

    assert!(
        default_report
            .findings()
            .iter()
            .all(|finding| finding.rule_id().as_str() != "personal.uk-nhs-number")
    );

    let scanner = scanner_for(builtins::personal::CURRENT.iter().copied());
    let results = scan_one(&scanner, source);
    let report = results.single_report().expect("one fixture was scanned");

    assert_eq!(report.len(), 1);
    assert_eq!(
        report.findings()[0].rule_id().as_str(),
        "personal.uk-nhs-number"
    );
}

#[test]
fn personal_nhs_number_reports_exact_compact_span_and_metadata() {
    let nhs_number = "9434765919";
    let source = format!("nhs_number=({nhs_number})");
    let scanner = scanner_for([builtins::personal::NHS_NUMBER]);

    let results = scan_one(&scanner, &source);
    let report = results.single_report().expect("one fixture was scanned");

    assert_eq!(report.len(), 1);

    let finding = &report.findings()[0];

    assert_eq!(finding.rule_id().as_str(), "personal.uk-nhs-number");
    assert_eq!(matched(&source, finding), nhs_number);
    assert_eq!(finding.severity(), Severity::High);
    assert_eq!(finding.confidence(), Confidence::High);
    assert_eq!(
        finding.remediation(),
        Some(Remediation::RemoveSensitiveValue)
    );
}

#[test]
fn personal_nhs_number_exposes_contextual_detection_metadata() {
    let scanner = scanner_for([builtins::personal::NHS_NUMBER]);

    let metadata = scanner
        .rule_metadata()
        .find(|metadata| metadata.id() == "personal.uk-nhs-number")
        .expect("NHS Number metadata must be exposed");

    assert_eq!(metadata.detection_mode(), DetectionMode::Contextual);
}

#[test]
fn personal_nhs_number_preserves_formatted_source_span() {
    let scanner = scanner_for([builtins::personal::NHS_NUMBER]);

    for nhs_number in ["943 476 5919", "943-476-5919"] {
        let source = format!("nhs_number=({nhs_number})");

        let results = scan_one(&scanner, &source);
        let report = results.single_report().expect("one fixture was scanned");

        assert_eq!(report.len(), 1);
        assert_eq!(
            report.findings()[0].rule_id().as_str(),
            "personal.uk-nhs-number"
        );
        assert_eq!(matched(&source, &report.findings()[0]), nhs_number);
    }
}

#[test]
fn personal_nhs_number_rejects_bare_checksum_valid_value() {
    let nhs_number = "9434765919";
    let scanner = scanner_for([builtins::personal::NHS_NUMBER]);

    let results = scan_one(&scanner, nhs_number);
    let report = results.single_report().expect("one fixture was scanned");

    assert!(
        report.is_empty(),
        "checksum-valid NHS Number without NHS-specific context must not be classified"
    );
}

#[test]
fn personal_nhs_number_rejects_unrelated_context() {
    let scanner = scanner_for([builtins::personal::NHS_NUMBER]);

    for source in [
        "number=9434765919",
        "patient_number=9434765919",
        "account_number=9434765919",
        "reference=9434765919",
    ] {
        let results = scan_one(&scanner, source);
        let report = results.single_report().expect("one fixture was scanned");

        assert!(
            report.is_empty(),
            "unrelated numeric context must not classify an NHS Number: {source}"
        );
    }
}

#[test]
fn personal_nhs_number_rejects_invalid_values_and_noncanonical_formatting() {
    let scanner = scanner_for([builtins::personal::NHS_NUMBER]);

    for source in [
        "nhs_number=9434765918",
        "nhs_number=1234567890",
        "nhs_number=0123456789",
        "nhs_number=943 476-5919",
        "nhs_number=943.476.5919",
    ] {
        let results = scan_one(&scanner, source);
        let report = results.single_report().expect("one fixture was scanned");

        assert!(
            report.is_empty(),
            "invalid NHS Number must not be detected: {source}"
        );
    }
}

#[test]
fn personal_nhs_number_rejects_alphanumeric_adjacency() {
    let scanner = scanner_for([builtins::personal::NHS_NUMBER]);

    for source in ["nhs_number=X9434765919", "nhs_number=9434765919X"] {
        let results = scan_one(&scanner, source);
        let report = results.single_report().expect("one fixture was scanned");

        assert!(
            report.is_empty(),
            "NHS Number embedded in a larger alphanumeric token must not be detected"
        );
    }
}

#[test]
fn personal_pack_preserves_nhs_number_ownership_when_composed_with_default_builtins() {
    let source = "nhs_number=9434765919";

    let scanner = Scanner::builder()
        .builtins(builtins::CURRENT)
        .builtins(builtins::personal::CURRENT)
        .build()
        .expect("default and personal packs must compose");

    let results = scan_one(&scanner, source);
    let report = results.single_report().expect("one fixture was scanned");

    assert_eq!(report.len(), 1);
    assert_eq!(
        report.findings()[0].rule_id().as_str(),
        "personal.uk-nhs-number"
    );
    assert_eq!(matched(source, &report.findings()[0]), "9434765919");
}

#[test]
fn personal_ssn_is_opt_in_and_not_part_of_default_scanner() {
    let source = "ssn=123-45-6789";

    let default_results = scan_one(&Scanner::default(), source);
    let default_report = default_results
        .single_report()
        .expect("one fixture was scanned");

    assert!(
        default_report
            .findings()
            .iter()
            .all(|finding| finding.rule_id().as_str() != "personal.us-ssn")
    );

    let scanner = scanner_for(builtins::personal::CURRENT.iter().copied());
    let results = scan_one(&scanner, source);
    let report = results.single_report().expect("one fixture was scanned");

    assert_eq!(report.len(), 1);
    assert_eq!(report.findings()[0].rule_id().as_str(), "personal.us-ssn");
}

#[test]
fn personal_ssn_reports_exact_span_and_metadata() {
    let ssn = "123-45-6789";
    let source = format!("ssn=({ssn})");
    let scanner = scanner_for([builtins::personal::SSN]);

    let results = scan_one(&scanner, &source);
    let report = results.single_report().expect("one fixture was scanned");

    assert_eq!(report.len(), 1);

    let finding = &report.findings()[0];

    assert_eq!(finding.rule_id().as_str(), "personal.us-ssn");
    assert_eq!(matched(&source, finding), ssn);
    assert_eq!(finding.severity(), Severity::High);
    assert_eq!(finding.confidence(), Confidence::High);
    assert_eq!(
        finding.remediation(),
        Some(Remediation::RemoveSensitiveValue)
    );
}

#[test]
fn personal_ssn_exposes_contextual_detection_metadata() {
    let scanner = scanner_for([builtins::personal::SSN]);

    let metadata = scanner
        .rule_metadata()
        .find(|metadata| metadata.id() == "personal.us-ssn")
        .expect("SSN metadata must be exposed");

    assert_eq!(metadata.detection_mode(), DetectionMode::Contextual);
}

#[test]
fn personal_ssn_preserves_compact_and_hyphenated_source_spans() {
    let scanner = scanner_for([builtins::personal::SSN]);

    for ssn in ["123456789", "123-45-6789"] {
        let source = format!("ssn=({ssn})");

        let results = scan_one(&scanner, &source);
        let report = results.single_report().expect("one fixture was scanned");

        assert_eq!(report.len(), 1);
        assert_eq!(report.findings()[0].rule_id().as_str(), "personal.us-ssn");
        assert_eq!(matched(&source, &report.findings()[0]), ssn);
    }
}

#[test]
fn personal_ssn_rejects_bare_structurally_valid_values() {
    let scanner = scanner_for([builtins::personal::SSN]);

    for ssn in ["123456789", "123-45-6789"] {
        let results = scan_one(&scanner, ssn);
        let report = results.single_report().expect("one fixture was scanned");

        assert!(
            report.is_empty(),
            "bare structurally valid SSN-like value must not be classified: {ssn}"
        );
    }
}

#[test]
fn personal_ssn_rejects_unrelated_context() {
    let scanner = scanner_for([builtins::personal::SSN]);

    for source in [
        "number=123-45-6789",
        "tax_id=123-45-6789",
        "national_id=123-45-6789",
        "employee_id=123-45-6789",
        "social_security=123-45-6789",
    ] {
        let results = scan_one(&scanner, source);
        let report = results.single_report().expect("one fixture was scanned");

        assert!(
            report.is_empty(),
            "unrelated context must not classify an SSN: {source}"
        );
    }
}

#[test]
fn personal_ssn_rejects_structurally_impossible_values() {
    let scanner = scanner_for([builtins::personal::SSN]);

    for source in [
        "ssn=000-45-6789",
        "ssn=666-45-6789",
        "ssn=900-45-6789",
        "ssn=999-45-6789",
        "ssn=123-00-6789",
        "ssn=123-45-0000",
    ] {
        let results = scan_one(&scanner, source);
        let report = results.single_report().expect("one fixture was scanned");

        assert!(
            report.is_empty(),
            "structurally impossible SSN must not be detected: {source}"
        );
    }
}

#[test]
fn personal_ssn_rejects_alphanumeric_adjacency() {
    let scanner = scanner_for([builtins::personal::SSN]);

    for source in [
        "ssn=X123456789",
        "ssn=123456789X",
        "ssn=X123-45-6789",
        "ssn=123-45-6789X",
    ] {
        let results = scan_one(&scanner, source);
        let report = results.single_report().expect("one fixture was scanned");

        assert!(
            report.is_empty(),
            "SSN embedded in a larger alphanumeric token must not be detected"
        );
    }
}

#[test]
fn personal_pack_preserves_ssn_ownership_when_composed_with_default_builtins() {
    let source = "ssn=123-45-6789";

    let scanner = Scanner::builder()
        .builtins(builtins::CURRENT)
        .builtins(builtins::personal::CURRENT)
        .build()
        .expect("default and personal packs must compose");

    let results = scan_one(&scanner, source);
    let report = results.single_report().expect("one fixture was scanned");

    assert_eq!(report.len(), 1);
    assert_eq!(report.findings()[0].rule_id().as_str(), "personal.us-ssn");
    assert_eq!(matched(source, &report.findings()[0]), "123-45-6789");
}

#[test]
fn otp_provisioning_secret_is_part_of_default_security_portfolio() {
    let secret = "JBSWY3DPEHPK3PXP";
    let source = format!("totp_secret={secret}");

    let results = scan_one(&Scanner::default(), &source);
    let report = results.single_report().expect("one fixture was scanned");

    let finding = report
        .findings()
        .iter()
        .find(|finding| finding.rule_id().as_str() == "mfa.otp-provisioning-secret")
        .expect("OTP provisioning secret");

    assert_eq!(matched(&source, finding), secret);
    assert_eq!(finding.severity(), Severity::Critical);
    assert_eq!(finding.confidence(), Confidence::High);
    assert_eq!(finding.remediation(), Some(Remediation::RotateCredential));
}

#[test]
fn otp_provisioning_secret_projects_only_secret_from_totp_uri() {
    let secret = "JBSWY3DPEHPK3PXP";
    let source = format!("otpauth://totp/example:user@example.com?secret={secret}&issuer=Example");

    let results = scan_one(&Scanner::default(), &source);
    let report = results.single_report().expect("one fixture was scanned");

    let finding = report
        .findings()
        .iter()
        .find(|finding| finding.rule_id().as_str() == "mfa.otp-provisioning-secret")
        .expect("OTP provisioning secret");

    assert_eq!(matched(&source, finding), secret);
}

#[test]
fn otp_provisioning_secret_projects_only_secret_from_hotp_uri() {
    let secret = "JBSWY3DPEHPK3PXP";
    let source = format!("otpauth://hotp/example?counter=0&secret={secret}&issuer=Example");

    let results = scan_one(&Scanner::default(), &source);
    let report = results.single_report().expect("one fixture was scanned");

    let finding = report
        .findings()
        .iter()
        .find(|finding| finding.rule_id().as_str() == "mfa.otp-provisioning-secret")
        .expect("OTP provisioning secret");

    assert_eq!(matched(&source, finding), secret);
}

#[test]
fn otp_provisioning_secret_supports_explicit_otp_fields() {
    let secret = "JBSWY3DPEHPK3PXP";

    for key in ["otp_secret", "totp_secret", "hotp_secret"] {
        let source = format!("{key}={secret}");
        let results = scan_one(&Scanner::default(), &source);
        let report = results.single_report().expect("one fixture was scanned");

        let matching: Vec<_> = report
            .findings()
            .iter()
            .filter(|finding| finding.rule_id().as_str() == "mfa.otp-provisioning-secret")
            .collect();

        assert_eq!(matching.len(), 1, "unexpected result for {key:?}");
        assert_eq!(matched(&source, matching[0]), secret);
    }
}

#[test]
fn otp_provisioning_secret_rejects_bare_base32_material() {
    let source = "JBSWY3DPEHPK3PXP";

    let results = scan_one(&Scanner::default(), source);
    let report = results.single_report().expect("one fixture was scanned");

    assert!(
        report
            .findings()
            .iter()
            .all(|finding| finding.rule_id().as_str() != "mfa.otp-provisioning-secret")
    );
}

#[test]
fn otp_provisioning_secret_rejects_non_otp_uri_context() {
    let secret = "JBSWY3DPEHPK3PXP";
    let source = format!("https://example.com/account?secret={secret}");

    let results = scan_one(&Scanner::default(), &source);
    let report = results.single_report().expect("one fixture was scanned");

    assert!(
        report
            .findings()
            .iter()
            .all(|finding| finding.rule_id().as_str() != "mfa.otp-provisioning-secret")
    );
}

#[test]
fn otp_provisioning_secret_outranks_generic_secret_for_same_span() {
    let secret = "JBSWY3DPEHPK3PXP";
    let source = format!("otp_secret={secret}");

    let scanner = scanner_for([builtins::GENERIC_SECRET, builtins::OTP_PROVISIONING_SECRET]);

    let results = scan_one(&scanner, &source);
    let report = results.single_report().expect("one fixture was scanned");

    assert_eq!(report.len(), 1);
    assert_eq!(
        report.findings()[0].rule_id().as_str(),
        "mfa.otp-provisioning-secret"
    );
    assert_eq!(matched(&source, &report.findings()[0]), secret);
}

#[test]
fn otp_provisioning_secret_exposes_contextual_detection_metadata() {
    let scanner = scanner_for([builtins::OTP_PROVISIONING_SECRET]);

    let metadata = scanner
        .rule_metadata()
        .find(|metadata| metadata.id() == "mfa.otp-provisioning-secret")
        .expect("OTP provisioning metadata");

    assert_eq!(metadata.detection_mode(), DetectionMode::Contextual);
}
