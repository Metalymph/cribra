use std::{hint::black_box, time::Duration};

use criterion::{BatchSize, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use silens_scan::{Rule, Scanner, Severity, builtins};

const SMALL: usize = 1_024;
const MEDIUM: usize = 64 * 1_024;
const LARGE: usize = 1_024 * 1_024;

const BENCH_SOURCE_ID: &str = "benchmark";

fn scan_one(scanner: &Scanner, source: &str) -> silens_scan::ScanResults<&'static str> {
    scanner.scan([(BENCH_SOURCE_ID, source)])
}

fn finding_count(scanner: &Scanner, source: &str) -> usize {
    scan_one(scanner, source)
        .single_report()
        .expect("one benchmark source must produce one report")
        .len()
}

fn build_current_scanner() -> Scanner {
    Scanner::builder()
        .builtins(builtins::CURRENT.iter().copied())
        .build()
        .expect("built-in rules must compile")
}

fn build_custom_scanner(rule_count: usize) -> Scanner {
    let mut builder = Scanner::builder();

    for index in 0..rule_count {
        builder = builder.rule(Rule::literal(
            format!("custom.literal.{index}"),
            format!("silens_custom_literal_{index:04}"),
            Severity::High,
        ));
    }

    builder.build().expect("custom rules must compile")
}

fn build_mixed_scanner(rule_count: usize) -> Scanner {
    let mut builder = Scanner::builder();

    for index in 0..rule_count {
        let rule = match index % 4 {
            0 => Rule::literal(
                format!("mixed.literal.{index}"),
                format!("silens_literal_{index:04}_"),
                Severity::High,
            ),
            1 => Rule::prefix(
                format!("mixed.prefix.{index}"),
                format!("silens_prefix_{index:04}_"),
                Severity::High,
            ),
            2 => Rule::suffix(
                format!("mixed.suffix.{index}"),
                format!("_silens_suffix_{index:04}"),
                Severity::High,
            ),
            _ => Rule::pattern(
                format!("mixed.pattern.{index}"),
                format!(r"\bsilens_pattern_{index:04}_[A-Za-z0-9]{{16}}\b"),
                Severity::High,
            )
            .expect("benchmark pattern must compile"),
        };

        builder = builder.rule(rule);
    }

    builder.build().expect("mixed rules must compile")
}

fn repeat_to_size(block: &str, size: usize) -> String {
    let mut source = String::with_capacity(size + block.len());

    while source.len() < size {
        source.push_str(block);
    }

    source.truncate(size);
    source
}

fn no_match_source(size: usize) -> String {
    repeat_to_size(
        "ordinary application configuration without sensitive values\n",
        size,
    )
}

fn realistic_sparse_source(size: usize) -> String {
    const BLOCK: &str = concat!(
        "application_name=silens-demo\n",
        "log_level=info\n",
        "feature_flag=true\n",
        "database_host=localhost\n",
        "cache_ttl=300\n",
    );

    let mut source = repeat_to_size(BLOCK, size);

    let findings = [
        "\nGITHUB_TOKEN=ghp_AbCdEf0123456789_AbCdEf0123456789\n",
        "\nSTRIPE_SECRET_KEY=sk_live_AbCdEf0123456789_AbCdEf0123456789\n",
        "\nAWS_SECRET_ACCESS_KEY=wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY\n",
        "\nAZURE_CLIENT_SECRET=AbCdEfGhIjKlMnOpQrStUvWxYz0123456789\n",
        "\nPOSTGRES_PASSWORD=CorrectHorseBatteryStaple!\n",
        "\nJWT=eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c\n",
    ];

    for (index, finding) in findings.iter().enumerate() {
        let position = ((index + 1) * source.len() / (findings.len() + 1)).min(source.len());
        source.insert_str(position, finding);
    }

    source
}

fn realistic_dense_source(size: usize) -> String {
    const BLOCK: &str = concat!(
        "GITHUB_TOKEN=ghp_AbCdEf0123456789_AbCdEf0123456789\n",
        "STRIPE_SECRET_KEY=sk_live_AbCdEf0123456789_AbCdEf0123456789\n",
        "AWS_SECRET_ACCESS_KEY=wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY\n",
        "POSTGRES_PASSWORD=CorrectHorseBatteryStaple!\n",
    );

    repeat_to_size(BLOCK, size)
}

fn custom_sparse_source(size: usize) -> String {
    let mut source = no_match_source(size);

    for index in [0_usize, 4, 16, 63] {
        let marker = format!(" silens_custom_literal_{index:04} ");
        let position = ((index + 1) * source.len() / 80).min(source.len());
        source.insert_str(position, &marker);
    }

    source
}

fn mixed_sparse_source(size: usize) -> String {
    let mut source = no_match_source(size);

    let markers = [
        " silens_literal_0000_ ",
        " silens_prefix_0001_VALUE1234567890 ",
        " value_silens_suffix_0002 ",
        " silens_pattern_0003_ABCDEF1234567890 ",
    ];

    for (index, marker) in markers.iter().enumerate() {
        let position = ((index + 1) * source.len() / (markers.len() + 1)).min(source.len());
        source.insert_str(position, marker);
    }

    source
}

fn bench_build_cost(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("build");

    group.sample_size(50);
    group.measurement_time(Duration::from_secs(8));

    group.bench_function("builtins-current", |bencher| {
        bencher.iter_batched(
            || builtins::CURRENT.iter().copied(),
            |rules| {
                black_box(
                    Scanner::builder()
                        .builtins(rules)
                        .build()
                        .expect("built-ins must compile"),
                );
            },
            BatchSize::SmallInput,
        );
    });

    for count in [4_usize, 64, 512] {
        group.bench_with_input(
            BenchmarkId::new("custom-literals", count),
            &count,
            |bencher, &count| {
                bencher.iter(|| black_box(build_custom_scanner(count)));
            },
        );
    }

    group.finish();
}

