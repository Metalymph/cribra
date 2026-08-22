use std::{hint::black_box, ptr, time::Duration};

use cribra::{Scanner, redact};
use cribra_capi::{
    CRIBRA_BATCH_EXECUTION_SERIAL, CRIBRA_OK, CribraBatchInput, CribraBatchResults, CribraError,
    CribraFindingView, CribraOutput, CribraReport, CribraScanner, CribraStringView,
    cribra_abi_version_major, cribra_batch_results_count, cribra_batch_results_free,
    cribra_error_free, cribra_output_free, cribra_report_finding_at, cribra_report_finding_count,
    cribra_report_free, cribra_scanner_free, cribra_scanner_new_current, cribra_scanner_scan,
    cribra_scanner_scan_batch, cribra_transform_redact,
};
use criterion::{Criterion, Throughput, criterion_group, criterion_main};

type RustCall = fn() -> u32;
type CAbiCall = extern "C" fn() -> u32;

#[inline(never)]
fn rust_version_baseline() -> u32 {
    0
}

fn bench_minimal_call_overhead(criterion: &mut Criterion) {
    let rust_call: RustCall = rust_version_baseline;
    let c_abi_call: CAbiCall = cribra_abi_version_major;

    assert_eq!(rust_call(), 0);
    assert_eq!(c_abi_call(), 0);

    let mut group = criterion.benchmark_group("capi/minimal-call");
    group.sample_size(200);
    group.measurement_time(Duration::from_secs(8));

    group.bench_function("rust-fn-pointer", |bencher| {
        bencher.iter(|| {
            let call = black_box(rust_call);
            black_box(call())
        });
    });

    group.bench_function("extern-c-fn-pointer", |bencher| {
        bencher.iter(|| {
            let call = black_box(c_abi_call);
            black_box(call())
        });
    });

    group.finish();
}

fn clean_source(bytes: usize) -> String {
    const LINE: &str = "service=cribra mode=production region=eu-west-1 enabled=true\n";

    let mut source = String::with_capacity(bytes);
    while source.len() < bytes {
        source.push_str(LINE);
    }
    source.truncate(bytes);

    // Keep UTF-8 valid if a future line fixture becomes non-ASCII.
    while !source.is_char_boundary(source.len()) {
        source.pop();
    }

    source
}

fn benchmark_single_scan_pair(
    criterion: &mut Criterion,
    name: &str,
    source: &str,
    sample_size: usize,
    measurement_secs: u64,
) {
    let rust_scanner = Scanner::default();

    let mut capi_scanner: *mut CribraScanner = ptr::null_mut();
    let status = unsafe { cribra_scanner_new_current(&mut capi_scanner) };
    assert_eq!(status, CRIBRA_OK);
    assert!(!capi_scanner.is_null());

    // Sanity-check semantic equivalence before timing.
    let rust_results = rust_scanner.scan([((), source)]);
    let rust_report = rust_results
        .single_report()
        .expect("one native source must produce one report");

    let mut capi_report: *mut CribraReport = ptr::null_mut();
    let mut capi_error: *mut CribraError = ptr::null_mut();
    let status = unsafe {
        cribra_scanner_scan(
            capi_scanner,
            source.as_ptr(),
            source.len(),
            &mut capi_report,
            &mut capi_error,
        )
    };
    assert_eq!(status, CRIBRA_OK);
    assert!(capi_error.is_null());
    assert!(!capi_report.is_null());

    let mut capi_count = 0usize;
    let status = unsafe { cribra_capi::cribra_report_finding_count(capi_report, &mut capi_count) };
    assert_eq!(status, CRIBRA_OK);
    assert_eq!(capi_count, rust_report.len());

    unsafe { cribra_report_free(capi_report) };

    let mut group = criterion.benchmark_group(format!("capi/single-scan/{name}"));
    group.throughput(Throughput::Bytes(source.len() as u64));
    group.sample_size(sample_size);
    group.measurement_time(Duration::from_secs(measurement_secs));

    group.bench_function("rust-native", |bencher| {
        bencher.iter(|| {
            let results = rust_scanner.scan([((), black_box(source))]);
            black_box(results)
        });
    });

    group.bench_function("c-abi", |bencher| {
        bencher.iter(|| {
            let mut report: *mut CribraReport = ptr::null_mut();
            let mut error: *mut CribraError = ptr::null_mut();

            let status = unsafe {
                cribra_scanner_scan(
                    black_box(capi_scanner),
                    black_box(source.as_ptr()),
                    black_box(source.len()),
                    &mut report,
                    &mut error,
                )
            };

            assert_eq!(status, CRIBRA_OK);
            assert!(error.is_null());
            assert!(!report.is_null());

            black_box(report);

            unsafe {
                cribra_report_free(report);
                cribra_error_free(error);
            }
        });
    });

    group.finish();

    unsafe { cribra_scanner_free(capi_scanner) };
}

