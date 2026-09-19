#include "cribra.h"

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

static int fail(const char *message) {
    fprintf(stderr, "cribra example: %s\n", message);
    return EXIT_FAILURE;
}

static void print_view(CribraStringView view) {
    if (view.ptr != NULL && view.len != 0) {
        fwrite(view.ptr, 1, view.len, stdout);
    }
}

int main(void) {
    /*
     * Synthetic data only. Cribra borrows this source while scanning and
     * transforming it; the application retains ownership.
     */
    static const uint8_t source[] =
        "service=checkout\n"
        "card_number=1234567890123452\n"
        "iban=IT60X0542811101000000123456\n"
        "log_level=info\n";

    const size_t source_len = sizeof(source) - 1;

    CribraBuilder *builder = NULL;
    CribraScanner *scanner = NULL;
    CribraReport *report = NULL;
    CribraOutput *output = NULL;
    CribraError *error = NULL;

    if (cribra_builder_new(&builder) != CRIBRA_OK || builder == NULL) {
        return fail("could not create builder");
    }

    if (cribra_builder_add_current_builtins(builder) != CRIBRA_OK) {
        cribra_builder_free(builder);
        return fail("could not add current built-ins");
    }

    if (cribra_builder_add_financial_builtins(builder) != CRIBRA_OK) {
        cribra_builder_free(builder);
        return fail("could not add financial built-ins");
    }

    /*
     * cribra_builder_build consumes builder on every non-null build attempt.
     * Do not free or reuse builder after this call.
     */
    CribraStatus status = cribra_builder_build(builder, &scanner, &error);
    builder = NULL;

    if (status != CRIBRA_OK || scanner == NULL || error != NULL) {
        cribra_error_free(error);
        cribra_scanner_free(scanner);
        return fail("could not build scanner");
    }

    status = cribra_scanner_scan(
        scanner,
        source,
        source_len,
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

    if (cribra_report_finding_count(report, &finding_count) != CRIBRA_OK) {
        cribra_report_free(report);
        cribra_scanner_free(scanner);
        return fail("could not read finding count");
    }

    printf("classified %zu finding(s)\n", finding_count);

    for (size_t index = 0; index < finding_count; ++index) {
        CribraFindingView finding = {0};

        if (cribra_report_finding_at(report, index, &finding) != CRIBRA_OK) {
            cribra_report_free(report);
            cribra_scanner_free(scanner);
            return fail("could not read finding");
        }

        print_view(finding.rule_id);
        printf(
            " line=%zu column=%zu severity=%d confidence=%d\n",
            finding.line,
            finding.column,
            (int)finding.severity,
            (int)finding.confidence
        );
    }

    status = cribra_transform_redact(
        source,
        source_len,
        report,
        &output,
        &error
    );

    if (status != CRIBRA_OK || output == NULL || error != NULL) {
        cribra_error_free(error);
        cribra_output_free(output);
        cribra_report_free(report);
        cribra_scanner_free(scanner);
        return fail("redaction failed");
    }

    CribraStringView redacted = {0};

    if (cribra_output_view(output, &redacted) != CRIBRA_OK) {
        cribra_output_free(output);
        cribra_report_free(report);
        cribra_scanner_free(scanner);
        return fail("could not read transformed output");
    }

    puts("\nSafe derivative:");
    print_view(redacted);
    putchar('\n');

    /*
     * Borrowed views become invalid with their owners. Destroy each
     * Rust-owned handle exactly once.
     */
    cribra_output_free(output);
    cribra_report_free(report);
    cribra_scanner_free(scanner);

    return EXIT_SUCCESS;
}