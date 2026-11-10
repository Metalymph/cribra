#include "CribraReactNative.h"

#include <utility>

namespace facebook::react {
namespace {

constexpr uint64_t kMaxJsSafeInteger = 9007199254740991ull;

jsi::Value PromiseRejected(
    jsi::Runtime& rt,
    const std::string& message,
    const std::shared_ptr<CallInvoker>& jsInvoker) {
  auto promise = AsyncPromise<>(rt, jsInvoker);
  promise.reject(Error(message));
  return bridging::toJs(rt, promise, jsInvoker);
}

}  // namespace

CribraSerialWorker::CribraSerialWorker()
    : thread_(&CribraSerialWorker::run, this) {}

CribraSerialWorker::~CribraSerialWorker() {
  {
    std::lock_guard<std::mutex> lock(mutex_);
    stopping_ = true;
  }
  condition_.notify_all();
  if (thread_.joinable()) {
    thread_.join();
  }
}

void CribraSerialWorker::post(std::function<void()> job) {
  {
    std::lock_guard<std::mutex> lock(mutex_);
    if (stopping_) {
      return;
    }
    queue_.push_back(std::move(job));
  }
  condition_.notify_one();
}

void CribraSerialWorker::run() {
  for (;;) {
    std::function<void()> job;
    {
      std::unique_lock<std::mutex> lock(mutex_);
      condition_.wait(lock, [this] { return stopping_ || !queue_.empty(); });
      if (queue_.empty()) {
        return;
      }
      job = std::move(queue_.front());
      queue_.pop_front();
    }
    job();
  }
}

CribraReactNative::CribraReactNative(std::shared_ptr<CallInvoker> jsInvoker)
    : NativeCribraReactNativeCxxSpec(std::move(jsInvoker)) {}

CribraReactNative::~CribraReactNative() = default;

jsi::Object CribraReactNative::getAbiVersion(jsi::Runtime& rt) {
  return bridging::toJs(rt, cribra::rn::GetAbiVersion(), jsInvoker_);
}

jsi::Value CribraReactNative::createDefaultScanner(jsi::Runtime& rt) {
  auto promise = AsyncPromise<double>(rt, jsInvoker_);
  // Copy, do not move: bridging::toJs below still needs this promise.
  worker_.post([this, promise]() mutable {
    std::string errorMessage;
    auto scanner = cribra::rn::CreateDefaultScanner(errorMessage);
    if (!scanner) {
      promise.reject(Error(errorMessage));
      return;
    }

    uint64_t scannerId = 0;
    {
      std::lock_guard<std::mutex> lock(scannersMutex_);
      scannerId = nextScannerId_++;
    }

    if (scannerId > kMaxJsSafeInteger) {
      promise.reject(Error("cribra_native_error:scanner_id_exhausted"));
      return;
    }

    registerScanner(scannerId, std::move(scanner));
    promise.resolve(static_cast<double>(scannerId));
  });
  return bridging::toJs(rt, promise, jsInvoker_);
}

jsi::Value CribraReactNative::scan(
    jsi::Runtime& rt,
    double scannerId,
    jsi::String sourceId,
    jsi::String text) {
  std::string sourceIdUtf8;
  std::string textUtf8;
  try {
    sourceIdUtf8 = sourceId.utf8(rt);
    textUtf8 = text.utf8(rt);
  } catch (const std::exception&) {
    // Never forward the input or a UTF-16 diagnostic that could contain it.
    return PromiseRejected(rt, "cribra_native_error:invalid_input_string", jsInvoker_);
  }

  auto scanner = findScanner(static_cast<uint64_t>(scannerId));
  if (!scanner) {
    return PromiseRejected(rt, "cribra_native_error:scanner_not_found", jsInvoker_);
  }

  auto promise = AsyncPromise<cribra::rn::ScanReportDto>(rt, jsInvoker_);
  // Copy, do not move: bridging::toJs below still needs this promise.
  worker_.post([promise,
                scanner = std::move(scanner),
                sourceId = std::move(sourceIdUtf8),
                text = std::move(textUtf8)]() mutable {
    try {
      auto report = cribra::rn::Scan(scanner, sourceId, text);
      promise.resolve(std::move(report));
    } catch (const std::exception& error) {
      promise.reject(Error(error.what()));
    } catch (...) {
      promise.reject(Error("cribra_native_error:unknown"));
    }
  });
  return bridging::toJs(rt, promise, jsInvoker_);
}

jsi::Value CribraReactNative::disposeScanner(
    jsi::Runtime& rt,
    double scannerId) {
  auto promise = AsyncPromise<>(rt, jsInvoker_);
  // Copy, do not move: bridging::toJs below still needs this promise.
  worker_.post([this, promise, scannerId]() mutable {
    auto scanner = takeScanner(static_cast<uint64_t>(scannerId));
    if (!scanner) {
      promise.reject(Error("cribra_native_error:scanner_not_found"));
      return;
    }
    // The handle is released on the worker after any in-flight scan that still
    // holds a shared reference has completed.
    scanner.reset();
    promise.resolve();
  });
  return bridging::toJs(rt, promise, jsInvoker_);
}

double CribraReactNative::getLiveScannerCount(jsi::Runtime& /*rt*/) {
  std::lock_guard<std::mutex> lock(scannersMutex_);
  return static_cast<double>(scanners_.size());
}

CribraReactNative::ScannerPtr CribraReactNative::findScanner(
    uint64_t scannerId) const {
  std::lock_guard<std::mutex> lock(scannersMutex_);
  const auto it = scanners_.find(scannerId);
  if (it == scanners_.end()) {
    return nullptr;
  }
  return it->second;
}

void CribraReactNative::registerScanner(
    uint64_t scannerId,
    ScannerPtr scanner) {
  std::lock_guard<std::mutex> lock(scannersMutex_);
  scanners_.emplace(scannerId, std::move(scanner));
}

CribraReactNative::ScannerPtr CribraReactNative::takeScanner(
    uint64_t scannerId) {
  std::lock_guard<std::mutex> lock(scannersMutex_);
  const auto it = scanners_.find(scannerId);
  if (it == scanners_.end()) {
    return nullptr;
  }
  auto scanner = std::move(it->second);
  scanners_.erase(it);
  return scanner;
}

}  // namespace facebook::react
