#include "cribra.h"

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static int fail(const char *message) {
    fprintf(stderr, "cribra-capi smoke failure: %s\n", message);
    return EXIT_FAILURE;
}

static int view_equals(CribraStringView view, const char *expected) {
    const size_t expected_len = strlen(expected);
    return view.len == expected_len &&
           (expected_len == 0 || memcmp(view.ptr, expected, expected_len) == 0);
}

/* Real C byte-stream consumer, using the same owned report/read surface. */
static int stream_smoke(CribraScanner *builtins) {
    CribraBuilder *builder = NULL;
    CribraScanner *scanner = NULL;
    CribraStream *stream = NULL;
    CribraReport *whole = NULL;
    CribraReport *report = NULL;
    CribraOutput *output = NULL;
    CribraError *error = NULL;
    const char *failure = NULL;
    static const uint8_t source[] =
        "\xc3\xa9\xf0\x9f\x99\x82 SECRET\nABCD-EFGH-IJKL-MNOP";
    const size_t source_len = sizeof(source) - 1;
    const CribraRuleConfig rule = {
        .kind = CRIBRA_RULE_KIND_LITERAL,
        .id = {(const uint8_t *)"literal", 7},
        .value = {(const uint8_t *)"SECRET", 6},
        .severity = CRIBRA_SEVERITY_HIGH,
        .remediation = CRIBRA_REMEDIATION_REMOVE_SENSITIVE_VALUE
    };
    CribraStreamLimits limits = {0};
    CribraStatus status;
    size_t count = 0;
    CribraFindingView finding = {0};
    CribraFindingView expected = {0};
    CribraCandidateView candidate = {0};
    CribraCandidateView expected_candidate = {0};

#define STREAM_CHECK(condition, message) \
    do { if (!(condition)) { failure = (message); goto cleanup; } } while (0)

    status = cribra_scanner_stream_new(builtins, &stream, &error);
    STREAM_CHECK(status == CRIBRA_STREAM_UNSUPPORTED_RULE_SET && stream == NULL && error != NULL,
                 "built-in streaming admission did not fail closed");
    cribra_error_free(error);
    error = NULL;
    STREAM_CHECK(cribra_builder_new(&builder) == CRIBRA_OK, "stream builder failed");
    STREAM_CHECK(cribra_builder_add_rule(builder, &rule, &error) == CRIBRA_OK && error == NULL,
                 "stream custom rule failed");
    status = cribra_builder_build(builder, &scanner, &error);
    builder = NULL; /* Build consumes the builder, including on build failure. */
    STREAM_CHECK(status == CRIBRA_OK && scanner != NULL && error == NULL, "stream scanner failed");

    limits.max_stream_storage_bytes_enabled = 1;
    limits.max_stream_storage_bytes = 0;
    status = cribra_scanner_stream_new_with_limits(scanner, &limits, &stream, &error);
    STREAM_CHECK(status == CRIBRA_STREAM_STORAGE_LIMIT_EXCEEDED && stream == NULL && error != NULL,
                 "enabled zero storage budget was ignored");
    cribra_error_free(error);
    error = NULL;
    limits.max_stream_storage_bytes_enabled = 0;
    limits.max_retained_results_enabled = 1;
    limits.max_retained_results = 0;
    STREAM_CHECK(cribra_scanner_stream_new_with_limits(scanner, &limits, &stream, &error) == CRIBRA_OK,
                 "result-limited stream failed admission");
    status = cribra_stream_push(stream, source, source_len, &error);
    STREAM_CHECK(status == CRIBRA_STREAM_RESULT_LIMIT_EXCEEDED && error != NULL,
                 "stream result budget was ignored");
    cribra_error_free(error);
    error = NULL;
    STREAM_CHECK(cribra_stream_finish(stream, &report, NULL) == CRIBRA_STREAM_NOT_ACTIVE && report == NULL,
                 "failed stream returned a report");
    cribra_stream_free(stream);
    stream = NULL;

    limits.max_retained_results_enabled = 0;
    limits.max_candidates_enabled = 1;
    limits.max_candidates = 0;
    STREAM_CHECK(cribra_scanner_stream_new_with_limits(scanner, &limits, &stream, &error) == CRIBRA_OK,
                 "candidate-limited stream failed admission");
    STREAM_CHECK(cribra_stream_push(stream, source, source_len, &error) == CRIBRA_STREAM_CANDIDATE_LIMIT_EXCEEDED,
                 "stream candidate budget was ignored");
    cribra_error_free(error);
    error = NULL;
    cribra_stream_free(stream);
    stream = NULL;

    STREAM_CHECK(cribra_scanner_scan(scanner, source, source_len, &whole, &error) == CRIBRA_OK,
                 "whole-source stream oracle failed");
    STREAM_CHECK(cribra_scanner_stream_new(scanner, &stream, &error) == CRIBRA_OK && error == NULL,
                 "unlimited stream admission failed");
    cribra_scanner_free(scanner); /* The stream must survive the scanner. */
    scanner = NULL;
    STREAM_CHECK(cribra_stream_push(stream, NULL, 0, NULL) == CRIBRA_OK, "empty push failed");
    for (size_t i = 0; i < source_len; ++i) {
        /* Includes every interior byte of both the two- and four-byte scalars. */
        uint8_t temporary = source[i];
        STREAM_CHECK(cribra_stream_push(stream, &temporary, 1, &error) == CRIBRA_OK && error == NULL,
                     "byte-oriented UTF-8 push failed");
    }
    STREAM_CHECK(cribra_stream_finish(stream, &report, &error) == CRIBRA_OK && report != NULL && error == NULL,
                 "stream finish failed");
    {
        CribraReport *again = NULL;
        STREAM_CHECK(cribra_stream_finish(stream, &again, NULL) == CRIBRA_STREAM_NOT_ACTIVE && again == NULL,
                     "repeated finish did not report inactive");
        STREAM_CHECK(cribra_stream_push(stream, NULL, 0, NULL) == CRIBRA_STREAM_NOT_ACTIVE,
                     "push after finish did not report inactive");
    }
    cribra_stream_free(stream);
    stream = NULL;
    STREAM_CHECK(cribra_report_finding_count(report, &count) == CRIBRA_OK && count == 1,
                 "stream finding count mismatch");
    STREAM_CHECK(cribra_report_finding_at(report, 0, &finding) == CRIBRA_OK &&
                 cribra_report_finding_at(whole, 0, &expected) == CRIBRA_OK,
                 "stream finding traversal failed");
    STREAM_CHECK(view_equals(finding.rule_id, "literal") && finding.start == expected.start &&
                 finding.end == expected.end && finding.line == expected.line &&
                 finding.column == expected.column && finding.severity == expected.severity &&
                 finding.confidence == expected.confidence && finding.remediation == expected.remediation &&
                 finding.start == 7 && finding.end == 13 && finding.column == 4,
                 "stream finding/UTF-8 parity mismatch");
    STREAM_CHECK(cribra_report_candidate_count(report, &count) == CRIBRA_OK && count == 1,
                 "stream candidate count mismatch");
    STREAM_CHECK(cribra_report_candidate_at(report, 0, &candidate) == CRIBRA_OK &&
                 cribra_report_candidate_at(whole, 0, &expected_candidate) == CRIBRA_OK,
                 "stream candidate traversal failed");
    STREAM_CHECK(candidate.kind == expected_candidate.kind && candidate.evidence == expected_candidate.evidence &&
                 candidate.start == expected_candidate.start && candidate.end == expected_candidate.end &&
                 candidate.line == expected_candidate.line && candidate.column == expected_candidate.column,
                 "stream candidate parity mismatch");
    STREAM_CHECK(cribra_transform_redact(source, source_len, report, &output, &error) == CRIBRA_OK,
                 "stream report source-byte accounting mismatch");
    cribra_output_free(output);
    output = NULL;
    STREAM_CHECK(cribra_transform_redact(source, source_len - 1, report, &output, &error) == CRIBRA_TRANSFORM_ERROR && output == NULL,
                 "stream report accepted mismatched source length");
    cribra_error_free(error);
    error = NULL;

cleanup:
    cribra_error_free(error);
    cribra_output_free(output);
    cribra_report_free(report);
    cribra_report_free(whole);
    cribra_stream_free(stream);
    cribra_scanner_free(scanner);
    cribra_builder_free(builder);
#undef STREAM_CHECK
    return failure == NULL ? EXIT_SUCCESS : fail(failure);
}