fn bench_single_scan_overhead(criterion: &mut Criterion) {
    let tiny = clean_source(64);
    let medium = clean_source(64 * 1024);
    let large = clean_source(1024 * 1024);

    benchmark_single_scan_pair(criterion, "64B-clean", &tiny, 200, 8);
    benchmark_single_scan_pair(criterion, "64KiB-clean", &medium, 100, 10);
    benchmark_single_scan_pair(criterion, "1MiB-clean", &large, 30, 12);
}

fn dense_finding_source(repetitions: usize) -> String {
    const LINE: &str = "GITHUB_TOKEN=ghp_AbCdEf0123456789_AbCdEf0123456789\n";

    let mut source = String::with_capacity(LINE.len() * repetitions);
    for _ in 0..repetitions {
        source.push_str(LINE);
    }
    source
}

fn bench_report_traversal(criterion: &mut Criterion) {
    let source = dense_finding_source(256);

    let rust_scanner = Scanner::default();
    let rust_results = rust_scanner.scan([((), source.as_str())]);
    let rust_report = rust_results
        .single_report()
        .expect("one source must produce one native report");
    let rust_count = rust_report.len();

    assert!(
        rust_count >= 128,
        "dense traversal fixture must produce many findings"
    );

    let mut capi_scanner: *mut CribraScanner = ptr::null_mut();
    let mut capi_report: *mut CribraReport = ptr::null_mut();
    let mut capi_error: *mut CribraError = ptr::null_mut();

    unsafe {
        assert_eq!(cribra_scanner_new_current(&mut capi_scanner), CRIBRA_OK);
        assert_eq!(
            cribra_scanner_scan(
                capi_scanner,
                source.as_ptr(),
                source.len(),
                &mut capi_report,
                &mut capi_error,
            ),
            CRIBRA_OK
        );
    }

    assert!(capi_error.is_null());
    assert!(!capi_report.is_null());

    let mut capi_count = 0usize;
    unsafe {
        assert_eq!(
            cribra_report_finding_count(capi_report, &mut capi_count),
            CRIBRA_OK
        );
    }
    assert_eq!(capi_count, rust_count);

    let mut group = criterion.benchmark_group("capi/report-traversal");
    group.throughput(Throughput::Elements(rust_count as u64));
    group.sample_size(200);
    group.measurement_time(Duration::from_secs(10));

    group.bench_function("rust-native", |bencher| {
        bencher.iter(|| {
            let mut checksum = 0usize;

            for finding in black_box(rust_report.findings()) {
                let location = finding.location();
                checksum = checksum
                    .wrapping_add(finding.rule_id().as_str().len())
                    .wrapping_add(location.start())
                    .wrapping_add(location.end())
                    .wrapping_add(location.line())
                    .wrapping_add(location.column());
            }

            black_box(checksum)
        });
    });

    group.bench_function("c-abi", |bencher| {
        bencher.iter(|| {
            let mut checksum = 0usize;

            for index in 0..black_box(capi_count) {
                let mut finding = CribraFindingView::default();

                let status = unsafe {
                    cribra_report_finding_at(black_box(capi_report), black_box(index), &mut finding)
                };
                assert_eq!(status, CRIBRA_OK);

                checksum = checksum
                    .wrapping_add(finding.rule_id.len)
                    .wrapping_add(finding.start)
                    .wrapping_add(finding.end)
                    .wrapping_add(finding.line)
                    .wrapping_add(finding.column);
            }

            black_box(checksum)
        });
    });

    group.finish();

    unsafe {
        cribra_report_free(capi_report);
        cribra_scanner_free(capi_scanner);
    }
}

fn string_view(value: &str) -> CribraStringView {
    CribraStringView {
        ptr: value.as_ptr(),
        len: value.len(),
    }
}

fn bench_transform_overhead(criterion: &mut Criterion) {
    let source = dense_finding_source(256);

    let rust_scanner = Scanner::default();
    let rust_results = rust_scanner.scan([((), source.as_str())]);
    let rust_report = rust_results
        .single_report()
        .expect("one source must produce one native report");

    assert!(rust_report.len() >= 128);

    let mut capi_scanner: *mut CribraScanner = ptr::null_mut();
    let mut capi_report: *mut CribraReport = ptr::null_mut();
    let mut capi_error: *mut CribraError = ptr::null_mut();

    unsafe {
        assert_eq!(cribra_scanner_new_current(&mut capi_scanner), CRIBRA_OK);
        assert_eq!(
            cribra_scanner_scan(
                capi_scanner,
                source.as_ptr(),
                source.len(),
                &mut capi_report,
                &mut capi_error,
            ),
            CRIBRA_OK
        );
    }

    assert!(capi_error.is_null());
    assert!(!capi_report.is_null());

    // Validate equivalent transform behavior before timing.
    let native = redact(source.as_str(), rust_report).expect("native redaction must succeed");

    let mut capi_output: *mut CribraOutput = ptr::null_mut();
    unsafe {
        assert_eq!(
            cribra_transform_redact(
                source.as_ptr(),
                source.len(),
                capi_report,
                &mut capi_output,
                &mut capi_error,
            ),
            CRIBRA_OK
        );
    }
    assert!(capi_error.is_null());
    assert!(!capi_output.is_null());
    unsafe { cribra_output_free(capi_output) };

    let mut group = criterion.benchmark_group("capi/transform-redact");
    group.throughput(Throughput::Bytes(source.len() as u64));
    group.sample_size(150);
    group.measurement_time(Duration::from_secs(10));

    group.bench_function("rust-native", |bencher| {
        bencher.iter(|| {
            let output = redact(black_box(source.as_str()), black_box(rust_report))
                .expect("native redaction must succeed");
            black_box(output)
        });
    });

    group.bench_function("c-abi", |bencher| {
        bencher.iter(|| {
            let mut output: *mut CribraOutput = ptr::null_mut();
            let mut error: *mut CribraError = ptr::null_mut();

            let status = unsafe {
                cribra_transform_redact(
                    black_box(source.as_ptr()),
                    black_box(source.len()),
                    black_box(capi_report),
                    &mut output,
                    &mut error,
                )
            };

            assert_eq!(status, CRIBRA_OK);
            assert!(error.is_null());
            assert!(!output.is_null());

            black_box(output);
            unsafe {
                cribra_output_free(output);
                cribra_error_free(error);
            }
        });
    });

    group.finish();

    black_box(native);

    unsafe {
        cribra_report_free(capi_report);
        cribra_scanner_free(capi_scanner);
    }
}

