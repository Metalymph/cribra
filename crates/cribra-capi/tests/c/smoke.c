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

/* C11 incremental consumer: Core owns the document; snapshots own their reports. */
static int incremental_smoke(void) {
    CribraBuilder *builder = NULL;
    CribraScanner *scanner = NULL;
    CribraIncremental *document = NULL;
    CribraIncremental *limited = NULL;
    CribraReport *whole = NULL;
    CribraError *error = NULL;
    CribraIncrementalSnapshot initial = {0};
    CribraIncrementalSnapshot edited = {0};
    CribraIncrementalSnapshot terminal = {0};
    CribraIncrementalLimits limits = {0};
    CribraFindingView finding = {0};
    CribraFindingView expected = {0};
    CribraExplanationView explanation = {0};
    CribraStatus status;
    uint64_t revision = UINT64_MAX;
    uint32_t active = 0;
    size_t count = 0;
    const char *failure = NULL;
    static const uint8_t source[] =
        "\xc3\xa9\xf0\x9f\x99\x82 SECRET\nABCD-EFGH-IJKL-MNOP";
    static const uint8_t secret[] = "SECRET";
    static const uint8_t malformed[] = {0xff};
    const CribraRuleConfig rule = {
        .kind = CRIBRA_RULE_KIND_LITERAL,
        .id = {(const uint8_t *)"literal", 7},
        .value = {secret, sizeof(secret) - 1},
        .severity = CRIBRA_SEVERITY_HIGH,
        .remediation = CRIBRA_REMEDIATION_REMOVE_SENSITIVE_VALUE
    };
#define INCR_CHECK(condition, message) \
    do { if (!(condition)) { failure = (message); goto cleanup; } } while (0)

    INCR_CHECK(cribra_builder_new(&builder) == CRIBRA_OK, "incremental builder failed");
    INCR_CHECK(cribra_builder_add_rule(builder, &rule, &error) == CRIBRA_OK && error == NULL,
               "incremental rule failed");
    status = cribra_builder_build(builder, &scanner, &error);
    builder = NULL;
    INCR_CHECK(status == CRIBRA_OK && scanner != NULL && error == NULL, "incremental scanner failed");
    limits.max_candidates_enabled = 1;
    status = cribra_scanner_incremental_new_with_limits(scanner, secret, sizeof(secret) - 1,
                                                       &limits, &limited, &error);
    INCR_CHECK(status == CRIBRA_INCREMENTAL_CANDIDATE_LIMIT_EXCEEDED && limited == NULL && error != NULL,
               "incremental zero construction budget was ignored");
    cribra_error_free(error);
    error = NULL;
    limits.max_candidates_enabled = 0;
    limits.max_materialized_results_enabled = 1;
    limits.max_materialized_results = 1;
    INCR_CHECK(cribra_scanner_incremental_new_with_limits(scanner, secret, sizeof(secret) - 1,
                                                        &limits, &limited, &error) == CRIBRA_OK,
               "incremental limited construction failed");
    status = cribra_incremental_apply_edit(limited, 0, 0, 0, secret, sizeof(secret) - 1, &revision, &error);
    INCR_CHECK(status == CRIBRA_INCREMENTAL_RESULT_LIMIT_EXCEEDED && revision == 0 && error != NULL,
               "incremental edit result budget was ignored");
    cribra_error_free(error);
    error = NULL;
    INCR_CHECK(cribra_incremental_is_active(limited, &active, &error) == CRIBRA_OK && active == 0,
               "incremental resource failure remained active");
    INCR_CHECK(cribra_incremental_snapshot(limited, &terminal, &error) == CRIBRA_OK &&
               terminal.revision == 0 && terminal.source_bytes == sizeof(secret) - 1 && terminal.report != NULL,
               "incremental terminal snapshot was lost");
    INCR_CHECK(cribra_report_finding_count(terminal.report, &count) == CRIBRA_OK && count == 1,
               "incremental terminal snapshot changed");
    INCR_CHECK(cribra_incremental_apply_edit(limited, UINT64_MAX, 99, 0, NULL, 0, &revision, &error)
               == CRIBRA_INCREMENTAL_NOT_ACTIVE, "terminal incremental session recovered");
    cribra_error_free(error);
    error = NULL;

    INCR_CHECK(cribra_scanner_incremental_new(scanner, source, sizeof(source) - 1, &document, &error) == CRIBRA_OK,
               "incremental construction failed");
    INCR_CHECK(cribra_incremental_snapshot(document, &initial, &error) == CRIBRA_OK &&
               initial.revision == 0 && initial.source_bytes == sizeof(source) - 1 && initial.report != NULL,
               "incremental initial revision/length mismatch");
    INCR_CHECK(cribra_scanner_scan(scanner, source, sizeof(source) - 1, &whole, &error) == CRIBRA_OK,
               "incremental whole-source oracle failed");
    INCR_CHECK(cribra_scanner_explain_finding(scanner, initial.report, 0, &explanation, &error) == CRIBRA_OK &&
               explanation.kind == CRIBRA_EXPLANATION_CLASSIFIED &&
               explanation.detection_mode == CRIBRA_DETECTION_MODE_MATCHER_ONLY,
               "incremental finding explanation mismatch");
    cribra_scanner_free(scanner);
    scanner = NULL; /* The document must outlive the scanner. */
    INCR_CHECK(cribra_incremental_apply_edit(document, 0, 1, 1, NULL, 0, &revision, &error)
               == CRIBRA_INCREMENTAL_INVALID_UTF8_BOUNDARY && revision == 0,
               "incremental accepted a split UTF-8 scalar");
    cribra_error_free(error);
    error = NULL;
    INCR_CHECK(cribra_incremental_apply_edit(document, UINT64_MAX, 99, 0, malformed, sizeof(malformed), &revision, &error)
               == CRIBRA_INVALID_UTF8, "incremental replacement UTF-8 was not checked first");
    cribra_error_free(error);
    error = NULL;
    INCR_CHECK(cribra_incremental_apply_edit(document, 1, 0, 0, NULL, 0, &revision, &error)
               == CRIBRA_INCREMENTAL_STALE_REVISION, "incremental stale revision was accepted");
    cribra_error_free(error);
    error = NULL;
    INCR_CHECK(cribra_incremental_apply_edit(document, 0, 99, 0, NULL, 0, &revision, &error)
               == CRIBRA_INCREMENTAL_INVALID_RANGE, "incremental reversed range was accepted");
    cribra_error_free(error);
    error = NULL;
    INCR_CHECK(cribra_incremental_is_active(document, &active, &error) == CRIBRA_OK && active == 1,
               "recoverable incremental errors poisoned the session");
    INCR_CHECK(cribra_incremental_apply_edit(document, 0, 0, 0, (const uint8_t *)"Z", 1, &revision, &error)
               == CRIBRA_OK && revision == 1, "incremental insertion failed");
    INCR_CHECK(cribra_incremental_apply_edit(document, 1, 0, 1, (const uint8_t *)"Y", 1, &revision, &error)
               == CRIBRA_OK && revision == 2, "incremental replacement failed");
    INCR_CHECK(cribra_incremental_apply_edit(document, 2, 0, 1, NULL, 0, &revision, &error)
               == CRIBRA_OK && revision == 3, "incremental deletion failed");
    INCR_CHECK(cribra_incremental_apply_edit(document, 3, 0, 0, NULL, 0, &revision, &error)
               == CRIBRA_OK && revision == 4, "incremental no-op did not advance revision");
    INCR_CHECK(cribra_incremental_snapshot(document, &edited, &error) == CRIBRA_OK &&
               edited.revision == 4 && edited.source_bytes == sizeof(source) - 1 &&
               edited.report != NULL && edited.report != initial.report,
               "incremental snapshot revision/ownership mismatch");
    cribra_incremental_free(document);
    document = NULL;
    INCR_CHECK(cribra_report_finding_at(edited.report, 0, &finding) == CRIBRA_OK &&
               cribra_report_finding_at(whole, 0, &expected) == CRIBRA_OK,
               "incremental report traversal failed");
    INCR_CHECK(view_equals(finding.rule_id, "literal") && finding.start == expected.start &&
               finding.end == expected.end && finding.line == expected.line && finding.column == expected.column &&
               finding.severity == expected.severity && finding.confidence == expected.confidence &&
               finding.remediation == expected.remediation && finding.start == 7 && finding.column == 4,
               "incremental finding/UTF-8 parity mismatch");
    INCR_CHECK(cribra_report_finding_count(initial.report, &count) == CRIBRA_OK && count == 1,
               "earlier incremental snapshot lost independence");
    INCR_CHECK(cribra_report_candidate_count(edited.report, &count) == CRIBRA_OK && count == 1 &&
               cribra_report_explain_candidate(edited.report, 0, &explanation) == CRIBRA_OK &&
               explanation.kind == CRIBRA_EXPLANATION_AMBIGUOUS &&
               explanation.candidate_evidence == CRIBRA_CANDIDATE_EVIDENCE_STRUCTURAL,
               "incremental review-only candidate parity mismatch");

cleanup:
    cribra_error_free(error);
    cribra_report_free(whole);
    cribra_report_free(initial.report);
    cribra_report_free(edited.report);
    cribra_report_free(terminal.report);
    cribra_incremental_free(document);
    cribra_incremental_free(limited);
    cribra_scanner_free(scanner);
    cribra_builder_free(builder);
#undef INCR_CHECK
    return failure == NULL ? EXIT_SUCCESS : fail(failure);
}

