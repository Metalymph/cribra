//! Collision, overlap and multi-detector regression contract for Cribra v0.2.

use cribra::{Rule, Scanner, Severity, builtins};

const TOKEN: &str = "ghp_AbCdEf0123456789_AbCdEf0123456789";
const CANDIDATE: &str = "QRST-UVWX-YZ12-3456";

#[test]
fn equal_priority_identical_custom_spans_are_ordered_by_rule_id_not_insertion_order() {
    fn ids(reverse: bool) -> Vec<String> {
        let first = Rule::literal("zeta.rule", "shared-secret", Severity::High);
        let second = Rule::literal("alpha.rule", "shared-secret", Severity::High);

        let scanner = if reverse {
            Scanner::builder().rule(second).rule(first)
        } else {
            Scanner::builder().rule(first).rule(second)
        }
        .build()
        .expect("scanner should compile");

        scanner
            .scan([("source", "shared-secret")])
            .single_report()
            .expect("one source")
            .findings()
            .iter()
            .map(|finding| finding.rule_id().as_str().to_owned())
            .collect()
    }

    assert_eq!(ids(false), ["alpha.rule", "zeta.rule"]);
    assert_eq!(ids(false), ids(true));
}

#[test]
fn partial_overlaps_remain_independently_observable_and_position_ordered() {
    let scanner = Scanner::builder()
        .rule(Rule::literal("whole", "secret-value", Severity::High))
        .rule(Rule::literal("prefix", "secret", Severity::Medium))
        .rule(Rule::literal("suffix", "value", Severity::Low))
        .build()
        .expect("scanner should compile");

    let results = scanner.scan([("source", "secret-value")]);
    let report = results.single_report().expect("one source");
    let spans = report
        .findings()
        .iter()
        .map(|finding| {
            (
                finding.rule_id().as_str(),
                finding.location().start(),
                finding.location().end(),
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(
        spans,
        [("prefix", 0, 6), ("whole", 0, 12), ("suffix", 7, 12),]
    );
}

#[test]
fn custom_finding_promotes_ambiguous_span_and_suppresses_candidate() {
    let scanner = Scanner::builder()
        .builtins(builtins::CURRENT)
        .rule(Rule::literal(
            "domain.recovery-code",
            CANDIDATE,
            Severity::Critical,
        ))
        .build()
        .expect("scanner should compile");

    let results = scanner.scan([("source", CANDIDATE)]);
    let report = results.single_report().expect("one source");

    assert_eq!(report.findings().len(), 1);
    assert_eq!(
        report.findings()[0].rule_id().as_str(),
        "domain.recovery-code"
    );
    assert!(report.candidates().is_empty());
}

#[test]
fn nearby_non_overlapping_finding_and_candidate_remain_separate() {
    let source = format!("{TOKEN},{CANDIDATE}");
    let scanner = Scanner::default();
    let results = scanner.scan([("source", source.as_str())]);
    let report = results.single_report().expect("one source");

    assert!(
        report
            .findings()
            .iter()
            .any(|finding| finding.rule_id().as_str() == "github.classic-pat")
    );
    assert_eq!(report.candidate_len(), 1);
    assert_eq!(
        &source[report.candidates()[0].location().byte_range()],
        CANDIDATE
    );
}

#[test]
fn multiple_findings_on_one_line_are_sorted_by_source_position() {
    let scanner = Scanner::builder()
        .rule(Rule::literal("third", "THREE", Severity::Low))
        .rule(Rule::literal("first", "ONE", Severity::Critical))
        .rule(Rule::literal("second", "TWO", Severity::High))
        .build()
        .expect("scanner should compile");

    let results = scanner.scan([("source", "ONE TWO THREE")]);
    let report = results.single_report().expect("one source");

    let ids = report
        .findings()
        .iter()
        .map(|finding| finding.rule_id().as_str())
        .collect::<Vec<_>>();

    assert_eq!(ids, ["first", "second", "third"]);
}

#[cfg(feature = "parallel")]
#[test]
fn collision_heavy_batch_is_serial_parallel_equivalent() {
    let scanner = Scanner::builder()
        .builtins(builtins::CURRENT)
        .rule(Rule::literal(
            "domain.recovery-code",
            CANDIDATE,
            Severity::Critical,
        ))
        .rule(Rule::literal(
            "overlap.whole",
            "secret-value",
            Severity::High,
        ))
        .rule(Rule::literal("overlap.part", "secret", Severity::Medium))
        .build()
        .expect("scanner should compile");

    let sources = [
        ("provider", TOKEN),
        ("promoted", CANDIDATE),
        ("overlap", "secret-value"),
        ("mixed", "ordinary text"),
    ];

    let serial = scanner.scan(sources);
    let parallel = scanner.parallel_scan(sources);

    assert_eq!(serial, parallel);
}

#[test]
fn gitlab_provider_finding_wins_over_generic_token_on_the_same_span() {
    const GITLAB_TOKEN: &str = "glpat-AbCdEf0123456789_AbCdEf0123456789";
    let source = format!("token={GITLAB_TOKEN}");

    let scanner = Scanner::default();
    let results = scanner.scan([("source", source.as_str())]);
    let report = results.single_report().expect("one source");

    let matching = report
        .findings()
        .iter()
        .filter(|finding| &source[finding.location().byte_range()] == GITLAB_TOKEN)
        .collect::<Vec<_>>();

    assert_eq!(matching.len(), 1);
    assert_eq!(matching[0].rule_id().as_str(), "gitlab.access-token");
}

#[cfg(feature = "parallel")]
#[test]
fn v043_detection_families_are_serial_parallel_equivalent() {
    let sources = [
        ("gitlab", "glpat-AbCdEf0123456789_AbCdEf0123456789"),
        (
            "database",
            "postgresql://cribra:CorrectHorseBatteryStaple@localhost/app",
        ),
        (
            "quoted-password",
            r#"password="Correct Horse Battery Staple""#,
        ),
        (
            "clean",
            "sha256=0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        ),
    ];

    let scanner = Scanner::default();

    let serial = scanner.scan(sources);
    let parallel = scanner.parallel_scan(sources);

    assert_eq!(serial, parallel);
}

#[test]
fn v047_families_preserve_semantic_ownership_when_composed() {
    const OTP_SECRET: &str = "JBSWY3DPEHPK3PXP";
    const TAILSCALE_AUTH_KEY: &str = "tskey-auth-a1B2c3D4e5F6";
    const NATS_SEED: &str = "SUAB6M5NSNGSXV6SQ3TV6LQUH6S2OFHGJDZ2ENNZ4W5VBKBX3Z6TH3RW4E";

    let source = format!(
        "otp_secret={OTP_SECRET}\n\
         tailscale_auth_key={TAILSCALE_AUTH_KEY}\n\
         nats_seed={NATS_SEED}\n\
         ssn=123-45-6789\n\
         card_number=4111111111111111\n\
         cvv=123\n"
    );

    let scanner = Scanner::builder()
        .builtins(builtins::CURRENT)
        .builtins(builtins::personal::CURRENT)
        .builtins(builtins::financial::CURRENT)
        .build()
        .expect("default, personal, and financial packs must compose");

    let results = scanner.scan([("source", source.as_str())]);
    let report = results.single_report().expect("one source");

    for (value, expected_rule) in [
        (OTP_SECRET, "mfa.otp-provisioning-secret"),
        (TAILSCALE_AUTH_KEY, "tailscale.auth-key"),
        (NATS_SEED, "nats.nkey-seed"),
        ("123-45-6789", "personal.us-ssn"),
        ("4111111111111111", "financial.pan"),
        ("123", "financial.card-verification-code"),
    ] {
        let matching = report
            .findings()
            .iter()
            .filter(|finding| &source[finding.location().byte_range()] == value)
            .collect::<Vec<_>>();

        assert_eq!(
            matching.len(),
            1,
            "expected exactly one semantic owner for {value:?}; got {:?}",
            matching
                .iter()
                .map(|finding| finding.rule_id().as_str())
                .collect::<Vec<_>>(),
        );
        assert_eq!(matching[0].rule_id().as_str(), expected_rule);
    }
}

#[test]
fn v047_specific_security_rules_win_exact_span_generic_collisions() {
    const OTP_SECRET: &str = "JBSWY3DPEHPK3PXP";
    const TAILSCALE_AUTH_KEY: &str = "tskey-auth-a1B2c3D4e5F6";

    let cases = [
        (
            format!("secret=ordinary-value\notp_secret={OTP_SECRET}"),
            OTP_SECRET,
            "mfa.otp-provisioning-secret",
        ),
        (
            format!("token={TAILSCALE_AUTH_KEY}"),
            TAILSCALE_AUTH_KEY,
            "tailscale.auth-key",
        ),
    ];

    for (source, value, expected_rule) in cases {
        let scanner = Scanner::default();
        let results = scanner.scan([("source", source.as_str())]);
        let report = results.single_report().expect("one source");

        let matching = report
            .findings()
            .iter()
            .filter(|finding| &source[finding.location().byte_range()] == value)
            .collect::<Vec<_>>();

        assert_eq!(
            matching.len(),
            1,
            "expected exactly one owner for exact span {value:?}; got {:?}",
            matching
                .iter()
                .map(|finding| finding.rule_id().as_str())
                .collect::<Vec<_>>(),
        );

        assert_eq!(matching[0].rule_id().as_str(), expected_rule);
    }
}

#[test]
fn v047_identifier_and_verifier_categories_do_not_collapse_into_generic_credentials() {
    let source = concat!(
        "ssn=123-45-6789\n",
        "nhs_number=9434765919\n",
        "alice:$6$abcdefghijklmnop$0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789:20000:0:99999:7:::\n",
        "bob:$apr1$hfT7jp2q$2VbDVlM1QY3wP1uQYxJYB/\n",
    );

    let scanner = Scanner::builder()
        .builtins(builtins::CURRENT)
        .builtins(builtins::personal::CURRENT)
        .build()
        .expect("default and personal packs must compose");

    let results = scanner.scan([("source", source)]);
    let report = results.single_report().expect("one source");

    for expected in [
        "personal.us-ssn",
        "personal.uk-nhs-number",
        "system.shadow-password-verifier",
        "system.htpasswd-password-verifier",
    ] {
        assert!(
            report
                .findings()
                .iter()
                .any(|finding| finding.rule_id().as_str() == expected),
            "missing semantic owner {expected}",
        );
    }

    for finding in report.findings() {
        let id = finding.rule_id().as_str();

        if matches!(
            id,
            "personal.us-ssn"
                | "personal.uk-nhs-number"
                | "system.shadow-password-verifier"
                | "system.htpasswd-password-verifier"
        ) {
            assert!(
                !id.starts_with("generic."),
                "specific identifier/verifier semantics collapsed into generic ownership"
            );
        }
    }
}

#[test]
fn v047_security_references_placeholders_and_public_material_remain_clean() {
    for source in [
        "totp_secret=YOUR_SECRET_HERE",
        "hotp_secret=EXAMPLE_SECRET",
        "otp_secret=xxxxxxxxxxxxxxxx",
        "tskey-api-your_token_here",
        "tskey-auth-example_token_here",
        "tskey-client-xxxxxxxx",
        "tskey-scim-your_key_here",
        "tskey-webhook-example_key",
        // Public NKey: valid public material, not a credential.
        "OBLXELMC2C6DOHNN47MCOP6BSWKPJOT4XJWIVCU2N2T63A4UT5OQXB6K",
    ] {
        let results = Scanner::default().scan([("source", source)]);
        let report = results.single_report().expect("one source");

        assert!(
            report.findings().is_empty(),
            "reference/placeholder/configuration material produced findings for {source:?}: {:?}",
            report
                .findings()
                .iter()
                .map(|finding| finding.rule_id().as_str())
                .collect::<Vec<_>>(),
        );
    }
}

#[test]
fn v047_malformed_security_material_is_not_promoted() {
    let source = concat!(
        // Tailscale near misses.
        "tskey-auth-\n",
        "tskey-unknown-a1B2c3D4e5F6\n",
        "tskey-AUTH-a1B2c3D4e5F6\n",
        // OTP-like material without authoritative OTP context.
        "JBSWY3DPEHPK3PXP\n",
        "https://example.com/?secret=JBSWY3DPEHPK3PXP\n",
        // NKey near misses / noncanonical material.
        "suaafo5zrymbov7kynbjpwvkoqlfapz6sfrkxnvf32ucpxfdm45hend4ci\n",
        "SOAAFO5ZRYMBOV7KYNBJPWVKOQLFAPZ6SFRKXNVF32UCPXFDM45HEND40I\n",
        // Password-verifier-looking material without its required representation.
        "$apr1$hfT7jp2q$2VbDVlM1QY3wP1uQYxJYB/\n",
        "$6$abcdefghijklmnop$0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789\n",
    );

    let scanner = Scanner::default();
    let results = scanner.scan([("v047-malformed", source)]);
    let report = results.single_report().expect("one source");

    assert!(
        report.findings().iter().all(|finding| {
            !matches!(
                finding.rule_id().as_str(),
                "mfa.otp-provisioning-secret"
                    | "tailscale.api-access-token"
                    | "tailscale.auth-key"
                    | "tailscale.oauth-client-secret"
                    | "tailscale.scim-key"
                    | "tailscale.webhook-key"
                    | "nats.nkey-seed"
                    | "nats.nkey-private-key"
            )
        }),
        "malformed/non-authoritative material was promoted to specific ownership: {:?}",
        report
            .findings()
            .iter()
            .map(|finding| finding.rule_id().as_str())
            .collect::<Vec<_>>()
    );

    assert!(
        report.candidates().is_empty(),
        "malformed/non-authoritative material produced review candidates"
    );
}

#[test]
fn v047_valid_security_material_survives_adversarial_surroundings_with_exact_spans() {
    const OTP: &str = "JBSWY3DPEHPK3PXP";
    const TAILSCALE: &str = "tskey-auth-a1B2c3D4e5F6";
    const NATS: &str = "SUAB6M5NSNGSXV6SQ3TV6LQUH6S2OFHGJDZ2ENNZ4W5VBKBX3Z6TH3RW4E";

    let source = format!(
        "docs say TOTP_SECRET is required\n\
         totp_secret=({OTP})\n\
         tailscale_auth_key=\"{TAILSCALE}\"\n\
         nats_seed=[{NATS}]\n"
    );

    let scanner = Scanner::default();
    let results = scanner.scan([("v047-boundaries", source.as_str())]);
    let report = results.single_report().expect("one source");

    for (value, expected_rule) in [
        (OTP, "mfa.otp-provisioning-secret"),
        (TAILSCALE, "tailscale.auth-key"),
        (NATS, "nats.nkey-seed"),
    ] {
        let matching = report
            .findings()
            .iter()
            .filter(|finding| &source[finding.location().byte_range()] == value)
            .collect::<Vec<_>>();

        assert_eq!(
            matching.len(),
            1,
            "expected one exact-span finding for {value:?}; got {:?}",
            matching
                .iter()
                .map(|finding| finding.rule_id().as_str())
                .collect::<Vec<_>>(),
        );
        assert_eq!(matching[0].rule_id().as_str(), expected_rule);
    }
}

#[test]
fn v047_personal_and_financial_valid_values_keep_exact_semantic_ownership() {
    let source = concat!(
        "codice_fiscale=(RSSMRA85T10A562S)\n",
        "pesel=[02070803628]\n",
        "nhs_number=\"943 476 5919\"\n",
        "ssn=(123-45-6789)\n",
        "iban=[GB82WEST12345698765432]\n",
        "card_number=(4111111111111111)\n",
        "cvv=123\n",
    );

    let scanner = Scanner::builder()
        .builtins(builtins::CURRENT)
        .builtins(builtins::personal::CURRENT)
        .builtins(builtins::financial::CURRENT)
        .build()
        .expect("default, personal, and financial packs must compose");

    let results = scanner.scan([("v047-structured-positive", source)]);
    let report = results.single_report().expect("one source");

    for (value, expected_rule) in [
        ("RSSMRA85T10A562S", "personal.it-codice-fiscale"),
        ("02070803628", "personal.pl-pesel"),
        ("943 476 5919", "personal.uk-nhs-number"),
        ("123-45-6789", "personal.us-ssn"),
        ("GB82WEST12345698765432", "financial.iban"),
        ("4111111111111111", "financial.pan"),
        ("123", "financial.card-verification-code"),
    ] {
        let matching = report
            .findings()
            .iter()
            .filter(|finding| &source[finding.location().byte_range()] == value)
            .collect::<Vec<_>>();

        assert_eq!(
            matching.len(),
            1,
            "expected exactly one semantic owner for {value:?}; got {:?}",
            matching
                .iter()
                .map(|finding| finding.rule_id().as_str())
                .collect::<Vec<_>>(),
        );
        assert_eq!(matching[0].rule_id().as_str(), expected_rule);
    }
}

#[test]
fn v047_structured_identifiers_reject_invalid_and_unrelated_near_misses() {
    let source = concat!(
        // Codice Fiscale: invalid control/structure.
        "codice_fiscale=RSSMRA85T10A562A\n",
        // PESEL: invalid checksum.
        "pesel=02070803629\n",
        // NHS: invalid checksum.
        "nhs_number=9434765918\n",
        // SSN: structurally impossible groups.
        "ssn=000-45-6789\n",
        "ssn=123-00-6789\n",
        "ssn=123-45-0000\n",
        // IBAN: invalid checksum.
        "iban=GB82WEST12345698765431\n",
        // PAN: invalid Luhn.
        "card_number=4111111111111112\n",
    );

    let scanner = Scanner::builder()
        .builtins(builtins::CURRENT)
        .builtins(builtins::personal::CURRENT)
        .builtins(builtins::financial::CURRENT)
        .build()
        .expect("default, personal, and financial packs must compose");

    let results = scanner.scan([("v047-structured-invalid", source)]);
    let report = results.single_report().expect("one source");

    assert!(
        report.findings().iter().all(|finding| {
            !finding.rule_id().as_str().starts_with("personal.")
                && !finding.rule_id().as_str().starts_with("financial.")
        }),
        "invalid structured identifiers were promoted: {:?}",
        report
            .findings()
            .iter()
            .map(|finding| finding.rule_id().as_str())
            .collect::<Vec<_>>()
    );
}

#[test]
fn v047_context_required_identifiers_do_not_promote_bare_values() {
    let source = concat!(
        // Structurally plausible/valid but context-sensitive.
        "9434765919\n",
        "123-45-6789\n",
        "4111111111111111\n",
        "123\n",
    );

    let scanner = Scanner::builder()
        .builtins(builtins::CURRENT)
        .builtins(builtins::personal::CURRENT)
        .builtins(builtins::financial::CURRENT)
        .build()
        .expect("default, personal, and financial packs must compose");

    let results = scanner.scan([("v047-context-required", source)]);
    let report = results.single_report().expect("one source");

    for rule_id in [
        "personal.uk-nhs-number",
        "personal.us-ssn",
        "financial.pan",
        "financial.card-verification-code",
    ] {
        assert!(
            report
                .findings()
                .iter()
                .all(|finding| finding.rule_id().as_str() != rule_id),
            "{rule_id} must require authoritative context"
        );
    }
}