fn batch_sources(count: usize, bytes_per_source: usize) -> (Vec<String>, Vec<String>) {
    let keys = (0..count)
        .map(|index| format!("input-{index:03}"))
        .collect::<Vec<_>>();
    let sources = (0..count)
        .map(|_| clean_source(bytes_per_source))
        .collect::<Vec<_>>();
    (keys, sources)
}

fn bench_batch_amortization(criterion: &mut Criterion) {
    const INPUT_COUNT: usize = 32;
    const BYTES_PER_SOURCE: usize = 64 * 1024;

    let (keys, sources) = batch_sources(INPUT_COUNT, BYTES_PER_SOURCE);
    let total_bytes = INPUT_COUNT * BYTES_PER_SOURCE;

    let native_entries = sources
        .iter()
        .enumerate()
        .map(|(index, source)| (index, source.as_str()))
        .collect::<Vec<_>>();

    let capi_inputs = keys
        .iter()
        .zip(&sources)
        .map(|(key, source)| CribraBatchInput {
            key: string_view(key),
            source: string_view(source),
        })
        .collect::<Vec<_>>();

    let rust_scanner = Scanner::default();

    let mut capi_scanner: *mut CribraScanner = ptr::null_mut();
    unsafe {
        assert_eq!(cribra_scanner_new_current(&mut capi_scanner), CRIBRA_OK);
    }

    // Semantic sanity check outside timing.
    let native_results = rust_scanner.scan(native_entries.iter().copied());
    assert_eq!(native_results.len(), INPUT_COUNT);

    let mut capi_results: *mut CribraBatchResults = ptr::null_mut();
    let mut capi_error: *mut CribraError = ptr::null_mut();
    unsafe {
        assert_eq!(
            cribra_scanner_scan_batch(
                capi_scanner,
                capi_inputs.as_ptr(),
                capi_inputs.len(),
                CRIBRA_BATCH_EXECUTION_SERIAL,
                &mut capi_results,
                &mut capi_error,
            ),
            CRIBRA_OK
        );
    }
    assert!(capi_error.is_null());

    let mut capi_count = 0usize;
    unsafe {
        assert_eq!(
            cribra_batch_results_count(capi_results, &mut capi_count),
            CRIBRA_OK
        );
    }
    assert_eq!(capi_count, INPUT_COUNT);
    unsafe { cribra_batch_results_free(capi_results) };

    let mut group = criterion.benchmark_group("capi/batch-32x64KiB-serial");
    group.throughput(Throughput::Bytes(total_bytes as u64));
    group.sample_size(30);
    group.measurement_time(Duration::from_secs(12));

    group.bench_function("rust-native", |bencher| {
        bencher.iter(|| {
            let results = rust_scanner.scan(black_box(native_entries.iter().copied()));
            black_box(results)
        });
    });

    group.bench_function("c-abi", |bencher| {
        bencher.iter(|| {
            let mut results: *mut CribraBatchResults = ptr::null_mut();
            let mut error: *mut CribraError = ptr::null_mut();

            let status = unsafe {
                cribra_scanner_scan_batch(
                    black_box(capi_scanner),
                    black_box(capi_inputs.as_ptr()),
                    black_box(capi_inputs.len()),
                    CRIBRA_BATCH_EXECUTION_SERIAL,
                    &mut results,
                    &mut error,
                )
            };

            assert_eq!(status, CRIBRA_OK);
            assert!(error.is_null());
            assert!(!results.is_null());

            black_box(results);

            unsafe {
                cribra_batch_results_free(results);
                cribra_error_free(error);
            }
        });
    });

    group.finish();

    unsafe { cribra_scanner_free(capi_scanner) };
}

criterion_group!(
    benches,
    bench_minimal_call_overhead,
    bench_single_scan_overhead,
    bench_report_traversal,
    bench_transform_overhead,
    bench_batch_amortization
);
criterion_main!(benches);
