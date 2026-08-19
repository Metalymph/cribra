use std::{hint::black_box, time::Duration};

use cribra::{RuleSpec, Scanner, builtins};
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};

const SIZE: usize = 64 * 1024;

const DETERMINISTIC_PREFIXES: &[RuleSpec] = &[
    builtins::GITHUB_CLASSIC_PAT,
    builtins::GITHUB_FINE_GRAINED_PAT,
    builtins::GITHUB_OAUTH_TOKEN,
    builtins::GITHUB_APP_USER_TOKEN,
    builtins::GITHUB_APP_INSTALLATION_TOKEN,
    builtins::GITHUB_APP_REFRESH_TOKEN,
    builtins::STRIPE_LIVE_SECRET_KEY,
    builtins::STRIPE_TEST_SECRET_KEY,
    builtins::STRIPE_LIVE_RESTRICTED_KEY,
    builtins::STRIPE_TEST_RESTRICTED_KEY,
    builtins::STRIPE_WEBHOOK_SIGNING_SECRET,
    builtins::CLOUDFLARE_GLOBAL_API_KEY,
    builtins::CLOUDFLARE_USER_API_TOKEN,
    builtins::CLOUDFLARE_ACCOUNT_API_TOKEN,
    builtins::SLACK_BOT_TOKEN,
    builtins::SLACK_USER_TOKEN,
    builtins::SLACK_APP_LEVEL_TOKEN,
    builtins::SLACK_WORKFLOW_TOKEN,
];

const DETERMINISTIC_PATTERNS: &[RuleSpec] = &[
    builtins::GITHUB_STATELESS_INSTALLATION_TOKEN,
    builtins::TELEGRAM_BOT_TOKEN,
    builtins::SIGNED_JWT,
];

const CONTEXTUAL_PREFIXES: &[RuleSpec] = &[
    builtins::AWS_ACCESS_KEY_ID,
    builtins::AWS_TEMPORARY_ACCESS_KEY_ID,
];

const CONTEXTUAL_PATTERNS: &[RuleSpec] = &[
    builtins::AWS_SECRET_ACCESS_KEY,
    builtins::AWS_SESSION_TOKEN,
    builtins::AZURE_CLIENT_SECRET,
    builtins::AZURE_STORAGE_ACCOUNT_KEY,
    builtins::AZURE_SHARED_ACCESS_SIGNATURE,
    builtins::GCP_PRIVATE_KEY_ID,
    builtins::GCP_CLIENT_SECRET,
    builtins::GCP_PRIVATE_KEY,
    builtins::PASSWORD_FIELD,
    builtins::DATABASE_PASSWORD_FIELD,
    builtins::PASSPHRASE_FIELD,
    builtins::SENSITIVE_HASH,
    builtins::GENERIC_API_KEY,
    builtins::GENERIC_AUTH_TOKEN,
    builtins::GENERIC_SECRET,
];

fn repeat_to_size(block: &str, size: usize) -> String {
    let mut source = String::with_capacity(size + block.len());
    while source.len() < size {
        source.push_str(block);
    }
    source.truncate(size);
    source
}

fn clean_source() -> String {
    repeat_to_size(
        "application_name=cribra-demo\n\
         log_level=info\n\
         feature_flag=true\n\
         database_host=localhost\n\
         cache_ttl=300\n\
         ordinary application configuration without sensitive values\n",
        SIZE,
    )
}

fn scanner(rules: &[RuleSpec]) -> Scanner {
    Scanner::builder()
        .builtins(rules)
        .build()
        .expect("diagnostic scanner should compile")
}

fn joined(left: &[RuleSpec], right: &[RuleSpec]) -> Vec<RuleSpec> {
    left.iter().chain(right).copied().collect()
}

fn bench_family_isolation(criterion: &mut Criterion) {
    let source = clean_source();

    let deterministic_all = joined(DETERMINISTIC_PREFIXES, DETERMINISTIC_PATTERNS);
    let contextual_all = joined(CONTEXTUAL_PREFIXES, CONTEXTUAL_PATTERNS);

    let cases = [
        ("empty", Scanner::builder().build().expect("empty scanner")),
        ("deterministic-prefixes", scanner(DETERMINISTIC_PREFIXES)),
        ("deterministic-patterns", scanner(DETERMINISTIC_PATTERNS)),
        ("deterministic-all", scanner(&deterministic_all)),
        ("contextual-prefixes", scanner(CONTEXTUAL_PREFIXES)),
        ("contextual-patterns", scanner(CONTEXTUAL_PATTERNS)),
        ("contextual-all", scanner(&contextual_all)),
        ("current", scanner(builtins::CURRENT)),
    ];

    for (name, configured) in &cases {
        let report = configured.scan([("setup", source.as_str())]);
        let report = report.single_report().expect("one source");
        assert_eq!(
            report.len(),
            0,
            "{name} diagnostic clean fixture unexpectedly produced findings"
        );
        assert_eq!(
            report.candidate_len(),
            0,
            "{name} diagnostic clean fixture unexpectedly produced candidates"
        );
    }

    let mut group = criterion.benchmark_group("v02/isolation/family");
    group.sample_size(60);
    group.measurement_time(Duration::from_secs(10));
    group.throughput(Throughput::Bytes(source.len() as u64));

    for (name, configured) in &cases {
        group.bench_with_input(
            BenchmarkId::new("scan-64k", name),
            configured,
            |bencher, scanner| {
                bencher.iter(|| black_box(scanner.scan([("bench", black_box(source.as_str()))])));
            },
        );
    }

    group.finish();
}

fn bench_contextual_pattern_scaling(criterion: &mut Criterion) {
    let source = clean_source();
    let mut group = criterion.benchmark_group("v02/isolation/contextual-pattern-count");
    group.sample_size(60);
    group.measurement_time(Duration::from_secs(10));
    group.throughput(Throughput::Bytes(source.len() as u64));

    for count in [1_usize, 2, 4, 8, CONTEXTUAL_PATTERNS.len()] {
        let configured = scanner(&CONTEXTUAL_PATTERNS[..count]);
        let report = configured.scan([("setup", source.as_str())]);
        assert_eq!(report.single_report().expect("one source").len(), 0);

        group.bench_with_input(
            BenchmarkId::new("scan-64k", count),
            &configured,
            |bencher, scanner| {
                bencher.iter(|| black_box(scanner.scan([("bench", black_box(source.as_str()))])));
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_family_isolation,
    bench_contextual_pattern_scaling,
);
criterion_main!(benches);
