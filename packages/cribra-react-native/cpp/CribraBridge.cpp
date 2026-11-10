#include "CribraBridge.h"

#include "CribraAbi.h"
#include "CribraEnums.h"

#include <utility>

namespace cribra::rn {
namespace {

std::string StatusLabel(CribraStatus status) {
  switch (status) {
    case CRIBRA_OK:
      return "ok";
    case CRIBRA_INVALID_ARGUMENT:
      return "invalid_argument";
    case CRIBRA_INVALID_UTF8:
      return "invalid_utf8";
    case CRIBRA_INTERNAL_ERROR:
      return "internal_error";
    default:
      return "status_" + std::to_string(status);
  }
}

/// Status-first error text so the facade's stable `cribra_native_error:PREFIX`
/// parser preserves the real failure category. The operation is diagnostic
/// context only and never includes source text.
std::string Failure(const char* operation, CribraStatus status) {
  std::string message = std::string("cribra_native_error:") + StatusLabel(status);
  if (operation != nullptr && *operation != '\0') {
    message += ":";
    message += operation;
  }
  return message;
}

[[noreturn]] void ProtocolError(const char* field, uint32_t code) {
  throw BridgeError(
      std::string("cribra_native_error:invalid_native_report:") + field + ":" +
      std::to_string(code));
}

/// Returns an empty string when the linked C ABI is compatible with the
/// vendored header. The failure text is source-free and carries only versions.
std::string AbiCompatibilityFailure() {
  const auto major = cribra_abi_version_major();
  const auto minor = cribra_abi_version_minor();
  const auto patch = cribra_abi_version_patch();
  if (AbiCompatible(major, minor, patch)) {
    return {};
  }
  return "cribra_native_error:abi_mismatch:expected=" +
      AbiVersionText(kExpectedAbiMajor, kExpectedAbiMinor, kExpectedAbiPatch) +
      ":actual=" + AbiVersionText(major, minor, patch);
}

double ToJsNumber(uint64_t value, const char* field) {
  if (!FitsJsSafeInteger(value)) {
    throw BridgeError(std::string("cribra_native_error:numeric_overflow:") + field);
  }
  return static_cast<double>(value);
}

/// Checked accumulation: a summary counter can never exceed JavaScript's safe
/// integer range, even if a future Core returns an unexpectedly large value.
void CheckedAdd(uint64_t& accumulator, uint64_t value, const char* field) {
  if (value > kMaxJsSafeInteger - accumulator) {
    throw BridgeError(std::string("cribra_native_error:numeric_overflow:") + field);
  }
  accumulator += value;
}

std::string BorrowedString(const CribraStringView& view) {
  if (view.ptr == nullptr || view.len == 0) {
    return {};
  }
  return std::string(reinterpret_cast<const char*>(view.ptr), view.len);
}

/// Owns a CribraError for the duration of one bridge operation.
class OwnedError final {
 public:
  OwnedError() = default;
  ~OwnedError() { cribra_error_free(error_); }

  OwnedError(const OwnedError&) = delete;
  OwnedError& operator=(const OwnedError&) = delete;

  [[nodiscard]] CribraError** out() noexcept { return &error_; }

  [[nodiscard]] std::string describe(
      CribraStatus fallback,
      const char* operation) const {
    CribraStatus status = fallback;
    std::string message = Failure(operation, status);

    if (error_ == nullptr) {
      return message;
    }

    if (cribra_error_status(error_, &status) == CRIBRA_OK) {
      message = Failure(operation, status);
    }

    CribraStringView view{};
    if (cribra_error_message(error_, &view) == CRIBRA_OK && view.ptr != nullptr) {
      const auto detail = BorrowedString(view);
      if (!detail.empty()) {
        message += ":";
        message += detail;
      }
    }
    return message;
  }

 private:
  CribraError* error_ = nullptr;
};

/// Owns a CribraReport for the duration of one scan projection.
class OwnedReport final {
 public:
  explicit OwnedReport(CribraReport* report) noexcept : report_(report) {}
  ~OwnedReport() { cribra_report_free(report_); }

  OwnedReport(const OwnedReport&) = delete;
  OwnedReport& operator=(const OwnedReport&) = delete;

  [[nodiscard]] const CribraReport* get() const noexcept { return report_; }

