#pragma once

#include <cribra.h>

#include <cstdint>
#include <string>

namespace cribra::rn {

// The vendored header is the ABI contract this package was compiled against.
inline constexpr uint32_t kExpectedAbiMajor = ABI_VERSION_MAJOR;
inline constexpr uint32_t kExpectedAbiMinor = ABI_VERSION_MINOR;
inline constexpr uint32_t kExpectedAbiPatch = ABI_VERSION_PATCH;

/// Largest integer representable exactly by an IEEE-754 double and therefore
/// the largest value JavaScript can address with Number.MAX_SAFE_INTEGER.
inline constexpr uint64_t kMaxJsSafeInteger = 9007199254740991ull;

constexpr bool FitsJsSafeInteger(uint64_t value) noexcept {
  return value <= kMaxJsSafeInteger;
}

/// Major and minor identify the protocol surface. Patch releases are expected
/// to remain compatible within a minor version, so a patch difference alone is
/// tolerated; any major or minor difference fails closed before scanner work.
constexpr bool AbiCompatible(
    uint32_t major,
    uint32_t minor,
    uint32_t /*patch*/) noexcept {
  return major == kExpectedAbiMajor && minor == kExpectedAbiMinor;
}

inline std::string AbiVersionText(
    uint32_t major,
    uint32_t minor,
    uint32_t patch) {
  return std::to_string(major) + "." + std::to_string(minor) + "." +
      std::to_string(patch);
}

}  // namespace cribra::rn
