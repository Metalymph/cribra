#include "cribra.h"

#include <cstdint>
#include <cstdlib>
#include <iostream>
#include <stdexcept>
#include <string_view>
#include <utility>

namespace {

std::string_view view(CribraStringView value) {
    return {
        reinterpret_cast<const char *>(value.ptr),
        value.len,
    };
}

class Scanner {
public:
    explicit Scanner(CribraScanner *scanner) : scanner_(scanner) {}

    Scanner(const Scanner &) = delete;
    Scanner &operator=(const Scanner &) = delete;

    Scanner(Scanner &&other) noexcept
        : scanner_(std::exchange(other.scanner_, nullptr)) {}

    Scanner &operator=(Scanner &&other) noexcept {
        if (this != &other) {
            cribra_scanner_free(scanner_);
            scanner_ = std::exchange(other.scanner_, nullptr);
        }
        return *this;
    }

    ~Scanner() {
        cribra_scanner_free(scanner_);
    }

    [[nodiscard]] CribraScanner *get() const {
        return scanner_;
    }

private:
    CribraScanner *scanner_ = nullptr;
};

class Report {
public:
    explicit Report(CribraReport *report) : report_(report) {}

    Report(const Report &) = delete;
    Report &operator=(const Report &) = delete;

    Report(Report &&other) noexcept
        : report_(std::exchange(other.report_, nullptr)) {}

    Report &operator=(Report &&other) noexcept {
        if (this != &other) {
            cribra_report_free(report_);
            report_ = std::exchange(other.report_, nullptr);
        }
        return *this;
    }

    ~Report() {
        cribra_report_free(report_);
    }

    [[nodiscard]] CribraReport *get() const {
        return report_;
    }

private:
    CribraReport *report_ = nullptr;
};

class Output {
public:
    explicit Output(CribraOutput *output) : output_(output) {}

    Output(const Output &) = delete;
    Output &operator=(const Output &) = delete;

    Output(Output &&other) noexcept
        : output_(std::exchange(other.output_, nullptr)) {}

    Output &operator=(Output &&other) noexcept {
        if (this != &other) {
            cribra_output_free(output_);
            output_ = std::exchange(other.output_, nullptr);
        }
        return *this;
    }

    ~Output() {
        cribra_output_free(output_);
    }

    [[nodiscard]] CribraOutput *get() const {
        return output_;
    }

private:
    CribraOutput *output_ = nullptr;
};

Scanner make_scanner() {
    CribraBuilder *builder = nullptr;

    if (cribra_builder_new(&builder) != CRIBRA_OK || builder == nullptr) {
        throw std::runtime_error("could not create builder");
    }

    if (cribra_builder_add_current_builtins(builder) != CRIBRA_OK) {
        cribra_builder_free(builder);
        throw std::runtime_error("could not add current built-ins");
    }

    if (cribra_builder_add_financial_builtins(builder) != CRIBRA_OK) {
        cribra_builder_free(builder);
        throw std::runtime_error("could not add financial built-ins");
    }

    CribraScanner *scanner = nullptr;
    CribraError *error = nullptr;

    /*
     * The build call consumes builder on every non-null attempt.
     */
    const CribraStatus status =
        cribra_builder_build(builder, &scanner, &error);

    if (status != CRIBRA_OK || scanner == nullptr || error != nullptr) {
        cribra_error_free(error);
        cribra_scanner_free(scanner);
        throw std::runtime_error("could not build scanner");
    }

    return Scanner(scanner);
}

} // namespace

int main() {
    try {
        const Scanner scanner = make_scanner();

        constexpr std::string_view source =
            "service=checkout\n"
            "card_number=1234567890123452\n"
            "iban=IT60X0542811101000000123456\n"
            "log_level=info\n";

        CribraReport *raw_report = nullptr;
        CribraError *error = nullptr;

        CribraStatus status = cribra_scanner_scan(
            scanner.get(),
            reinterpret_cast<const std::uint8_t *>(source.data()),
            source.size(),
            &raw_report,
            &error
        );

        if (status != CRIBRA_OK || raw_report == nullptr || error != nullptr) {
            cribra_error_free(error);
            cribra_report_free(raw_report);
            throw std::runtime_error("scan failed");
        }

        const Report report(raw_report);

        std::size_t finding_count = 0;

        if (cribra_report_finding_count(report.get(), &finding_count) != CRIBRA_OK) {
            throw std::runtime_error("could not read finding count");
        }

        std::cout << "classified " << finding_count << " finding(s)\n";

        for (std::size_t index = 0; index < finding_count; ++index) {
            CribraFindingView finding{};

            if (cribra_report_finding_at(report.get(), index, &finding) != CRIBRA_OK) {
                throw std::runtime_error("could not read finding");
            }

            std::cout
                << view(finding.rule_id)
                << " line=" << finding.line
                << " column=" << finding.column
                << " severity=" << static_cast<int>(finding.severity)
                << " confidence=" << static_cast<int>(finding.confidence)
                << '\n';
        }

        CribraOutput *raw_output = nullptr;
        error = nullptr;

        status = cribra_transform_redact(
            reinterpret_cast<const std::uint8_t *>(source.data()),
            source.size(),
            report.get(),
            &raw_output,
            &error
        );

        if (status != CRIBRA_OK || raw_output == nullptr || error != nullptr) {
            cribra_error_free(error);
            cribra_output_free(raw_output);
            throw std::runtime_error("redaction failed");
        }

        const Output output(raw_output);

        CribraStringView redacted{};

        if (cribra_output_view(output.get(), &redacted) != CRIBRA_OK) {
            throw std::runtime_error("could not read transformed output");
        }

        std::cout << "\nSafe derivative:\n";
        std::cout << view(redacted) << '\n';

        return EXIT_SUCCESS;
    } catch (const std::exception &error) {
        std::cerr << "cribra C++ example: " << error.what() << '\n';
        return EXIT_FAILURE;
    }
}