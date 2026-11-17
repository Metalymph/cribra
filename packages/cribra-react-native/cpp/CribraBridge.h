#pragma once

#include <CribraReactNativeSpecJSI.h>

#include <cribra.h>

#include "CribraEnums.h"
#include "CribraOwnership.h"

#include <array>
#include <cstddef>
#include <cstdint>
#include <memory>
#include <stdexcept>
#include <string>
#include <vector>

namespace cribra::rn {

using AbiVersionDto =
    facebook::react::NativeCribraReactNativeAbiVersion<double, double, double>;

using FindingDto = facebook::react::NativeCribraReactNativeFindingDto<
    std::string,
    double,
    double,
    double,
    double,
    std::string,
    std::string,
    std::string,
    std::string,
    std::string>;

using CandidateDto = facebook::react::NativeCribraReactNativeCandidateDto<
    std::string,
    double,
    double,
    double,
    double,
    std::string,
    std::string,
    std::string>;

using ScanReportDto = facebook::react::NativeCribraReactNativeScanReportDto<
    std::string,
    double,
    std::vector<FindingDto>,
    double,
    bool,
    std::vector<CandidateDto>>;

using ScanSummaryDto =
    facebook::react::NativeCribraReactNativeScanSummaryDto<
        double,
        double,
        double,
        double,
        double,
        double,
        double,
        bool,
        double,
        double,
        double,
        double,
        double,
        bool,
        bool>;

using ScanBatchDto = facebook::react::NativeCribraReactNativeScanBatchDto<
    std::vector<ScanReportDto>,
    ScanSummaryDto>;

using TransformResultDto =
    facebook::react::NativeCribraReactNativeTransformResultDto<
        double,
        std::vector<FindingDto>,
        double,
        bool,
        std::vector<CandidateDto>,
        std::string>;

using RuleSpecDto = facebook::react::NativeCribraReactNativeRuleSpecDto<
    std::string,
    std::string,
    std::string,
    std::string,
    std::string>;

using RuleMetadataDto = facebook::react::NativeCribraReactNativeRuleMetadataDto<
    std::string,
    std::string,
    std::string,
    std::string,
    std::string>;

using ScannerCreationDto =
    facebook::react::NativeCribraReactNativeScannerCreationDto<
        double,
        std::vector<RuleMetadataDto>,
        std::string,
        double,
        std::string,
        std::string>;

/// Raised by the synchronous bridge for ABI failures. The message is derived
/// from Cribra's privacy-safe status metadata and never includes source text.
class BridgeError : public std::runtime_error {
 public:
  explicit BridgeError(const std::string& message) : std::runtime_error(message) {}
};

/// One copied custom rule. Strings are owned through every ABI call that
/// borrows them and are never retained by Core.
struct RuleSpecInput {
  uint32_t kind = CRIBRA_RULE_KIND_LITERAL;
  std::string id;
  std::string value;
  uint32_t severity = CRIBRA_SEVERITY_MEDIUM;
  uint32_t remediation = CRIBRA_REMEDIATION_NONE;
};

/// Fully copied scanner configuration. Only this plain value crosses the
/// worker boundary; no JSI object is ever touched off the JS thread.
struct ScannerConfigInput {
  bool current = true;
  bool financial = false;
  bool personal = false;
  std::vector<RuleSpecInput> rules;

