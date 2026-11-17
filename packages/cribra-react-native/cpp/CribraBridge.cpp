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
    case CRIBRA_RULE_ERROR:
      return "rule_error";
    case CRIBRA_BUILD_ERROR:
      return "build_error";
    case CRIBRA_INTERNAL_ERROR:
      return "internal_error";
    case CRIBRA_TRANSFORM_ERROR:
      return "transform_failed";
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
  explicit OwnedError(CribraError* error) noexcept : error_(error) {}
  ~OwnedError() { cribra_error_free(error_); }

  OwnedError(const OwnedError&) = delete;
  OwnedError& operator=(const OwnedError&) = delete;

  [[nodiscard]] CribraError** out() noexcept { return &error_; }
  [[nodiscard]] CribraError* get() const noexcept { return error_; }

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

/// Owns a CribraOutput for the duration of one transformation.
class OwnedOutput final {
 public:
  explicit OwnedOutput(CribraOutput* output) noexcept : output_(output) {}
  ~OwnedOutput() { cribra_output_free(output_); }

  OwnedOutput(const OwnedOutput&) = delete;
  OwnedOutput& operator=(const OwnedOutput&) = delete;

  [[nodiscard]] const CribraOutput* get() const noexcept { return output_; }
  [[nodiscard]] explicit operator bool() const noexcept {
    return output_ != nullptr;
  }

 private:
  CribraOutput* output_;
};

/// Null for an empty string so the C ABI sees the documented empty-view form.
const uint8_t* Bytes(const std::string& value) noexcept {
  return value.empty()
      ? nullptr
      : reinterpret_cast<const uint8_t*>(value.data());
}

/// Source-free operation name used only in transform diagnostics.
const char* OperationName(TransformMode mode) noexcept {
  switch (mode) {
    case TransformMode::Redact:
      return "redact";
    case TransformMode::Template:
      return "template";
    case TransformMode::Pseudonymize:
      return "pseudonymize";
    case TransformMode::Synthesize:
      return "synthesize";
  }
  return "transform";
}

/// Dispatches exactly one C ABI transformation against the live report.
CribraStatus ApplyTransform(
    const TransformInput& input,
    const std::string& text,
    const CribraReport* report,
    CribraOutput** out_output,
    CribraError** out_error) {
  const auto* source = Bytes(text);
  const auto sourceLen = text.size();

  switch (input.mode) {
    case TransformMode::Redact:
      if (!input.hasReplacement) {
        return cribra_transform_redact(
            source, sourceLen, report, out_output, out_error);
      }
      return cribra_transform_redact_with(
          source,
          sourceLen,
          report,
          Bytes(input.option),
          input.option.size(),
          out_output,
          out_error);
    case TransformMode::Template: {
      CribraTemplateConfig config{};
      config.namespace_ =
          CribraStringView{Bytes(input.option), input.option.size()};
      config.numbered = input.numbered ? 1 : 0;
      return cribra_transform_template_with(
          source, sourceLen, report, &config, out_output, out_error);
    }
    case TransformMode::Pseudonymize: {
      CribraPseudonymizeConfig config{};
      config.key = input.key ? input.key->bytes.data() : nullptr;
      config.key_len = input.key ? input.key->bytes.size() : 0;
      config.prefix =
          CribraStringView{Bytes(input.option), input.option.size()};
      config.digest_bytes = input.digestBytes;
      return cribra_transform_pseudonymize(
          source, sourceLen, report, &config, out_output, out_error);
    }
    case TransformMode::Synthesize: {
      CribraSynthesizeConfig config{};
      config.key = input.key ? input.key->bytes.data() : nullptr;
      config.key_len = input.key ? input.key->bytes.size() : 0;
      config.marker =
          CribraStringView{Bytes(input.option), input.option.size()};
      return cribra_transform_synthesize(
          source, sourceLen, report, &config, out_output, out_error);
    }
  }
  return CRIBRA_INVALID_ARGUMENT;
}

/// Owns an unconsumed builder. `cribra_builder_build` takes ownership of the
/// handle even when construction fails, so the bridge marks consumption before
/// any later return path. A consumed builder is never freed again.
class BuilderHandle final {
 public:
  explicit BuilderHandle(CribraBuilder* builder) noexcept : builder_(builder) {}
  ~BuilderHandle() {
    if (!consumed_) {
      cribra_builder_free(builder_);
    }
  }

  BuilderHandle(const BuilderHandle&) = delete;
  BuilderHandle& operator=(const BuilderHandle&) = delete;