fn bench_current_input_sizes(criterion: &mut Criterion) {
    let scanner = build_current_scanner();
    let mut group = criterion.benchmark_group("scan/current/input-size");

    for size in [SMALL, MEDIUM, LARGE] {
        let source = realistic_sparse_source(size);

        assert!(
            finding_count(&scanner, &source) >= 6,
            "realistic sparse fixture must produce expected findings",
        );

        group.throughput(Throughput::Bytes(source.len() as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            &source,
            |bencher, source| {
                bencher.iter(|| black_box(scan_one(&scanner, black_box(source.as_str()))));
            },
        );
    }

    group.finish();
}

fn bench_current_match_density(criterion: &mut Criterion) {
    let scanner = build_current_scanner();
    let cases = [
        ("none", no_match_source(MEDIUM)),
        ("sparse", realistic_sparse_source(MEDIUM)),
        ("dense", realistic_dense_source(MEDIUM)),
    ];

    let counts = cases
        .iter()
        .map(|(name, source)| (*name, source.len(), finding_count(&scanner, source)))
        .collect::<Vec<_>>();

    assert_eq!(counts[0].2, 0);
    assert!(counts[1].2 >= 6);
    assert!(counts[2].2 > 0);

    eprintln!("match-density setup:");
    for (name, bytes, findings) in &counts {
        eprintln!(
            "  {name}: bytes={bytes}, findings={findings}, bytes_per_finding={}",
            if *findings == 0 { 0 } else { bytes / findings },
        );
    }

    let mut group = criterion.benchmark_group("scan/current/match-density");

    for (name, source) in cases {
        group.throughput(Throughput::Bytes(source.len() as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(name),
            &source,
            |bencher, source| {
                bencher.iter(|| black_box(scan_one(&scanner, black_box(source.as_str()))));
            },
        );
    }

    group.finish();
}

fn bench_pipeline_comparison(criterion: &mut Criterion) {
    let custom = build_custom_scanner(64);
    let mixed = build_mixed_scanner(64);
    let current = build_current_scanner();

    let custom_source = custom_sparse_source(MEDIUM);
    let mixed_source = mixed_sparse_source(MEDIUM);
    let current_source = realistic_sparse_source(MEDIUM);

    assert_eq!(finding_count(&custom, &custom_source), 4);
    assert_eq!(finding_count(&mixed, &mixed_source), 4);
    assert!(finding_count(&current, &current_source) >= 6);

    let mut group = criterion.benchmark_group("scan/pipeline-comparison");
    group.throughput(Throughput::Bytes(MEDIUM as u64));

    group.bench_function("custom-literal-only-64", |bencher| {
        bencher.iter(|| black_box(scan_one(&custom, black_box(custom_source.as_str()))));
    });

    group.bench_function("custom-mixed-64", |bencher| {
        bencher.iter(|| black_box(scan_one(&mixed, black_box(mixed_source.as_str()))));
    });

    group.bench_function("builtins-current", |bencher| {
        bencher.iter(|| black_box(scan_one(&current, black_box(current_source.as_str()))));
    });

    group.finish();
}

fn bench_custom_rule_scaling(criterion: &mut Criterion) {
    let source = custom_sparse_source(MEDIUM);
    let mut group = criterion.benchmark_group("scan/custom/rule-count");

    group.sample_size(60);
    group.measurement_time(Duration::from_secs(10));
    group.throughput(Throughput::Bytes(source.len() as u64));

    for count in [4_usize, 64, 512] {
        let scanner = build_custom_scanner(count);

        group.bench_with_input(
            BenchmarkId::from_parameter(count),
            &scanner,
            |bencher, scanner| {
                bencher.iter(|| black_box(scan_one(scanner, black_box(source.as_str()))));
            },
        );
    }

    group.finish();
}

#[cfg(feature = "parallel")]
fn batch_sources(count: usize, size: usize) -> Vec<(usize, String)> {
    (0..count)
        .map(|index| {
            let mut source = realistic_sparse_source(size);
            source.push_str(&format!("\n# batch-source={index}\n"));
            (index, source)
        })
        .collect()
}

fn bench_batch_parallelism(criterion: &mut Criterion) {
    #[cfg(feature = "parallel")]
    {
        let scanner = build_current_scanner();
        let mut group = criterion.benchmark_group("scan/batch");

        for size in [4 * 1024_usize, MEDIUM, LARGE] {
            let sources = batch_sources(32, size);

            group.throughput(Throughput::Bytes(
                sources.iter().map(|(_, source)| source.len() as u64).sum(),
            ));

            group.bench_with_input(
                BenchmarkId::new("serial", size),
                &sources,
                |bencher, sources| {
                    bencher.iter(|| {
                        black_box(
                            scanner
                                .scan(sources.iter().map(|(key, source)| (*key, source.as_str()))),
                        )
                    });
                },
            );

            group.bench_with_input(
                BenchmarkId::new("parallel", size),
                &sources,
                |bencher, sources| {
                    bencher.iter(|| {
                        black_box(scanner.parallel_scan(
                            sources.iter().map(|(key, source)| (*key, source.as_str())),
                        ))
                    });
                },
            );
        }

        group.finish();
    }

    #[cfg(not(feature = "parallel"))]
    let _ = criterion;
}

criterion_group!(
    benches,
    bench_build_cost,
    bench_current_input_sizes,
    bench_current_match_density,
    bench_pipeline_comparison,
    bench_custom_rule_scaling,
    bench_batch_parallelism,
);
criterion_main!(benches);