  /// The configuration whose Core result is identical to `Scanner::default()`.
  [[nodiscard]] bool IsCanonicalDefault() const noexcept {
    return IsCanonicalDefaultConfig(
        current, financial, personal, !rules.empty());
  }
};

/// One already-validated, detached rule metadata projection. Strings are
/// copied out of the scanner before its handle can be released.
struct RuleMetadataEntry {
  std::string ruleId;
  std::string kind;
  std::string detectionMode;
  std::string severity;
  std::string remediation;
};

/// Structured, source-free construction failure. `buildCode` and `ruleId` come
/// only from `cribra_error_build_info`; `detail` is never parsed.
struct CreationFailure {
  std::string category;
  uint32_t buildCode = CRIBRA_BUILD_CODE_NONE;
  std::string ruleId;
  std::string detail;
};

struct ScannerCreation {
  std::shared_ptr<ScannerHandle> scanner;
  std::vector<RuleMetadataEntry> rules;
  CreationFailure failure;
};

struct SeverityCounts {
  uint64_t info = 0;
  uint64_t low = 0;
  uint64_t medium = 0;
  uint64_t high = 0;
  uint64_t critical = 0;
};

/// One projected report plus the Core metadata needed for native aggregation.
struct ScanProjection {
  ScanReportDto report;
  uint64_t sourceBytes = 0;
  uint64_t findingCount = 0;
  uint64_t candidateCount = 0;
  SeverityCounts severity;
};

/// Checks the linked C ABI against the vendored header, then returns nullptr
/// and fills `errorMessage` when the ABI or scanner construction fails.
std::shared_ptr<ScannerHandle> CreateDefaultScanner(std::string& errorMessage);

/// Builds one scanner from a fully copied configuration.
///
/// Ownership: the builder is owned by a scope guard until
/// `cribra_builder_build` consumes it. The guard never frees a consumed
/// builder. Core build errors are captured structurally and returned; metadata
/// protocol violations throw `BridgeError` after the new scanner is released.
ScannerCreation CreateScanner(const ScannerConfigInput& config);

/// Projects one successful creation into its generated DTO.
ScannerCreationDto ToDto(
    uint64_t scannerId,
    std::vector<RuleMetadataEntry> rules);

/// Projects one structured failure into its generated DTO.
ScannerCreationDto ToDto(const CreationFailure& failure);

/// Rechecks the linked C ABI, runs one synthetic scan, and materialises the
/// source-free projection. Throws BridgeError on any ABI failure.
ScanProjection Scan(
    const std::shared_ptr<ScannerHandle>& scanner,
    const std::string& sourceId,
    const std::string& text);

/// Runs the ordered inputs through serial `cribra_scanner_scan` calls and
/// aggregates the source-free summary. Throws on the first failure; the caller
/// never observes a partial batch.
ScanBatchDto ScanBatch(
    const std::shared_ptr<ScannerHandle>& scanner,
    const std::vector<ScanInput>& inputs);

/// Scans one source and transforms it with the report from that exact scan.
///
/// The report, output and error handles are owned by RAII guards. The output
/// view is copied before `cribra_output_free`. Throws BridgeError on any ABI,
/// Core or protocol failure; the caller never observes partial output.
TransformResultDto Transform(
    const std::shared_ptr<ScannerHandle>& scanner,
    const TransformInput& input);

AbiVersionDto GetAbiVersion() noexcept;

}  // namespace cribra::rn

namespace facebook::react {

template <>
struct Bridging<cribra::rn::AbiVersionDto>
    : NativeCribraReactNativeAbiVersionBridging<cribra::rn::AbiVersionDto> {};

template <>
struct Bridging<cribra::rn::FindingDto>
    : NativeCribraReactNativeFindingDtoBridging<cribra::rn::FindingDto> {};

template <>
struct Bridging<cribra::rn::CandidateDto>
    : NativeCribraReactNativeCandidateDtoBridging<cribra::rn::CandidateDto> {};

template <>
struct Bridging<cribra::rn::ScanReportDto>
    : NativeCribraReactNativeScanReportDtoBridging<cribra::rn::ScanReportDto> {};

template <>
struct Bridging<cribra::rn::ScanSummaryDto>
    : NativeCribraReactNativeScanSummaryDtoBridging<
          cribra::rn::ScanSummaryDto> {};

template <>
struct Bridging<cribra::rn::ScanBatchDto>
    : NativeCribraReactNativeScanBatchDtoBridging<cribra::rn::ScanBatchDto> {};

template <>
struct Bridging<cribra::rn::TransformResultDto>
    : NativeCribraReactNativeTransformResultDtoBridging<
          cribra::rn::TransformResultDto> {};

template <>
struct Bridging<cribra::rn::RuleSpecDto>
    : NativeCribraReactNativeRuleSpecDtoBridging<cribra::rn::RuleSpecDto> {};

template <>
struct Bridging<cribra::rn::RuleMetadataDto>
    : NativeCribraReactNativeRuleMetadataDtoBridging<
          cribra::rn::RuleMetadataDto> {};

template <>
struct Bridging<cribra::rn::ScannerCreationDto>
    : NativeCribraReactNativeScannerCreationDtoBridging<
          cribra::rn::ScannerCreationDto> {};

}  // namespace facebook::react