/* Scanner/error borrowed views keep their documented independent owners. */
static int configuration_smoke(void) {
    CribraBuilder *builder = NULL;
    CribraScanner *scanner = NULL;
    CribraError *error = NULL;
    const char *failure = NULL;
    static const uint8_t id[] = "custom/cl\xc3\xa9-\xf0\x9f\x94\x91";
    const CribraRuleConfig rules[] = {
        {CRIBRA_RULE_KIND_LITERAL, {id, sizeof(id)-1}, {(const uint8_t *)"SECRET",6}, CRIBRA_SEVERITY_HIGH, CRIBRA_REMEDIATION_NONE},
        {CRIBRA_RULE_KIND_PREFIX, {(const uint8_t *)"prefix",6}, {(const uint8_t *)"pre_",4}, CRIBRA_SEVERITY_LOW, CRIBRA_REMEDIATION_ROTATE_PASSWORD},
        {CRIBRA_RULE_KIND_SUFFIX, {(const uint8_t *)"suffix",6}, {(const uint8_t *)"_END",4}, CRIBRA_SEVERITY_MEDIUM, CRIBRA_REMEDIATION_NONE},
        {CRIBRA_RULE_KIND_PATTERN, {(const uint8_t *)"pattern",7}, {(const uint8_t *)"code[0-9]{2}",12}, CRIBRA_SEVERITY_CRITICAL, CRIBRA_REMEDIATION_REVIEW_PASSWORD_VERIFIER}
    };
    size_t count = 0;
    CribraRuleMetadataView metadata = {0};
    CribraBuildErrorView info = {0};
#define CONFIG_CHECK(condition, message) \
    do { if (!(condition)) { failure = (message); goto cleanup; } } while (0)
    CONFIG_CHECK(cribra_builder_new(&builder) == CRIBRA_OK, "metadata builder failed");
    CONFIG_CHECK(cribra_builder_add_current_builtins(builder) == CRIBRA_OK, "current catalog failed");
    CONFIG_CHECK(cribra_builder_add_financial_builtins(builder) == CRIBRA_OK, "financial catalog failed");
    CONFIG_CHECK(cribra_builder_add_personal_builtins(builder) == CRIBRA_OK, "personal catalog failed");
    for (size_t i = 0; i < 4; ++i) {
        CONFIG_CHECK(cribra_builder_add_rule(builder, &rules[i], &error) == CRIBRA_OK && error == NULL,
                     "metadata custom rule failed");
    }
    CribraStatus status = cribra_builder_build(builder, &scanner, &error);
    builder = NULL;
    CONFIG_CHECK(status == CRIBRA_OK && error == NULL, "metadata scanner build failed");
    CONFIG_CHECK(cribra_scanner_rule_count(scanner, &count) == CRIBRA_OK && count > 4,
                 "metadata count lost catalogs");
    for (size_t i = 0; i < 4; ++i) {
        CONFIG_CHECK(cribra_scanner_rule_at(scanner, i, &metadata) == CRIBRA_OK,
                     "metadata index failed");
        CONFIG_CHECK(metadata.kind == rules[i].kind && metadata.severity == rules[i].severity &&
                     metadata.remediation == rules[i].remediation && metadata.detection_mode == CRIBRA_DETECTION_MODE_MATCHER_ONLY,
                     "metadata fields/order changed");
    }
    CONFIG_CHECK(cribra_scanner_rule_at(scanner, 0, &metadata) == CRIBRA_OK &&
                 metadata.rule_id.len == sizeof(id)-1 && memcmp(metadata.rule_id.ptr, id, sizeof(id)-1) == 0,
                 "UTF-8 metadata ID changed");
    CONFIG_CHECK(cribra_scanner_rule_at(scanner, count, &metadata) == CRIBRA_OUT_OF_RANGE && metadata.rule_id.ptr == NULL,
                 "metadata invalid index not cleared");
    CONFIG_CHECK(cribra_scanner_rule_at(scanner, 0, NULL) == CRIBRA_INVALID_ARGUMENT,
                 "metadata null output accepted");
    cribra_scanner_free(scanner); scanner = NULL;

    CONFIG_CHECK(cribra_builder_new(&builder) == CRIBRA_OK, "error builder failed");
    CribraRuleConfig invalid = {CRIBRA_RULE_KIND_PATTERN, {(const uint8_t *)"invalid",7},
                               {(const uint8_t *)"[",1}, CRIBRA_SEVERITY_HIGH, CRIBRA_REMEDIATION_NONE};
    CONFIG_CHECK(cribra_builder_add_rule(builder, &invalid, &error) == CRIBRA_RULE_ERROR && error != NULL,
                 "pattern coarse status changed");
    CONFIG_CHECK(cribra_error_build_info(error, &info) == CRIBRA_OK &&
                 info.code == CRIBRA_BUILD_CODE_INVALID_PATTERN && view_equals(info.rule_id, "invalid"),
                 "structured pattern category missing");
    cribra_builder_free(builder); builder = NULL;
    CONFIG_CHECK(view_equals(info.rule_id, "invalid"), "error did not own rule ID");
    cribra_error_free(error); error = NULL;
    CONFIG_CHECK(cribra_error_build_info(NULL, &info) == CRIBRA_INVALID_ARGUMENT &&
                 info.code == CRIBRA_BUILD_CODE_NONE && info.rule_id.ptr == NULL,
                 "null error view not cleared");
    CONFIG_CHECK(cribra_builder_new(&builder) == CRIBRA_OK, "empty matcher builder failed");
    invalid.kind = CRIBRA_RULE_KIND_LITERAL;
    invalid.id = (CribraStringView){(const uint8_t *)"empty",5};
    invalid.value = (CribraStringView){NULL,0};
    CONFIG_CHECK(cribra_builder_add_rule(builder, &invalid, NULL) == CRIBRA_OK,
                 "empty matcher validation moved before Core build");
    status = cribra_builder_build(builder, &scanner, &error); builder = NULL;
    CONFIG_CHECK(status == CRIBRA_BUILD_ERROR && scanner == NULL, "build failure status changed");
    CONFIG_CHECK(cribra_error_build_info(error, &info) == CRIBRA_OK &&
                 info.code == CRIBRA_BUILD_CODE_EMPTY_MATCHER && view_equals(info.rule_id, "empty"),
                 "structured empty matcher category missing");
cleanup:
    cribra_error_free(error); cribra_scanner_free(scanner); cribra_builder_free(builder);
#undef CONFIG_CHECK
    return failure == NULL ? EXIT_SUCCESS : fail(failure);
}

int main(void) {
    if (configuration_smoke() != EXIT_SUCCESS) return EXIT_FAILURE;
    if (cribra_abi_version_major() != 0 || cribra_abi_version_minor() != 3 || cribra_abi_version_patch() != 0) {
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

    if (incremental_smoke() != EXIT_SUCCESS) {
        return EXIT_FAILURE;
    }

    puts("cribra-capi smoke: ok");
    return EXIT_SUCCESS;
}
