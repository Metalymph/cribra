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

int main(void) {
    if (cribra_abi_version_major() != 0) {
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
    cribra_scanner_free(scanner);

    puts("cribra-capi smoke: ok");
    return EXIT_SUCCESS;
}