  [[nodiscard]] CribraBuilder* get() const noexcept { return builder_; }
  void MarkConsumed() noexcept { consumed_ = true; }

 private:
  CribraBuilder* builder_;
  bool consumed_ = false;
};

/// A failure from a call without an error handle (builder creation or catalog
/// addition). The message carries only a status label and operation name.
CreationFailure StatusFailure(const char* operation, CribraStatus status) {
  CreationFailure failure;
  failure.category = StatusLabel(status);
  failure.detail = Failure(operation, status);
  return failure;
}

/// Copies structured construction metadata out of one owned error before the
/// error handle is released. The rule ID view is borrowed, so it is copied
/// synchronously here; the pattern/matcher value is never available.
CreationFailure ErrorFailure(
    const char* operation,
    CribraStatus status,
    const CribraError* error) {
  CreationFailure failure;
  failure.category = StatusLabel(status);

  if (error != nullptr) {
    CribraBuildErrorView info{};
    if (cribra_error_build_info(error, &info) == CRIBRA_OK) {
      failure.buildCode = info.code;
      failure.ruleId = BorrowedString(info.rule_id);
    }

    CribraStringView message{};
    if (cribra_error_message(error, &message) == CRIBRA_OK) {
      failure.detail = BorrowedString(message);
    }
  }

  if (failure.detail.empty()) {
    failure.detail = Failure(operation, status);
  }
  return failure;
}

/// Copies every active rule's metadata into detached, source-free values.
/// Borrowed views are read while the scanner is alive; failures throw and let
/// the caller release the newly created scanner.
std::vector<RuleMetadataEntry> ProjectRuleMetadata(
    const ScannerHandle& scanner) {
  size_t count = 0;
  const auto countStatus = cribra_scanner_rule_count(scanner.get(), &count);
  if (countStatus != CRIBRA_OK) {
    throw BridgeError(Failure("rule_count", countStatus));
  }
  if (!FitsJsSafeInteger(static_cast<uint64_t>(count))) {
    throw BridgeError("cribra_native_error:numeric_overflow:rules");
  }

  std::vector<RuleMetadataEntry> entries;
  entries.reserve(count);
  for (size_t index = 0; index < count; ++index) {
    CribraRuleMetadataView view{};
    const auto status = cribra_scanner_rule_at(scanner.get(), index, &view);
    if (status != CRIBRA_OK) {
      throw BridgeError(Failure("rule_at", status));
    }

    const auto kind = MapRuleKind(view.kind);
    if (!kind) {
      ProtocolError("rule.kind", view.kind);
    }
    const auto detectionMode = MapRuleDetectionMode(view.detection_mode);
    if (!detectionMode) {
      ProtocolError("rule.detection_mode", view.detection_mode);
    }
    const auto severity = MapSeverity(view.severity);
    if (!severity) {
      ProtocolError("rule.severity", view.severity);
    }
    const auto remediation = MapRemediation(view.remediation);
    if (!remediation) {
      ProtocolError("rule.remediation", view.remediation);
    }

    RuleMetadataEntry entry;
    entry.ruleId = BorrowedString(view.rule_id);
    // Core rejects empty identifiers at build time, so an active rule with an
    // empty ID is a protocol violation, not a valid projection.
    if (entry.ruleId.empty()) {
      throw BridgeError(
          "cribra_native_error:invalid_native_report:rule.rule_id");
    }
    entry.kind = std::string(*kind);
    entry.detectionMode = std::string(*detectionMode);
    entry.severity = std::string(severity->label);
    entry.remediation = std::string(*remediation);
    entries.push_back(std::move(entry));
  }

  return entries;
}

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

