#pragma once

#include <cribra.h>

#include <array>
#include <cstddef>
#include <cstdint>
#include <memory>
#include <string>

namespace cribra::rn {

/// Best-effort wipe that the compiler cannot elide as a dead store.
///
/// This header deliberately has no JSI or React Native dependency so the
/// ownership primitives can be exercised by JSI-free host lifecycle tests.
inline void SecureZero(void* data, size_t size) noexcept {
  if (data == nullptr || size == 0) {
    return;
  }
  volatile auto* bytes = static_cast<volatile uint8_t*>(data);
  for (size_t index = 0; index < size; ++index) {
    bytes[index] = 0;
  }
}

/// Owns one CribraScanner handle. Destruction is the only place that frees it.
class ScannerHandle final {
 public:
  explicit ScannerHandle(CribraScanner* scanner) noexcept : scanner_(scanner) {}
  ~ScannerHandle() noexcept { cribra_scanner_free(scanner_); }

  ScannerHandle(const ScannerHandle&) = delete;
  ScannerHandle& operator=(const ScannerHandle&) = delete;
  ScannerHandle(ScannerHandle&&) = delete;
  ScannerHandle& operator=(ScannerHandle&&) = delete;

  [[nodiscard]] CribraScanner* get() const noexcept { return scanner_; }

 private:
  CribraScanner* scanner_;
};

/// One copied batch input. Both strings are owned for the whole native call.
struct ScanInput {
  std::string id;
  std::string source;
};

/// One native copy of the caller-provided 32-byte transformation key.
///
/// Non-copyable. The destructor volatile-zeroes the bytes. A shared pointer to
/// this object gives the copyable worker job shared ownership without ever
/// duplicating key material.
inline constexpr size_t kTransformKeyBytes = 32;

struct KeyBytes {
  KeyBytes() = default;
  ~KeyBytes() noexcept { SecureZero(bytes.data(), bytes.size()); }

  KeyBytes(const KeyBytes&) = delete;
  KeyBytes& operator=(const KeyBytes&) = delete;

  std::array<uint8_t, kTransformKeyBytes> bytes{};
};

/// ABI-defined transformation mode for the bridge.
enum class TransformMode : uint8_t {
  Redact,
  Template,
  Pseudonymize,
  Synthesize,
};

/// One fully copied transformation request. Only this plain value crosses the
/// worker boundary. `key` is null for non-keyed modes.
struct TransformInput {
  TransformInput() = default;
  ~TransformInput() noexcept {
    // Best-effort wipe of the copied source. The caller's original string and
    // any JavaScript/Hermes copies cannot be reached from here.
    if (!text.empty()) {
      SecureZero(text.data(), text.size());
    }
  }

  TransformInput(const TransformInput&) = delete;
  TransformInput& operator=(const TransformInput&) = delete;

  TransformMode mode = TransformMode::Redact;
  std::string text;
  /// Replacement, namespace, prefix, or marker depending on `mode`.
  std::string option;
  bool hasReplacement = false;
  bool numbered = false;
  uint32_t digestBytes = 16;
  std::shared_ptr<KeyBytes> key;
};

}  // namespace cribra::rn
