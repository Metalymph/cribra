//! Public ShareBundle orchestration tests.

use cribra::{
    Rule, Scanner, Severity,
    transform::{PseudonymizationOptions, ShareBundle, ShareMode, ShareModeKind, SynthesisOptions},
};

#[test]
fn public_share_bundle_transforms_multiple_sources_in_input_order() {
    let scanner = Scanner::builder()
        .rule(Rule::literal("secret", "SECRET", Severity::Critical))
        .build()
        .expect("scanner should build");

    let first = "A=SECRET";
    let second = "clean";
    let third = "C=SECRET";

    let results = scanner.scan([("a.env", first), ("b.env", second), ("c.env", third)]);

    let bundle = ShareBundle::builder()
        .mode(ShareMode::Redact)
        .build(&results, [first, second, third])
        .unwrap();

    assert_eq!(
        bundle
            .sources()
            .iter()
            .map(|source| *source.key())
            .collect::<Vec<_>>(),
        ["a.env", "b.env", "c.env"],
    );
    assert_eq!(bundle.sources()[0].content(), "A=[REDACTED]");
    assert_eq!(bundle.sources()[1].content(), "clean");
    assert_eq!(bundle.sources()[2].content(), "C=[REDACTED]");
    assert_eq!(bundle.summary(), results.summary());
}

#[test]
fn public_share_bundle_supports_every_share_mode() {
    let scanner = Scanner::builder()
        .rule(Rule::literal("secret", "SECRET", Severity::Critical))
        .build()
        .expect("scanner should build");

    let source = "TOKEN=SECRET";
    let results = scanner.scan([("memory", source)]);

    let modes = [
        ShareMode::Redact,
        ShareMode::Template,
        ShareMode::Pseudonymize(PseudonymizationOptions::new([3; 32])),
        ShareMode::Synthesize(SynthesisOptions::new([4; 32])),
    ];

    let expected = [
        ShareModeKind::Redact,
        ShareModeKind::Template,
        ShareModeKind::Pseudonymize,
        ShareModeKind::Synthesize,
    ];

    for (mode, expected_kind) in modes.into_iter().zip(expected) {
        let bundle = ShareBundle::builder()
            .mode(mode)
            .build(&results, [source])
            .unwrap();

        assert_eq!(bundle.manifest().mode(), expected_kind);
        assert_ne!(bundle.sources()[0].content(), source);
    }
}

#[cfg(feature = "parallel")]
#[test]
fn share_bundle_accepts_parallel_scan_results_without_semantic_difference() {
    let scanner = Scanner::builder()
        .rule(Rule::literal("secret", "SECRET", Severity::Critical))
        .build()
        .expect("scanner should build");

    let sources = ["A=SECRET", "clean", "C=SECRET"];
    let serial = scanner.scan([("a", sources[0]), ("b", sources[1]), ("c", sources[2])]);
    let parallel = scanner.parallel_scan([("a", sources[0]), ("b", sources[1]), ("c", sources[2])]);

    let serial_bundle = ShareBundle::builder()
        .mode(ShareMode::Template)
        .build(&serial, sources)
        .unwrap();

    let parallel_bundle = ShareBundle::builder()
        .mode(ShareMode::Template)
        .build(&parallel, sources)
        .unwrap();

    assert_eq!(serial_bundle.sources(), parallel_bundle.sources());
    assert_eq!(serial_bundle.summary(), parallel_bundle.summary());
    assert_eq!(
        serial_bundle.manifest().mode(),
        parallel_bundle.manifest().mode(),
    );
}
