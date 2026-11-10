#include "CribraReactNative.h"

#include "CribraAbi.h"

#include <cmath>
#include <optional>
#include <utility>

namespace facebook::react {
namespace {

jsi::Value PromiseRejected(
    jsi::Runtime& rt,
    const std::string& message,
    const std::shared_ptr<CallInvoker>& jsInvoker) {
  auto promise = AsyncPromise<>(rt, jsInvoker);
  promise.reject(Error(message));
  return bridging::toJs(rt, promise, jsInvoker);
}

/// Rejects NaN, infinities, fractions, zero, negatives, and values outside the
/// exactly representable JavaScript integer range before any uint64 cast.
std::optional<uint64_t> LiveScannerId(double value) {
  if (!std::isfinite(value) || value < 1.0 ||
      value > static_cast<double>(cribra::rn::kMaxJsSafeInteger)) {
    return std::nullopt;
  }
  const auto id = static_cast<uint64_t>(value);
  if (static_cast<double>(id) != value) {
    return std::nullopt;
  }
  return id;
}

/// Copies every batch input on the JS thread. Type errors are reported as
/// `invalid_argument`; string conversion errors are reported as
/// `invalid_input_string` without retaining or forwarding the JSI exception
/// text, which may contain fragments of the source.
bool DecodeBatchInputs(
    jsi::Runtime& rt,
    const jsi::Array& sources,
    std::vector<cribra::rn::ScanInput>& inputs,
    std::string& error) {
  size_t count = 0;
  try {
    count = sources.size(rt);
  } catch (...) {
    error = "cribra_native_error:invalid_argument";
    return false;
  }

  inputs.reserve(count);
  for (size_t index = 0; index < count; ++index) {
    std::string id;
    std::string text;
    bool conversionFailed = false;
    try {
      const auto element = sources.getValueAtIndex(rt, index);
      if (!element.isObject()) {
        error = "cribra_native_error:invalid_argument";
        return false;
      }
      const auto object = element.asObject(rt);
      const auto idValue = object.getProperty(rt, "id");
      const auto sourceValue = object.getProperty(rt, "source");
      if (!idValue.isString() || !sourceValue.isString()) {
        error = "cribra_native_error:invalid_argument";
        return false;
      }
      try {
        id = idValue.asString(rt).utf8(rt);
        text = sourceValue.asString(rt).utf8(rt);
      } catch (...) {
        conversionFailed = true;
      }
    } catch (...) {
      error = "cribra_native_error:invalid_argument";
      return false;
    }

    if (conversionFailed) {
      error = "cribra_native_error:invalid_input_string";
      return false;
    }
    inputs.push_back({std::move(id), std::move(text)});
  }
  return true;
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

    if (!cribra::rn::FitsJsSafeInteger(scannerId)) {
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
  } catch (...) {
    // Never forward the input or a UTF-16 diagnostic that could contain it.
    return PromiseRejected(rt, "cribra_native_error:invalid_input_string", jsInvoker_);
  }

  const auto id = LiveScannerId(scannerId);
  if (!id) {
    return PromiseRejected(rt, "cribra_native_error:scanner_not_found", jsInvoker_);
  }
  auto scanner = findScanner(*id);
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
      auto projection = cribra::rn::Scan(scanner, sourceId, text);
      promise.resolve(std::move(projection.report));
    } catch (const std::exception& error) {
      promise.reject(Error(error.what()));
    } catch (...) {
      promise.reject(Error("cribra_native_error:unknown"));
    }
  });
  return bridging::toJs(rt, promise, jsInvoker_);
}

jsi::Value CribraReactNative::scanBatch(
    jsi::Runtime& rt,
    double scannerId,
    jsi::Array sources) {
  std::vector<cribra::rn::ScanInput> inputs;
  std::string inputError;
  if (!DecodeBatchInputs(rt, sources, inputs, inputError)) {
    return PromiseRejected(rt, inputError, jsInvoker_);
  }

  const auto id = LiveScannerId(scannerId);
  if (!id) {
    return PromiseRejected(rt, "cribra_native_error:scanner_not_found", jsInvoker_);
  }
  auto scanner = findScanner(*id);
  if (!scanner) {
    return PromiseRejected(rt, "cribra_native_error:scanner_not_found", jsInvoker_);
  }

  auto promise = AsyncPromise<cribra::rn::ScanBatchDto>(rt, jsInvoker_);
  // One job for the whole ordered batch. Inputs are owned strings already
  // copied on the JS thread; only plain C++ values cross the worker boundary.
  worker_.post([promise,
                scanner = std::move(scanner),
                inputs = std::move(inputs)]() mutable {
    try {
      promise.resolve(cribra::rn::ScanBatch(scanner, inputs));
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
  const auto id = LiveScannerId(scannerId);
  if (!id) {
    return PromiseRejected(rt, "cribra_native_error:scanner_not_found", jsInvoker_);
  }

  auto promise = AsyncPromise<>(rt, jsInvoker_);
  // Copy, do not move: bridging::toJs below still needs this promise.
  worker_.post([this, promise, id = *id]() mutable {
    auto scanner = takeScanner(id);
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
