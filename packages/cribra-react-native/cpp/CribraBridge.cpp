#include "CribraBridge.h"

#include "CribraAbi.h"

#include <utility>

namespace cribra::rn {
namespace {

constexpr uint64_t kMaxJsSafeInteger = 9007199254740991ull;

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

std::string Failure(const char* operation, CribraStatus status) {
  return std::string("cribra_native_error:") + operation + ":" +
      StatusLabel(status);
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

double ToJsNumber(size_t value, const char* field) {
  if (static_cast<uint64_t>(value) > kMaxJsSafeInteger) {
    throw BridgeError(std::string("cribra_native_error:numeric_overflow:") + field);
  }
  return static_cast<double>(value);
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

  [[nodiscard]] std::string describe(CribraStatus fallback) const {
    CribraStatus status = fallback;
    std::string message = Failure("abi", status);

    if (error_ == nullptr) {
      return message;
    }

    if (cribra_error_status(error_, &status) == CRIBRA_OK) {
      message = Failure("abi", status);
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

std::string SeverityLabel(CribraSeverity severity) {
  switch (severity) {
    case CRIBRA_SEVERITY_INFO:
      return "info";
    case CRIBRA_SEVERITY_LOW:
      return "low";
    case CRIBRA_SEVERITY_MEDIUM:
      return "medium";
    case CRIBRA_SEVERITY_HIGH:
      return "high";
    case CRIBRA_SEVERITY_CRITICAL:
      return "critical";
    default:
      return "unknown";
  }
}

std::string ConfidenceLabel(CribraConfidence confidence) {
  switch (confidence) {
    case CRIBRA_CONFIDENCE_LOW:
      return "low";
    case CRIBRA_CONFIDENCE_MEDIUM:
      return "medium";
    case CRIBRA_CONFIDENCE_HIGH:
      return "high";
    default:
      return "unknown";
  }
}

std::string RemediationLabel(CribraRemediation remediation) {
  switch (remediation) {
    case CRIBRA_REMEDIATION_NONE:
      return {};
    case CRIBRA_REMEDIATION_REVOKE_AND_ROTATE_CREDENTIAL:
      return "revoke_and_rotate_credential";
    case CRIBRA_REMEDIATION_ROTATE_CREDENTIAL:
      return "rotate_credential";
    case CRIBRA_REMEDIATION_ROTATE_PASSWORD:
      return "rotate_password";
    case CRIBRA_REMEDIATION_REPLACE_PRIVATE_KEY:
      return "replace_private_key";
    case CRIBRA_REMEDIATION_REMOVE_SENSITIVE_VALUE:
      return "remove_sensitive_value";
    case CRIBRA_REMEDIATION_REVIEW_SENSITIVE_HASH:
      return "review_sensitive_hash";
    case CRIBRA_REMEDIATION_REVIEW_PASSWORD_VERIFIER:
      return "review_password_verifier";
    default:
      return "unknown";
  }
}

std::string DetectionModeLabel(CribraDetectionMode mode) {
  switch (mode) {
    case CRIBRA_DETECTION_MODE_MATCHER_ONLY:
      return "matcher_only";
    case CRIBRA_DETECTION_MODE_DETERMINISTIC:
      return "deterministic";
    case CRIBRA_DETECTION_MODE_CONTEXTUAL:
      return "contextual";
    default:
      return "unknown";
  }
}

std::string CandidateEvidenceLabel(CribraCandidateEvidence evidence) {
  switch (evidence) {
    case CRIBRA_CANDIDATE_EVIDENCE_STRUCTURAL:
      return "structural";
    default:
      return "unknown";
  }
}

std::string CandidateKindLabel(CribraCandidateKind kind) {
  switch (kind) {
    case CRIBRA_CANDIDATE_KIND_RECOVERY_LIKE_CODE:
      return "recovery_like_code";
    default:
      return "unknown";
  }
}

void AssignExplanation(
    const CribraExplanationView& explanation,
    std::string& kind,
    std::string& value) {
  switch (explanation.kind) {
    case CRIBRA_EXPLANATION_NONE:
      kind = "none";
      value.clear();
      return;
    case CRIBRA_EXPLANATION_CLASSIFIED:
      kind = "classified";
      value = DetectionModeLabel(explanation.detection_mode);
      return;
    case CRIBRA_EXPLANATION_AMBIGUOUS:
      kind = "ambiguous";
      value = CandidateEvidenceLabel(explanation.candidate_evidence);
      return;
    default:
      kind = "unknown";
      value.clear();
      return;
  }
}

std::vector<FindingDto> ProjectFindings(
    const ScannerHandle& scanner,
    const CribraReport* report,
    size_t findingCount) {
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
      throw BridgeError(explanationError.describe(explanationStatus));
    }

    FindingDto dto{};
    dto.ruleId = BorrowedString(view.rule_id);
    dto.start = ToJsNumber(view.start, "finding.start");
    dto.end = ToJsNumber(view.end, "finding.end");
    dto.line = ToJsNumber(view.line, "finding.line");
    dto.column = ToJsNumber(view.column, "finding.column");
    dto.severity = SeverityLabel(view.severity);
    dto.confidence = ConfidenceLabel(view.confidence);
    dto.remediation = RemediationLabel(view.remediation);
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

    CandidateDto dto{};
    dto.kind = CandidateKindLabel(view.kind);
    dto.start = ToJsNumber(view.start, "candidate.start");
    dto.end = ToJsNumber(view.end, "candidate.end");
    dto.line = ToJsNumber(view.line, "candidate.line");
    dto.column = ToJsNumber(view.column, "candidate.column");
    dto.evidence = CandidateEvidenceLabel(view.evidence);
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

ScanReportDto Scan(
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
    throw BridgeError(scanError.describe(scanStatus));
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

  ScanReportDto dto{};
  dto.sourceId = sourceId;
  dto.sourceBytes = ToJsNumber(text.size(), "source_bytes");
  dto.findings = ProjectFindings(*scanner, report.get(), findingCount);
  dto.candidateCount = ToJsNumber(candidateCount, "candidate_count");
  // Core's report.needs_review() is true for findings or candidates.
  dto.needsReview = findingCount > 0 || candidateCount > 0;
  dto.candidates = ProjectCandidates(report.get(), candidateCount);
  return dto;
}

AbiVersionDto GetAbiVersion() noexcept {
  return AbiVersionDto{
      static_cast<double>(cribra_abi_version_major()),
      static_cast<double>(cribra_abi_version_minor()),
      static_cast<double>(cribra_abi_version_patch())};
}

}  // namespace cribra::rn
