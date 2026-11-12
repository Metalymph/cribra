#pragma once

#include "CribraBridge.h"
#include "CribraSerialWorker.h"

#include <jsi/jsi.h>

#include <cstdint>
#include <memory>
#include <mutex>
#include <unordered_map>

namespace facebook::react {

class CribraReactNative final
    : public NativeCribraReactNativeCxxSpec<CribraReactNative> {
 public:
  explicit CribraReactNative(std::shared_ptr<CallInvoker> jsInvoker);
  ~CribraReactNative() override;

  jsi::Object getAbiVersion(jsi::Runtime& rt);

  jsi::Value createDefaultScanner(jsi::Runtime& rt);

  jsi::Value createScanner(
      jsi::Runtime& rt,
      jsi::String builtins,
      bool financialBuiltins,
      bool personalBuiltins,
      jsi::Array rules);

  jsi::Value scan(
      jsi::Runtime& rt,
      double scannerId,
      jsi::String sourceId,
      jsi::String text);

  jsi::Value scanBatch(
      jsi::Runtime& rt,
      double scannerId,
      jsi::Array sources);

  jsi::Value transformRedact(
      jsi::Runtime& rt,
      double scannerId,
      jsi::String text,
      jsi::String replacement,
      bool hasReplacement);

  jsi::Value transformTemplate(
      jsi::Runtime& rt,
      double scannerId,
      jsi::String text,
      jsi::String namespaceName,
      bool numbered);

  jsi::Value transformPseudonymize(
      jsi::Runtime& rt,
      double scannerId,
      jsi::String text,
      jsi::ArrayBuffer key,
      jsi::String prefix,
      double digestBytes);

  jsi::Value transformSynthesize(
      jsi::Runtime& rt,
      double scannerId,
      jsi::String text,
      jsi::ArrayBuffer key,
      jsi::String marker);

  jsi::Value disposeScanner(jsi::Runtime& rt, double scannerId);

  double getLiveScannerCount(jsi::Runtime& rt);

 private:
  using ScannerPtr = std::shared_ptr<cribra::rn::ScannerHandle>;

  [[nodiscard]] ScannerPtr findScanner(uint64_t scannerId) const;
  void registerScanner(uint64_t scannerId, ScannerPtr scanner);
  [[nodiscard]] ScannerPtr takeScanner(uint64_t scannerId);
  void ClearScanners() noexcept;
  jsi::Value runTransform(
      jsi::Runtime& rt,
      double scannerId,
      std::shared_ptr<cribra::rn::TransformInput> input);

  mutable std::mutex scannersMutex_;
  std::unordered_map<uint64_t, ScannerPtr> scanners_;
  uint64_t nextScannerId_ = 1;

  /// Closes before the worker stops so no new JS settlement is scheduled during
  /// module teardown. Declared before the worker so it outlives every job.
  std::shared_ptr<SettlementGate> settlementGate_;

  // Declared last so the worker is joined before the registry and this object
  // are torn down.
  CribraSerialWorker worker_;
};

}  // namespace facebook::react
