//! Public-API timings. Both primary paths copy/stage the replacement document
//! and retain the old report until commit. Compilation and setup are untimed.
use std::{hint::black_box, ops::Range, time::Duration};

use cribra::{Rule, ScanReport, Scanner, Severity, builtins};
use criterion::{BatchSize, BenchmarkId, Criterion, criterion_group, criterion_main};

struct Fixture {
    name: &'static str,
    scanner: Scanner,
    source: String,
    range: Range<usize>,
    replacement: String,
}

fn repeated(block: &str, size: usize) -> String {
    let mut source = block.repeat(size.div_ceil(block.len()));
    let mut end = size;
    while !source.is_char_boundary(end) {
        end -= 1;
    }
    source.truncate(end);
    source
}

fn fixtures() -> Vec<Fixture> {
    let literal = |count: usize, dense: bool| {
        Scanner::builder()
            .rules((0..count).map(|i| {
                Rule::literal(
                    format!("literal-{i}"),
                    if dense {
                        "aa".into()
                    } else {
                        format!("SECRET_{i:04}")
                    },
                    Severity::High,
                )
            }))
            .build()
            .unwrap()
    };
    let core = || {
        Scanner::builder()
            .builtins(builtins::CURRENT)
            .build()
            .unwrap()
    };
    let mut fixtures = Vec::new();
    for (name, scanner, source) in [
        (
            "sparse-4-1k",
            literal(4, false),
            repeated("ordinary value\n", 1024),
        ),
        (
            "sparse-64-64k",
            literal(64, false),
            repeated("ordinary value\n", 65536),
        ),
        (
            "sparse-512-1m",
            literal(512, false),
            repeated("ordinary value\n", 1024 * 1024),
        ),
        ("dense-8-64k", literal(8, true), "a".repeat(65536)),
        (
            "structural-64k",
            Scanner::builder().build().unwrap(),
            repeated("QRST-UVWX-YZ12-3456 \n", 65536),
        ),
        (
            "mixed-64k",
            Scanner::builder()
                .rules([
                    Rule::literal("literal", "ordinary", Severity::Low),
                    Rule::prefix("prefix", "pre_", Severity::High),
                    Rule::suffix("suffix", "_tail", Severity::High),
                    Rule::pattern("regex", "[a-z]{3,12}_[0-9]{2}", Severity::High).unwrap(),
                ])
                .build()
                .unwrap(),
            repeated("ordinary pre_value stuff_tail test_12\n", 65536),
        ),
        (
            "core-64k",
            core(),
            repeated(
                "ordinary value\npassword=CorrectHorseBatteryStaple\n",
                65536,
            ),
        ),
        (
            "personal-64k",
            Scanner::builder()
                .builtins(builtins::personal::CURRENT)
                .build()
                .unwrap(),
            repeated("ordinary value\n44051401458\n", 65536),
        ),
        (
            "financial-64k",
            Scanner::builder()
                .builtins(builtins::financial::CURRENT)
                .build()
                .unwrap(),
            repeated("ordinary value\n4111111111111111\n", 65536),
        ),
        (
            "combined-64k",
            Scanner::builder()
                .builtins(builtins::CURRENT)
                .builtins(builtins::personal::CURRENT)
                .builtins(builtins::financial::CURRENT)
                .build()
                .unwrap(),
            repeated(
                "password=CorrectHorseBatteryStaple\n44051401458\n4111111111111111\n",
                65536,
            ),
        ),
        (
            "contextual-64k",
            Scanner::builder()
                .builtins([builtins::PASSWORD_FIELD, builtins::MAVEN_SERVER_PASSWORD])
                .build()
                .unwrap(),
            repeated(
                "password=CorrectHorseBatteryStaple\n<settings><servers><server><password>CorrectHorseBatteryStaple</password></server></servers></settings>\n",
                65536,
            ),
        ),
        (
            "whole-replacement-64k",
            literal(64, false),
            repeated("ordinary value\n", 65536),
        ),
    ] {
        let middle = source.len() / 2;
        let (range, replacement) = if name == "whole-replacement-64k" {
            (0..source.len(), repeated("changed value\n", 65536))
        } else {
            (middle..middle + 1, "x".into())
        };
        fixtures.push(Fixture {
            name,
            scanner,
            source,
            range,
            replacement,
        });
    }
    // Favorable cases also contain sparse results, not only empty reports.
    for fixture in &mut fixtures {
        if fixture.name.starts_with("sparse-") {
            fixture.source.push_str("\nSECRET_0000\n");
        }
    }
    fixtures
}

fn replacement(source: &str, range: Range<usize>, replacement: &str) -> String {
    let mut next = String::with_capacity(source.len() - range.len() + replacement.len());
    next.push_str(&source[..range.start]);
    next.push_str(replacement);
    next.push_str(&source[range.end..]);
    next
}

fn report(scanner: &Scanner, source: &str) -> ScanReport {
    scanner
        .scan([((), source)])
        .into_inner()
        .pop()
        .unwrap()
        .into_parts()
        .2
}