 private:
  CribraReport* report_;
};

void AssignExplanation(
    const CribraExplanationView& explanation,
    std::string& kind,
    std::string& value) {
  const auto mapped = MapExplanation(
      explanation.kind,
      explanation.detection_mode,
      explanation.candidate_evidence);
  if (!mapped) {
    // Include the three source-free codes so an impossible kind/payload
    // combination is diagnosable without weakening the failure.
    throw BridgeError(
        "cribra_native_error:invalid_native_report:explanation:" +
        std::to_string(explanation.kind) + ":" +
        std::to_string(explanation.detection_mode) + ":" +
        std::to_string(explanation.candidate_evidence));
  }
  kind = std::string(mapped->kind);
  value = std::string(mapped->value);
}

std::vector<FindingDto> ProjectFindings(
    const ScannerHandle& scanner,
    const CribraReport* report,
    size_t findingCount,
    SeverityCounts& counts) {
  std::vector<FindingDto> findings;
  findings.reserve(findingCount);

  for (size_t index = 0; index < findingCount; ++index) {
    CribraFindingView view{};
    const auto viewStatus = cribra_report_finding_at(report, index, &view);
    if (viewStatus != CRIBRA_OK) {
      throw BridgeError(Failure("finding_at", viewStatus));
    }

    CribraExplanationView explanation{};
    OwnedError explanationError;
    const auto explanationStatus = cribra_scanner_explain_finding(
        scanner.get(), report, index, &explanation, explanationError.out());
    if (explanationStatus != CRIBRA_OK) {
      throw BridgeError(
          explanationError.describe(explanationStatus, "explain_finding"));
    }

    const auto severity = MapSeverity(view.severity);
    if (!severity) {
      ProtocolError("finding.severity", view.severity);
    }
    switch (severity->rank) {
      case SeverityRank::Info:
        ++counts.info;
        break;
      case SeverityRank::Low:
        ++counts.low;
        break;
      case SeverityRank::Medium:
        ++counts.medium;
        break;
      case SeverityRank::High:
        ++counts.high;
        break;
      case SeverityRank::Critical:
        ++counts.critical;
        break;
    }

    const auto confidence = MapConfidence(view.confidence);
    if (!confidence) {
      ProtocolError("finding.confidence", view.confidence);
    }
    const auto remediation = MapRemediation(view.remediation);
    if (!remediation) {
      ProtocolError("finding.remediation", view.remediation);
    }

    FindingDto dto{};
    dto.ruleId = BorrowedString(view.rule_id);
    dto.start = ToJsNumber(view.start, "finding.start");
    dto.end = ToJsNumber(view.end, "finding.end");
    dto.line = ToJsNumber(view.line, "finding.line");
    dto.column = ToJsNumber(view.column, "finding.column");
    dto.severity = std::string(severity->label);
    dto.confidence = std::string(*confidence);
    dto.remediation = std::string(*remediation);
    AssignExplanation(explanation, dto.explanationKind, dto.explanationValue);
    findings.push_back(std::move(dto));
  }

  return findings;
}

std::vector<CandidateDto> ProjectCandidates(
    const CribraReport* report,
    size_t candidateCount) {
  std::vector<CandidateDto> candidates;
  candidates.reserve(candidateCount);

  for (size_t index = 0; index < candidateCount; ++index) {
    CribraCandidateView view{};
    const auto viewStatus = cribra_report_candidate_at(report, index, &view);
    if (viewStatus != CRIBRA_OK) {
      throw BridgeError(Failure("candidate_at", viewStatus));
    }

    CribraExplanationView explanation{};
    const auto explanationStatus =
        cribra_report_explain_candidate(report, index, &explanation);
    if (explanationStatus != CRIBRA_OK) {
      throw BridgeError(Failure("candidate_explain", explanationStatus));
    }

    const auto kind = MapCandidateKind(view.kind);
    if (!kind) {
      ProtocolError("candidate.kind", view.kind);
    }
    const auto evidence = MapCandidateEvidence(view.evidence);
    if (!evidence) {
      ProtocolError("candidate.evidence", view.evidence);
    }

    CandidateDto dto{};
    dto.kind = std::string(*kind);
    dto.start = ToJsNumber(view.start, "candidate.start");
    dto.end = ToJsNumber(view.end, "candidate.end");
    dto.line = ToJsNumber(view.line, "candidate.line");
    dto.column = ToJsNumber(view.column, "candidate.column");
    dto.evidence = std::string(*evidence);
    AssignExplanation(explanation, dto.explanationKind, dto.explanationValue);
    candidates.push_back(std::move(dto));
  }

  return candidates;
}

}  // namespace

ScannerHandle::~ScannerHandle() { cribra_scanner_free(scanner_); }

std::shared_ptr<ScannerHandle> CreateDefaultScanner(std::string& errorMessage) {
  const auto abiFailure = AbiCompatibilityFailure();
  if (!abiFailure.empty()) {
    errorMessage = abiFailure;
    return nullptr;
  }

  CribraScanner* scanner = nullptr;
  const auto status = cribra_scanner_new_current(&scanner);
  if (status != CRIBRA_OK || scanner == nullptr) {
    errorMessage = Failure("scanner_new_current", status);
    return nullptr;
  }
  return std::make_shared<ScannerHandle>(scanner);
}

ScanProjection Scan(
    const std::shared_ptr<ScannerHandle>& scanner,
    const std::string& sourceId,
    const std::string& text) {
  const auto abiFailure = AbiCompatibilityFailure();
  if (!abiFailure.empty()) {
    throw BridgeError(abiFailure);
  }

  CribraReport* rawReport = nullptr;
  OwnedError scanError;

  const auto* bytes =
      reinterpret_cast<const uint8_t*>(text.empty() ? nullptr : text.data());
  const auto scanStatus = cribra_scanner_scan(
      scanner->get(), bytes, text.size(), &rawReport, scanError.out());
  if (scanStatus != CRIBRA_OK || rawReport == nullptr) {
    throw BridgeError(scanError.describe(scanStatus, "scan"));
  }
  OwnedReport report(rawReport);

  size_t findingCount = 0;
  const auto findingCountStatus =
      cribra_report_finding_count(report.get(), &findingCount);
  if (findingCountStatus != CRIBRA_OK) {
    throw BridgeError(Failure("finding_count", findingCountStatus));
  }

  size_t candidateCount = 0;
  const auto candidateCountStatus =
      cribra_report_candidate_count(report.get(), &candidateCount);
  if (candidateCountStatus != CRIBRA_OK) {
    throw BridgeError(Failure("candidate_count", candidateCountStatus));
  }

  ScanProjection projection;
  projection.sourceBytes = static_cast<uint64_t>(text.size());
  projection.findingCount = static_cast<uint64_t>(findingCount);
  projection.candidateCount = static_cast<uint64_t>(candidateCount);

  ScanReportDto dto{};
  dto.sourceId = sourceId;
  dto.sourceBytes = ToJsNumber(projection.sourceBytes, "source_bytes");
  dto.findings = ProjectFindings(
      *scanner, report.get(), findingCount, projection.severity);
  dto.candidateCount = ToJsNumber(projection.candidateCount, "candidate_count");
  // Core's report.needs_review() is true for findings or candidates.
  dto.needsReview = findingCount > 0 || candidateCount > 0;
  dto.candidates = ProjectCandidates(report.get(), candidateCount);
  projection.report = std::move(dto);
  return projection;
}

ScanBatchDto ScanBatch(
    const std::shared_ptr<ScannerHandle>& scanner,
    const std::vector<ScanInput>& inputs) {
  ScanBatchDto output;
  output.entries.reserve(inputs.size());

  uint64_t scannedBytes = 0;
  uint64_t reportsWithFindings = 0;
  uint64_t reportsWithCandidates = 0;
  uint64_t totalFindings = 0;
  uint64_t totalCandidates = 0;
  SeverityCounts severity;

  for (const auto& input : inputs) {
    auto projection = Scan(scanner, input.id, input.source);
    CheckedAdd(scannedBytes, projection.sourceBytes, "summary.scannedBytes");
    CheckedAdd(
        totalFindings, projection.findingCount, "summary.totalFindings");
    CheckedAdd(
        totalCandidates, projection.candidateCount, "summary.totalCandidates");
    if (projection.findingCount > 0) {
      ++reportsWithFindings;
    }
    if (projection.candidateCount > 0) {
      ++reportsWithCandidates;
    }
    CheckedAdd(severity.critical, projection.severity.critical, "summary.critical");
    CheckedAdd(severity.high, projection.severity.high, "summary.high");
    CheckedAdd(severity.medium, projection.severity.medium, "summary.medium");
    CheckedAdd(severity.low, projection.severity.low, "summary.low");
    CheckedAdd(severity.info, projection.severity.info, "summary.info");
    output.entries.push_back(std::move(projection.report));
  }

  const uint64_t scannedSources = static_cast<uint64_t>(inputs.size());
  if (reportsWithFindings > scannedSources) {
    throw BridgeError(
        "cribra_native_error:invalid_native_report:summary.reportsWithFindings");
  }

  ScanSummaryDto summary{};
  summary.scannedSources = ToJsNumber(scannedSources, "summary.scannedSources");
  summary.scannedBytes = ToJsNumber(scannedBytes, "summary.scannedBytes");
  summary.reportsWithFindings =
      ToJsNumber(reportsWithFindings, "summary.reportsWithFindings");
  summary.reportsWithoutFindings = ToJsNumber(
      scannedSources - reportsWithFindings, "summary.reportsWithoutFindings");
  summary.totalFindings = ToJsNumber(totalFindings, "summary.totalFindings");
  summary.totalCandidates =
      ToJsNumber(totalCandidates, "summary.totalCandidates");
  summary.reportsWithCandidates = ToJsNumber(
      reportsWithCandidates, "summary.reportsWithCandidates");
  summary.hasCandidates = totalCandidates != 0;
  summary.critical = ToJsNumber(severity.critical, "summary.critical");
  summary.high = ToJsNumber(severity.high, "summary.high");
  summary.medium = ToJsNumber(severity.medium, "summary.medium");
  summary.low = ToJsNumber(severity.low, "summary.low");
  summary.info = ToJsNumber(severity.info, "summary.info");
  summary.isClean = totalFindings == 0 && totalCandidates == 0;
  summary.hasCritical = severity.critical != 0;
  output.summary = std::move(summary);
  return output;
}

AbiVersionDto GetAbiVersion() noexcept {
  return AbiVersionDto{
      static_cast<double>(cribra_abi_version_major()),
      static_cast<double>(cribra_abi_version_minor()),
      static_cast<double>(cribra_abi_version_patch())};
}

}  // namespace cribra::rn