int main(void) {
    if (cribra_abi_version_major() != 0 || cribra_abi_version_minor() != 2 || cribra_abi_version_patch() != 0) {
        return fail("unexpected ABI major version");
    }

    CribraScanner *scanner = NULL;
    if (cribra_scanner_new_current(&scanner) != CRIBRA_OK || scanner == NULL) {
        return fail("could not create current scanner");
    }

    /*
     * Synthetic test credential only. Never use real credentials in ABI tests.
     */
    static const uint8_t source[] =
        "GITHUB_TOKEN=ghp_AbCdEf0123456789_AbCdEf0123456789";

    CribraReport *report = NULL;
    CribraError *error = NULL;

    CribraStatus status = cribra_scanner_scan(
        scanner,
        source,
        sizeof(source) - 1,
        &report,
        &error
    );

    if (status != CRIBRA_OK || report == NULL || error != NULL) {
        cribra_error_free(error);
        cribra_report_free(report);
        cribra_scanner_free(scanner);
        return fail("scan failed");
    }

    size_t finding_count = 0;
    if (cribra_report_finding_count(report, &finding_count) != CRIBRA_OK ||
        finding_count == 0) {
        cribra_report_free(report);
        cribra_scanner_free(scanner);
        return fail("expected at least one finding");
    }

    CribraFindingView finding = {0};
    if (cribra_report_finding_at(report, 0, &finding) != CRIBRA_OK) {
        cribra_report_free(report);
        cribra_scanner_free(scanner);
        return fail("could not read first finding");
    }

    if (finding.rule_id.ptr == NULL || finding.rule_id.len == 0) {
        cribra_report_free(report);
        cribra_scanner_free(scanner);
        return fail("finding rule id is empty");
    }

    if (finding.start >= finding.end || finding.end > sizeof(source) - 1) {
        cribra_report_free(report);
        cribra_scanner_free(scanner);
        return fail("finding span is invalid");
    }

    if (finding.line == 0 || finding.column == 0) {
        cribra_report_free(report);
        cribra_scanner_free(scanner);
        return fail("finding line/column must be one-based");
    }

    cribra_report_free(report);
    report = NULL;

    /*
     * Exercise the explicit error object with deliberately invalid UTF-8.
     */
    static const uint8_t invalid_utf8[] = {0xff};

    status = cribra_scanner_scan(
        scanner,
        invalid_utf8,
        sizeof(invalid_utf8),
        &report,
        &error
    );

    if (status != CRIBRA_INVALID_UTF8 || report != NULL || error == NULL) {
        cribra_error_free(error);
        cribra_report_free(report);
        cribra_scanner_free(scanner);
        return fail("invalid UTF-8 did not fail closed");
    }

    CribraStatus error_status = CRIBRA_OK;
    if (cribra_error_status(error, &error_status) != CRIBRA_OK ||
        error_status != CRIBRA_INVALID_UTF8) {
        cribra_error_free(error);
        cribra_scanner_free(scanner);
        return fail("error status mismatch");
    }

    CribraStringView message = {0};
    if (cribra_error_message(error, &message) != CRIBRA_OK ||
        message.ptr == NULL ||
        message.len == 0) {
        cribra_error_free(error);
        cribra_scanner_free(scanner);
        return fail("missing error diagnostic");
    }

    if (!view_equals(message, "input is not valid UTF-8")) {
        cribra_error_free(error);
        cribra_scanner_free(scanner);
        return fail("unexpected error diagnostic");
    }

    cribra_error_free(error);
    if (stream_smoke(scanner) != EXIT_SUCCESS) {
        cribra_scanner_free(scanner);
        return EXIT_FAILURE;
    }
    cribra_scanner_free(scanner);

    puts("cribra-capi smoke: ok");
    return EXIT_SUCCESS;
}
