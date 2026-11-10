#pragma once

#include <CribraReactNativeSpecJSI.h>

#include <cribra.h>

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

/// Raised by the synchronous bridge for ABI failures. The message is derived
/// from Cribra's privacy-safe status metadata and never includes source text.
class BridgeError : public std::runtime_error {
 public:
  explicit BridgeError(const std::string& message) : std::runtime_error(message) {}
};

/// Owns one CribraScanner handle. Destruction is the only place that frees it.
class ScannerHandle final {
 public:
  explicit ScannerHandle(CribraScanner* scanner) noexcept : scanner_(scanner) {}
  ~ScannerHandle();

  ScannerHandle(const ScannerHandle&) = delete;
  ScannerHandle& operator=(const ScannerHandle&) = delete;
  ScannerHandle(ScannerHandle&&) = delete;
  ScannerHandle& operator=(ScannerHandle&&) = delete;

  [[nodiscard]] CribraScanner* get() const noexcept { return scanner_; }

 private:
  CribraScanner* scanner_;
};

/// Checks the linked C ABI against the vendored header, then returns nullptr
/// and fills `errorMessage` when the ABI or scanner construction fails.
std::shared_ptr<ScannerHandle> CreateDefaultScanner(std::string& errorMessage);

/// Rechecks the linked C ABI, runs one synthetic scan, and materialises the
/// source-free DTO. Throws BridgeError on any ABI failure.
ScanReportDto Scan(
    const std::shared_ptr<ScannerHandle>& scanner,
    const std::string& sourceId,
    const std::string& text);

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

}  // namespace facebook::react