fn qualify(fixture: &Fixture) {
    let mut session = fixture
        .scanner
        .incremental_session(fixture.source.clone())
        .unwrap();
    assert_eq!(session.report(), &report(&fixture.scanner, &fixture.source));
    session
        .apply_edit(0, fixture.range.clone(), &fixture.replacement)
        .unwrap();
    let next = replacement(&fixture.source, fixture.range.clone(), &fixture.replacement);
    assert_eq!(session.report(), &report(&fixture.scanner, &next));
}

fn updates(c: &mut Criterion) {
    let mut group = c.benchmark_group("d4/update");
    group.sample_size(50);
    group.measurement_time(Duration::from_secs(8));
    for fixture in fixtures() {
        qualify(&fixture);
        group.bench_with_input(
            BenchmarkId::new("incremental", fixture.name),
            &fixture,
            |b, fixture| {
                b.iter_batched_ref(
                    || {
                        fixture
                            .scanner
                            .incremental_session(fixture.source.clone())
                            .unwrap()
                    },
                    |session| {
                        session
                            .apply_edit(0, fixture.range.clone(), black_box(&fixture.replacement))
                            .unwrap();
                        black_box(session.report());
                    },
                    BatchSize::PerIteration,
                );
            },
        );
        group.bench_with_input(
            BenchmarkId::new("edit-and-scan", fixture.name),
            &fixture,
            |b, fixture| {
                b.iter_batched_ref(
                    || {
                        (
                            fixture.source.clone(),
                            report(&fixture.scanner, &fixture.source),
                        )
                    },
                    |state| {
                        let next = replacement(
                            &state.0,
                            fixture.range.clone(),
                            black_box(&fixture.replacement),
                        );
                        let next_report = report(&fixture.scanner, &next);
                        *state = (next, next_report); // old source/report dropped inside timing, like incremental commit
                        black_box(&state.1);
                    },
                    BatchSize::PerIteration,
                );
            },
        );
    }
    group.finish();
}

fn construction_and_scan_reference(c: &mut Criterion) {
    let mut group = c.benchmark_group("d4/reference");
    for fixture in fixtures().into_iter().filter(|fixture| {
        matches!(
            fixture.name,
            "sparse-64-64k" | "dense-8-64k" | "combined-64k"
        )
    }) {
        qualify(&fixture);
        group.bench_with_input(
            BenchmarkId::new("construction", fixture.name),
            &fixture,
            |b, f| {
                b.iter_batched(
                    || f.source.clone(),
                    |source| black_box(f.scanner.incremental_session(source).unwrap()),
                    BatchSize::PerIteration,
                );
            },
        );
        let next = replacement(&fixture.source, fixture.range.clone(), &fixture.replacement);
        group.bench_with_input(
            BenchmarkId::new("scan-only", fixture.name),
            &next,
            |b, source| {
                b.iter(|| black_box(report(&fixture.scanner, black_box(source))));
            },
        );
    }
    group.finish();
}

fn sequence(c: &mut Criterion) {
    let fixture = fixtures().remove(1);
    // Typing, backspace, UTF-8 paste, replacement, no-op and a larger paste.
    let edits: Vec<_> = (0..32)
        .map(|i| match i % 6 {
            0 => (0..0, "x".into()),
            1 => (0..1, "".into()),
            2 => (0..0, "🦀\r\n".into()),
            3 => (0..4, "é".into()),
            4 => (0..0, "".into()),
            _ => (0..0, "ordinary value\n".repeat(274)),
        })
        .collect();
    let mut session = fixture
        .scanner
        .incremental_session(fixture.source.clone())
        .unwrap();
    let mut source = fixture.source.clone();
    for (range, text) in &edits {
        source = replacement(&source, range.clone(), text);
        session
            .apply_edit(session.revision(), range.clone(), text)
            .unwrap();
        assert_eq!(session.report(), &report(&fixture.scanner, &source));
    }
    let mut group = c.benchmark_group("d4/sequence");
    group.bench_function("incremental-32-edits", |b| {
        b.iter_batched_ref(
            || {
                fixture
                    .scanner
                    .incremental_session(fixture.source.clone())
                    .unwrap()
            },
            |session| {
                for (range, text) in &edits {
                    session
                        .apply_edit(session.revision(), range.clone(), text)
                        .unwrap();
                }
                black_box(session.report());
            },
            BatchSize::PerIteration,
        );
    });
    group.bench_function("edit-and-scan-32-edits", |b| {
        b.iter_batched_ref(
            || {
                (
                    fixture.source.clone(),
                    report(&fixture.scanner, &fixture.source),
                )
            },
            |state| {
                for (range, text) in &edits {
                    let next = replacement(&state.0, range.clone(), text);
                    let next_report = report(&fixture.scanner, &next);
                    *state = (next, next_report);
                }
                black_box(&state.1);
            },
            BatchSize::PerIteration,
        );
    });
    group.finish();
}

criterion_group!(benches, updates, construction_and_scan_reference, sequence);
criterion_main!(benches);
