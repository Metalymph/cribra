use std::{hint::black_box, time::Duration};

use cribra::{RuleSpec, Scanner, builtins};
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};

const SIZE: usize = 64 * 1024;

const TAIL: &[(&str, RuleSpec)] = &[
    ("password-field", builtins::PASSWORD_FIELD),
    ("database-password-field", builtins::DATABASE_PASSWORD_FIELD),
    ("passphrase-field", builtins::PASSPHRASE_FIELD),
    ("sensitive-hash", builtins::SENSITIVE_HASH),
    ("generic-api-key", builtins::GENERIC_API_KEY),
    ("generic-auth-token", builtins::GENERIC_AUTH_TOKEN),
    ("generic-secret", builtins::GENERIC_SECRET),
];

const FIRST_EIGHT: &[RuleSpec] = &[
    builtins::AWS_SECRET_ACCESS_KEY,
    builtins::AWS_SESSION_TOKEN,
    builtins::AZURE_CLIENT_SECRET,
    builtins::AZURE_STORAGE_ACCOUNT_KEY,
    builtins::AZURE_SHARED_ACCESS_SIGNATURE,
    builtins::GCP_PRIVATE_KEY_ID,
    builtins::GCP_CLIENT_SECRET,
    builtins::GCP_PRIVATE_KEY,
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

fn scanner<I>(rules: I) -> Scanner
where
    I: IntoIterator<Item = RuleSpec>,
{
    Scanner::builder()
        .builtins(rules)
        .build()
        .expect("diagnostic scanner should compile")
}

fn assert_clean(scanner: &Scanner, source: &str, name: &str) {
    let results = scanner.scan([("setup", source)]);
    let report = results.single_report().expect("one source");
    assert_eq!(report.len(), 0, "{name} unexpectedly produced findings");
    assert_eq!(
        report.candidate_len(),
        0,
        "{name} unexpectedly produced candidates"
    );
}

fn bench_tail_individual(criterion: &mut Criterion) {
    let source = clean_source();
    let mut group = criterion.benchmark_group("v02/isolation/contextual-tail-individual");
    group.sample_size(60);
    group.measurement_time(Duration::from_secs(8));
    group.throughput(Throughput::Bytes(source.len() as u64));

    for &(name, rule) in TAIL {
        let configured = scanner([rule]);
        assert_clean(&configured, &source, name);

        group.bench_with_input(
            BenchmarkId::new("scan-64k", name),
            &configured,
            |bencher, scanner| {
                bencher.iter(|| black_box(scanner.scan([("bench", black_box(source.as_str()))])));
            },
        );
    }

    group.finish();
}

fn bench_tail_incremental(criterion: &mut Criterion) {
    let source = clean_source();
    let mut group = criterion.benchmark_group("v02/isolation/contextual-tail-incremental");
    group.sample_size(60);
    group.measurement_time(Duration::from_secs(8));
    group.throughput(Throughput::Bytes(source.len() as u64));

    let mut rules = FIRST_EIGHT.to_vec();

    for &(name, rule) in TAIL {
        rules.push(rule);
        let configured = scanner(rules.iter().copied());
        assert_clean(&configured, &source, name);

        group.bench_with_input(
            BenchmarkId::new("through", name),
            &configured,
            |bencher, scanner| {
                bencher.iter(|| black_box(scanner.scan([("bench", black_box(source.as_str()))])));
            },
        );
    }

    group.finish();
}

criterion_group!(benches, bench_tail_individual, bench_tail_incremental,);
criterion_main!(benches);
