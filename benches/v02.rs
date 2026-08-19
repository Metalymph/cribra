use std::{hint::black_box, time::Duration};

use cribra::{Explanation, Rule, Scanner, Severity, builtins};
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};

const MEDIUM: usize = 64 * 1024;
const LARGE: usize = 1024 * 1024;

const _TOKEN: &str = "ghp_AbCdEf0123456789_AbCdEf0123456789";
const CANDIDATE: &str = "QRST-UVWX-YZ12-3456";

fn build_current_scanner() -> Scanner {
    Scanner::builder()
        .builtins(builtins::CURRENT)
        .build()
        .expect("built-in scanner should compile")
}

fn repeat_to_size(block: &str, size: usize) -> String {
    let mut source = String::with_capacity(size + block.len());

    while source.len() < size {
        source.push_str(block);
    }

    source.truncate(size);
    source
}

fn clean_source(size: usize) -> String {
    repeat_to_size(
        "ordinary application configuration without sensitive values\n",
        size,
    )
}

fn candidate_source(size: usize, spacing: usize) -> String {
    let filler = "ordinary-value ";
    let mut source = String::with_capacity(size + CANDIDATE.len());

    while source.len() < size {
        if source.len() % spacing < filler.len() {
            source.push_str(CANDIDATE);
            source.push('\n');
        } else {
            source.push_str(filler);
        }
    }

    source.truncate(size);
    source
}

fn realistic_source(size: usize) -> String {
    const BLOCK: &str = concat!(
        "application_name=cribra-demo\n",
        "log_level=info\n",
        "feature_flag=true\n",
        "database_host=localhost\n",
        "cache_ttl=300\n",
    );

    let mut source = repeat_to_size(BLOCK, size);
    let snippets = [
        "\nGITHUB_TOKEN=ghp_AbCdEf0123456789_AbCdEf0123456789\n",
        "\nAPI_KEY=AbCdEfGhIjKlMnOpQrStUvWx\n",
        "\nRECOVERY_CODE=QRST-UVWX-YZ12-3456\n",
    ];

    for (index, snippet) in snippets.iter().enumerate() {
        let position = ((index + 1) * source.len() / (snippets.len() + 1)).min(source.len());
        source.insert_str(position, snippet);
    }

    source
}

fn custom_scanner() -> Scanner {
    Scanner::builder()
        .builtins(builtins::CURRENT)
        .rule(Rule::literal(
            "domain.recovery-code",
            CANDIDATE,
            Severity::Critical,
        ))
        .build()
        .expect("scanner should compile")
}

fn bench_v02_candidate_path(criterion: &mut Criterion) {
    let scanner = build_current_scanner();
    let mut group = criterion.benchmark_group("v02/candidate-path");
    group.sample_size(60);
    group.measurement_time(Duration::from_secs(10));

    for (name, source) in [
        ("none-64k", clean_source(MEDIUM)),
        ("sparse-64k", candidate_source(MEDIUM, 16 * 1024)),
        ("dense-64k", candidate_source(MEDIUM, 256)),
        ("sparse-1m", candidate_source(LARGE, 128 * 1024)),
    ] {
        let results = scanner.scan([("setup", source.as_str())]);
        let report = results.single_report().expect("one source");

        if name.starts_with("none") {
            assert_eq!(report.candidate_len(), 0);
        } else {
            assert!(report.candidate_len() > 0);
        }

        group.throughput(Throughput::Bytes(source.len() as u64));
        group.bench_with_input(
            BenchmarkId::new("scan", name),
            &source,
            |bencher, source| {
                bencher.iter(|| black_box(scanner.scan([("bench", black_box(source.as_str()))])));
            },
        );
    }

    group.finish();
}

fn bench_v02_realistic_mix(criterion: &mut Criterion) {
    let scanner = build_current_scanner();
    let source = realistic_source(MEDIUM);
    let setup = scanner.scan([("setup", source.as_str())]);
    let report = setup.single_report().expect("one source");

    assert!(report.len() >= 2);
    assert_eq!(report.candidate_len(), 1);

    let mut group = criterion.benchmark_group("v02/realistic-mix");
    group.throughput(Throughput::Bytes(source.len() as u64));

    group.bench_function("64k", |bencher| {
        bencher.iter(|| black_box(scanner.scan([("bench", black_box(source.as_str()))])));
    });

    group.finish();
}

fn bench_v02_explainability_projection(criterion: &mut Criterion) {
    let scanner = build_current_scanner();
    let source = realistic_source(MEDIUM);
    let results = scanner.scan([("setup", source.as_str())]);
    let report = results.single_report().expect("one source");

    assert!(!report.findings().is_empty());
    assert_eq!(report.candidate_len(), 1);

    let mut group = criterion.benchmark_group("v02/explainability");
    group.sample_size(100);

    group.bench_function("classified-all", |bencher| {
        bencher.iter(|| {
            for finding in black_box(report.findings()) {
                let explanation = finding
                    .explanation(black_box(&scanner))
                    .expect("producing scanner should explain finding");
                black_box(explanation);
            }
        });
    });

    group.bench_function("ambiguous-all", |bencher| {
        bencher.iter(|| {
            for candidate in black_box(report.candidates()) {
                let explanation: Explanation = candidate.explanation();
                black_box(explanation);
            }
        });
    });

    group.finish();
}

fn bench_v02_ambiguity_promotion(criterion: &mut Criterion) {
    let generic = build_current_scanner();
    let promoted = custom_scanner();

    let source = repeat_to_size("mode=production\nrecovery=QRST-UVWX-YZ12-3456\n", MEDIUM);

    let generic_report = generic.scan([("setup", source.as_str())]);
    assert!(
        generic_report
            .single_report()
            .expect("one source")
            .candidate_len()
            > 0
    );

    let promoted_report = promoted.scan([("setup", source.as_str())]);
    let promoted_report = promoted_report.single_report().expect("one source");
    assert!(!promoted_report.is_empty());
    assert_eq!(promoted_report.candidate_len(), 0);

    let mut group = criterion.benchmark_group("v02/ambiguity-promotion");
    group.throughput(Throughput::Bytes(source.len() as u64));

    group.bench_function("generic-candidate", |bencher| {
        bencher.iter(|| black_box(generic.scan([("bench", black_box(source.as_str()))])));
    });

    group.bench_function("custom-finding", |bencher| {
        bencher.iter(|| black_box(promoted.scan([("bench", black_box(source.as_str()))])));
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_v02_candidate_path,
    bench_v02_realistic_mix,
    bench_v02_explainability_projection,
    bench_v02_ambiguity_promotion,
);
criterion_main!(benches);