ScannerCreation CreateScanner(const ScannerConfigInput& config) {
  ScannerCreation creation;

  const auto abiFailure = AbiCompatibilityFailure();
  if (!abiFailure.empty()) {
    creation.failure.category = "abi_mismatch";
    creation.failure.detail = abiFailure;
    return creation;
  }

  std::shared_ptr<ScannerHandle> scanner;

  if (config.IsCanonicalDefault()) {
    // Preserve the released default path exactly: Core reuses the already
    // compiled canonical catalog instead of recompiling it per scanner.
    CribraScanner* rawScanner = nullptr;
    const auto status = cribra_scanner_new_current(&rawScanner);
    if (status != CRIBRA_OK || rawScanner == nullptr) {
      creation.failure = StatusFailure("scanner_new_current", status);
      return creation;
    }
    scanner = std::make_shared<ScannerHandle>(rawScanner);
  } else {
    CribraBuilder* rawBuilder = nullptr;
    const auto newStatus = cribra_builder_new(&rawBuilder);
    if (newStatus != CRIBRA_OK || rawBuilder == nullptr) {
      creation.failure = StatusFailure("builder_new", newStatus);
      return creation;
    }
    // Any return below this point frees the builder unless it has been
    // consumed by cribra_builder_build.
    BuilderHandle builder(rawBuilder);

    struct CatalogStep {
      bool enabled;
      const char* operation;
      CribraStatus (*add)(CribraBuilder*);
    };
    const CatalogStep catalogs[] = {
        {config.current,
         "add_current_builtins",
         cribra_builder_add_current_builtins},
        {config.financial,
         "add_financial_builtins",
         cribra_builder_add_financial_builtins},
        {config.personal,
         "add_personal_builtins",
         cribra_builder_add_personal_builtins},
    };
    for (const auto& catalog : catalogs) {
      if (!catalog.enabled) {
        continue;
      }
      const auto status = catalog.add(builder.get());
      if (status != CRIBRA_OK) {
        creation.failure = StatusFailure(catalog.operation, status);
        return creation;
      }
    }

    for (const auto& rule : config.rules) {
      // Views borrow the owned strings for exactly this ABI call. The C ABI
      // copies both values before returning.
      CribraRuleConfig ruleConfig{};
      ruleConfig.kind = rule.kind;
      ruleConfig.id = CribraStringView{
          reinterpret_cast<const uint8_t*>(rule.id.data()), rule.id.size()};
      ruleConfig.value = CribraStringView{
          reinterpret_cast<const uint8_t*>(rule.value.data()), rule.value.size()};
      ruleConfig.severity = rule.severity;
      ruleConfig.remediation = rule.remediation;

      CribraError* rawError = nullptr;
      const auto status =
          cribra_builder_add_rule(builder.get(), &ruleConfig, &rawError);
      OwnedError error(rawError);
      if (status != CRIBRA_OK) {
        creation.failure = ErrorFailure("add_rule", status, error.get());
        return creation;
      }
    }

    CribraScanner* rawScanner = nullptr;
    OwnedError buildError;
    const auto buildStatus =
        cribra_builder_build(builder.get(), &rawScanner, buildError.out());
    // The C ABI consumes every valid builder when out_scanner is writable, on
    // success and on failure. This call always supplies a writable slot, so the
    // handle is dead here regardless of the returned status.
    builder.MarkConsumed();
    if (buildStatus != CRIBRA_OK || rawScanner == nullptr) {
      creation.failure = ErrorFailure("build", buildStatus, buildError.get());
      return creation;
    }
    scanner = std::make_shared<ScannerHandle>(rawScanner);
  }

  // Metadata is validated and copied before the scanner can be registered by
  // the caller. A projection failure releases the new scanner through RAII.
  creation.rules = ProjectRuleMetadata(*scanner);
  creation.scanner = std::move(scanner);
  return creation;
}

ScannerCreationDto ToDto(
    uint64_t scannerId,
    std::vector<RuleMetadataEntry> rules) {
  ScannerCreationDto dto{};
  dto.scannerId = static_cast<double>(scannerId);
  dto.failure = "";
  dto.buildCode = 0.0;
  dto.ruleId = "";
  dto.detail = "";
  dto.rules.reserve(rules.size());
  for (auto& rule : rules) {
    RuleMetadataDto ruleDto{};
    ruleDto.ruleId = std::move(rule.ruleId);
    ruleDto.kind = std::move(rule.kind);
    ruleDto.detectionMode = std::move(rule.detectionMode);
    ruleDto.severity = std::move(rule.severity);
    ruleDto.remediation = std::move(rule.remediation);
    dto.rules.push_back(std::move(ruleDto));
  }
  return dto;
}

ScannerCreationDto ToDto(const CreationFailure& failure) {
  ScannerCreationDto dto{};
  dto.scannerId = 0.0;
  dto.failure = failure.category;
  dto.buildCode = static_cast<double>(failure.buildCode);
  dto.ruleId = failure.ruleId;
  dto.detail = failure.detail;
  return dto;
}

