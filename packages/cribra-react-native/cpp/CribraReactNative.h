#pragma once

#include "CribraBridge.h"

#include <jsi/jsi.h>

#include <condition_variable>
#include <cstdint>
#include <deque>
#include <functional>
#include <memory>
#include <mutex>
#include <thread>
#include <unordered_map>

namespace facebook::react {

/// Serial native worker. All Cribra ABI calls happen here; only plain C++
/// values cross into and out of the queue. JSI objects never do.
class CribraSerialWorker final {
 public:
  CribraSerialWorker();
  ~CribraSerialWorker();

  CribraSerialWorker(const CribraSerialWorker&) = delete;
  CribraSerialWorker& operator=(const CribraSerialWorker&) = delete;

  void post(std::function<void()> job);

 private:
  void run();

  std::mutex mutex_;
  std::condition_variable condition_;
  std::deque<std::function<void()>> queue_;
  bool stopping_ = false;
  std::thread thread_;
};

class CribraReactNative final
    : public NativeCribraReactNativeCxxSpec<CribraReactNative> {
 public:
  explicit CribraReactNative(std::shared_ptr<CallInvoker> jsInvoker);
  ~CribraReactNative() override;

  jsi::Object getAbiVersion(jsi::Runtime& rt);

  jsi::Value createDefaultScanner(jsi::Runtime& rt);

  jsi::Value scan(
      jsi::Runtime& rt,
      double scannerId,
      jsi::String sourceId,
      jsi::String text);

  jsi::Value disposeScanner(jsi::Runtime& rt, double scannerId);

  double getLiveScannerCount(jsi::Runtime& rt);

 private:
  using ScannerPtr = std::shared_ptr<cribra::rn::ScannerHandle>;

  [[nodiscard]] ScannerPtr findScanner(uint64_t scannerId) const;
  void registerScanner(uint64_t scannerId, ScannerPtr scanner);
  [[nodiscard]] ScannerPtr takeScanner(uint64_t scannerId);

  mutable std::mutex scannersMutex_;
  std::unordered_map<uint64_t, ScannerPtr> scanners_;
  uint64_t nextScannerId_ = 1;

  // Declared last so the worker is joined before the registry and this object
  // are torn down.
  CribraSerialWorker worker_;
};

}  // namespace facebook::react
