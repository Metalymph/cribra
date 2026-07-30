use std::{hint::black_box, time::Duration};

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use silens_scan::{Rule, Scanner, Severity};

const SMALL: usize = 1_024;
const MEDIUM: usize = 64 * 1_024;
const LARGE: usize = 1_024 * 1_024;

fn build_rules(count: usize, family: &str) -> Scanner {
    let mut builder = Scanner::builder();

    for index in 0..count {
        let rule = match family {
            "literal" => Rule::literal(
                format!("literal-{index}"),
                format!("silens_literal_{index:04}_"),
                Severity::High,
            ),
            "prefix" => Rule::prefix(
                format!("prefix-{index}"),
                format!("silens_prefix_{index:04}_"),
                Severity::High,
            ),
            "suffix" => Rule::suffix(
                format!("suffix-{index}"),
                format!("_silens_suffix_{index:04}"),
                Severity::High,
            ),
            "pattern" => Rule::pattern(
                format!("pattern-{index}"),
                format!(r"\bsilens_pattern_{index:04}_[A-Za-z0-9]{{16}}\b"),
                Severity::High,
            )
            .expect("benchmark regex must be valid"),
            "mixed" => match index % 4 {
                0 => Rule::literal(
                    format!("mixed-literal-{index}"),
                    format!("silens_literal_{index:04}_"),
                    Severity::High,
                ),
                1 => Rule::prefix(
                    format!("mixed-prefix-{index}"),
                    format!("silens_prefix_{index:04}_"),
                    Severity::High,
                ),
                2 => Rule::suffix(
                    format!("mixed-suffix-{index}"),
                    format!("_silens_suffix_{index:04}"),
                    Severity::High,
                ),
                _ => Rule::pattern(
                    format!("mixed-pattern-{index}"),
                    format!(r"\bsilens_pattern_{index:04}_[A-Za-z0-9]{{16}}\b"),
                    Severity::High,
                )
                .expect("benchmark regex must be valid"),
            },
            _ => unreachable!("known benchmark family"),
        };

        builder = builder.rule(rule);
    }

    builder.build().expect("benchmark rules must compile")
}

fn no_match_input(size: usize) -> String {
    const CHUNK: &str = "ordinary application text without credentials or sensitive tokens\n";
    let mut input = String::with_capacity(size);
    while input.len() < size {
        input.push_str(CHUNK);
    }
    input.truncate(size);
    input
}

fn sparse_match_input(size: usize) -> String {
    let mut input = no_match_input(size);
    let markers = [
        " silens_literal_0000_ ",
        " silens_prefix_0001_VALUE1234567890 ",
        " value_silens_suffix_0002 ",
        " silens_pattern_0003_ABCDEF1234567890 ",
    ];

    for (index, marker) in markers.iter().enumerate().rev() {
        let position = ((index + 1) * size / (markers.len() + 1)).min(input.len());
        input.insert_str(position, marker);
    }

    input
}

fn dense_match_input(size: usize) -> String {
    const BLOCK: &str = concat!(
        "silens_literal_0000_ ",
        "silens_prefix_0001_VALUE1234567890 ",
        "value_silens_suffix_0002 ",
        "silens_pattern_0003_ABCDEF1234567890\n",
    );
    let mut input = String::with_capacity(size);
    while input.len() < size {
        input.push_str(BLOCK);
    }
    input.truncate(size);
    input
}

fn bench_input_sizes(c: &mut Criterion) {
    let scanner = build_rules(64, "mixed");
    let mut group = c.benchmark_group("scan/input-size");

    for size in [SMALL, MEDIUM, LARGE] {
        let input = sparse_match_input(size);
        assert_eq!(
            scanner.scan(&input).len(),
            4,
            "sparse mixed fixture must produce four findings",
        );
        group.throughput(Throughput::Bytes(input.len() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &input, |b, input| {
            b.iter(|| scanner.scan(black_box(input)));
        });
    }

    group.finish();
}

fn bench_rule_counts(c: &mut Criterion) {
    let input = sparse_match_input(MEDIUM);
    let mut group = c.benchmark_group("scan/rule-count");
    group.throughput(Throughput::Bytes(input.len() as u64));
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(60);

    for count in [4, 64, 512] {
        let scanner = build_rules(count, "mixed");
        assert_eq!(
            scanner.scan(&input).len(),
            4,
            "sparse mixed fixture must produce four findings",
        );
        group.bench_with_input(
            BenchmarkId::from_parameter(count),
            &scanner,
            |b, scanner| {
                b.iter(|| scanner.scan(black_box(&input)));
            },
        );
    }

    group.finish();
}

fn bench_match_density(c: &mut Criterion) {
    let scanner = build_rules(64, "mixed");
    let cases = [
        ("none", no_match_input(MEDIUM)),
        ("sparse", sparse_match_input(MEDIUM)),
        ("dense", dense_match_input(MEDIUM)),
    ];
    assert!(
        scanner.scan(&cases[0].1).is_empty(),
        "no-match fixture must produce no findings",
    );
    assert_eq!(
        scanner.scan(&cases[1].1).len(),
        4,
        "sparse fixture must produce four findings",
    );
    assert!(
        !scanner.scan(&cases[2].1).is_empty(),
        "dense fixture must produce findings",
    );

    let mut group = c.benchmark_group("scan/match-density");

    for (name, input) in cases {
        group.throughput(Throughput::Bytes(input.len() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(name), &input, |b, input| {
            b.iter(|| scanner.scan(black_box(input)));
        });
    }

    group.finish();
}

fn bench_matcher_families(c: &mut Criterion) {
    let input = sparse_match_input(MEDIUM);
    let mut group = c.benchmark_group("scan/matcher-family");
    group.throughput(Throughput::Bytes(input.len() as u64));

    for family in ["literal", "prefix", "suffix", "pattern", "mixed"] {
        let scanner = build_rules(64, family);
        let expected = if family == "mixed" { 4 } else { 1 };
        assert_eq!(
            scanner.scan(&input).len(),
            expected,
            "{family} fixture produced an unexpected finding count",
        );

        group.bench_with_input(
            BenchmarkId::from_parameter(family),
            &scanner,
            |b, scanner| {
                b.iter(|| scanner.scan(black_box(&input)));
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_input_sizes,
    bench_rule_counts,
    bench_match_density,
    bench_matcher_families,
);
criterion_main!(benches);