namespace {

/// Projects one live report into the shared scan DTO. Shared by scan and
/// transform so both paths expose byte-for-byte identical Core-derived fields.
ScanProjection ProjectReport(
    const ScannerHandle& scanner,
    const CribraReport* report,
    const std::string& sourceId,
    uint64_t sourceBytes) {
  size_t findingCount = 0;
  const auto findingCountStatus =
      cribra_report_finding_count(report, &findingCount);
  if (findingCountStatus != CRIBRA_OK) {
    throw BridgeError(Failure("finding_count", findingCountStatus));
  }

  size_t candidateCount = 0;
  const auto candidateCountStatus =
      cribra_report_candidate_count(report, &candidateCount);
  if (candidateCountStatus != CRIBRA_OK) {
    throw BridgeError(Failure("candidate_count", candidateCountStatus));
  }

  ScanProjection projection;
  projection.sourceBytes = sourceBytes;
  projection.findingCount = static_cast<uint64_t>(findingCount);
  projection.candidateCount = static_cast<uint64_t>(candidateCount);

  ScanReportDto dto{};
  dto.sourceId = sourceId;
  dto.sourceBytes = ToJsNumber(projection.sourceBytes, "source_bytes");
  dto.findings =
      ProjectFindings(scanner, report, findingCount, projection.severity);
  dto.candidateCount =
      ToJsNumber(projection.candidateCount, "candidate_count");
  // Core's report.needs_review() is true for findings or candidates.
  dto.needsReview = findingCount > 0 || candidateCount > 0;
  dto.candidates = ProjectCandidates(report, candidateCount);
  projection.report = std::move(dto);
  return projection;
}

}  // namespace

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

  const auto scanStatus = cribra_scanner_scan(
      scanner->get(), Bytes(text), text.size(), &rawReport, scanError.out());
  if (scanStatus != CRIBRA_OK || rawReport == nullptr) {
    throw BridgeError(scanError.describe(scanStatus, "scan"));
  }
  OwnedReport report(rawReport);

  return ProjectReport(
      *scanner,
      report.get(),
      sourceId,
      static_cast<uint64_t>(text.size()));
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

TransformResultDto Transform(
    const std::shared_ptr<ScannerHandle>& scanner,
    const TransformInput& input) {
  const auto abiFailure = AbiCompatibilityFailure();
  if (!abiFailure.empty()) {
    throw BridgeError(abiFailure);
  }

  CribraReport* rawReport = nullptr;
  OwnedError scanError;
  // Exactly one scan of the same UTF-8 source that will be transformed.
  const auto scanStatus = cribra_scanner_scan(
      scanner->get(),
      Bytes(input.text),
      input.text.size(),
      &rawReport,
      scanError.out());
  if (scanStatus != CRIBRA_OK || rawReport == nullptr) {
    throw BridgeError(scanError.describe(scanStatus, "scan"));
  }
  OwnedReport report(rawReport);

  auto projection = ProjectReport(
      *scanner,
      report.get(),
      std::string(),
      static_cast<uint64_t>(input.text.size()));

  CribraOutput* rawOutput = nullptr;
  OwnedError transformError;
  const auto transformStatus = ApplyTransform(
      input, input.text, report.get(), &rawOutput, transformError.out());
  if (transformStatus != CRIBRA_OK) {
    // describe() reads cribra_error_status first, so the `transform_failed`
    // token and the borrowed fixed diagnostic are emitted only when the C ABI
    // actually reported CRIBRA_TRANSFORM_ERROR. All other statuses keep the
    // existing status-first mapping.
    throw BridgeError(
        transformError.describe(transformStatus, OperationName(input.mode)));
  }
  OwnedOutput output(rawOutput);
  if (!output) {
    // Success must always produce a live output handle.
    throw BridgeError(
        "cribra_native_error:invalid_native_report:transform.output");
  }

  CribraStringView view{};
  const auto viewStatus = cribra_output_view(output.get(), &view);
  if (viewStatus != CRIBRA_OK) {
    throw BridgeError(Failure("output_view", viewStatus));
  }

  // Copy the UTF-8 output before the OwnedOutput guard frees it.
  TransformResultDto dto{};
  dto.sourceBytes = projection.report.sourceBytes;
  dto.findings = std::move(projection.report.findings);
  dto.candidateCount = projection.report.candidateCount;
  dto.needsReview = projection.report.needsReview;
  dto.candidates = std::move(projection.report.candidates);
  dto.output = BorrowedString(view);
  return dto;
}

AbiVersionDto GetAbiVersion() noexcept {
  return AbiVersionDto{
      static_cast<double>(cribra_abi_version_major()),
      static_cast<double>(cribra_abi_version_minor()),
      static_cast<double>(cribra_abi_version_patch())};
}

}  // namespace cribra::rn
