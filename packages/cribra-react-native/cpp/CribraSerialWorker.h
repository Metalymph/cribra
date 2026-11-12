#pragma once

#include <condition_variable>
#include <cstddef>
#include <deque>
#include <functional>
#include <memory>
#include <mutex>
#include <thread>
#include <utility>

namespace facebook::react {

/// Outcome of offering one job to the serial worker.
///
/// `Accepted` means the job was stored while the worker was accepting work. It
/// runs exactly once unless shutdown begins before the worker dequeues it, in
/// which case it is discarded without Core work. `Refused` means the worker
/// was already shutting down. `EnqueueFailed` means the job could not be stored
/// (allocation or synchronization failure); it will never run. Callers must
/// settle their promise for both non-accepted outcomes while the JavaScript
/// runtime is still valid.
enum class PostResult { Accepted, Refused, EnqueueFailed };

/// Fixed, source-free diagnostic for a job that was not accepted.
///
/// Returns nullptr for `Accepted`. The messages map to the existing
/// `native_error` public code; no lifecycle-specific public code is added.
constexpr const char* PostFailureMessage(PostResult result) noexcept {
  switch (result) {
    case PostResult::Refused:
      return "cribra_native_error:worker_stopped";
    case PostResult::EnqueueFailed:
      return "cribra_native_error:worker_unavailable";
    case PostResult::Accepted:
      return nullptr;
  }
  return nullptr;
}

/// Serializes JS-side settlement with module shutdown.
///
/// A closed gate prevents new settlement scheduling. It cannot revoke a
/// callback already handed to the React Native runtime; that part of the
/// contract is owned by React Native's weak `CallbackWrapper` invalidation.
struct SettlementGate {
  std::mutex mutex;
  bool closed = false;
};

/// Runs `settle` unless shutdown has closed the gate.
///
/// The mutex is held only for the scheduling call, never for JavaScript
/// execution. `AsyncPromise::resolve`/`reject` enqueue through `CallInvoker`
/// and return, so this cannot deadlock against the JS thread. Exceptions are
/// contained because a job must never unwind through the worker thread.
template <typename F>
void Settle(const std::shared_ptr<SettlementGate>& gate, F&& settle) noexcept {
  if (!gate) {
    return;
  }
  try {
    std::lock_guard<std::mutex> lock(gate->mutex);
    if (gate->closed) {
      return;
    }
    settle();
  } catch (...) {
    // A synchronization or allocation failure must not terminate the worker.
  }
}

/// Settles the promise associated with a job that was never accepted.
///
/// Returns false when the job was accepted and `reject` was not called. This is
/// the only rejection path for `Refused`/`EnqueueFailed`; it is shared by the
/// production TurboModule and the host lifecycle tests so both exercise the
/// same fixed, source-free messages.
template <typename Reject>
bool RejectNotAccepted(PostResult result, Reject&& reject) noexcept {
  if (result == PostResult::Accepted) {
    return false;
  }
  try {
    reject(PostFailureMessage(result));
  } catch (...) {
    // A settlement failure on the JS thread must not escape the module method.
  }
  return true;
}

/// Serial native worker. All Cribra ABI calls happen here. The queue carries
/// owned C++ values and an `AsyncPromise` settlement handle; the worker never
/// dereferences a raw JSI value, and settlement is marshalled through the
/// React Native `CallInvoker` (whose callback wrappers are weak).
///
/// Shutdown is explicit and one-way:
///  * the currently running job is allowed to finish its native work;
///  * queued jobs are destroyed without executing Core operations;
///  * the shutdown hook runs on the worker after queue cleanup;
///  * the thread is always joined, never detached.
///
/// Queue disposal is O(N) in the queued jobs and may zeroize copied sources
/// and release scanner references. It is not constant-time.
class CribraSerialWorker final {
 public:
  /// Allocation-free cleanup callback executed on the worker thread after the
  /// in-flight job and queue disposal. A plain function pointer is used so
  /// shutdown itself cannot fail on `std::function` allocation.
  struct ShutdownHook {
    void (*run)(void*) noexcept;
    void* context;

    constexpr ShutdownHook() noexcept : run(nullptr), context(nullptr) {}
    constexpr ShutdownHook(void (*fn)(void*) noexcept, void* ctx) noexcept
        : run(fn), context(ctx) {}
  };

  CribraSerialWorker();

  ~CribraSerialWorker() noexcept;

  CribraSerialWorker(const CribraSerialWorker&) = delete;
  CribraSerialWorker& operator=(const CribraSerialWorker&) = delete;

  /// Offers one job. Never throws.
  ///
  /// The `std::function` conversion (and any allocation it performs) happens
  /// inside this call, so callers cannot accidentally leave a half-accepted
  /// operation whose promise is never settled.
  template <typename F>
  PostResult post(F&& job) noexcept {
    try {
      std::function<void()> stored(std::forward<F>(job));
      std::lock_guard<std::mutex> lock(mutex_);
      if (stopping_) {
        // `stored` is destroyed here; captured inputs and keys are released
        // before the caller settles its promise.
        return PostResult::Refused;
      }
      queue_.push_back(std::move(stored));
    } catch (...) {
      return PostResult::EnqueueFailed;
    }
    condition_.notify_one();
    return PostResult::Accepted;
  }

  /// Stops accepting jobs, discards the queue on the worker, runs `hook` on the
  /// worker, and joins. Idempotent when called sequentially and never throws.
  ///
  /// The hook is executed at most once, by the worker thread, during the first
  /// stop. A second stop only waits for the already-stopped thread; it never
  /// runs a hook on the calling thread. Must not be called from the worker
  /// thread, and must not be called concurrently from multiple threads: the
  /// join is intentionally unguarded because production has exactly one caller
  /// (the module destructor).
  void stop(ShutdownHook hook = {}) noexcept;

 private:
  void run() noexcept;

  std::mutex mutex_;
  std::condition_variable condition_;
  std::deque<std::function<void()>> queue_;
  ShutdownHook hook_{};
  bool stopping_ = false;
  std::thread thread_;
};

inline CribraSerialWorker::CribraSerialWorker()
    : thread_(&CribraSerialWorker::run, this) {}

inline CribraSerialWorker::~CribraSerialWorker() noexcept { stop(); }

inline void CribraSerialWorker::stop(ShutdownHook hook) noexcept {
  try {
    std::lock_guard<std::mutex> lock(mutex_);
    if (!stopping_) {
      stopping_ = true;
      hook_ = hook;
    }
    // A second call cannot own the hook: the first caller already stored it and
    // the worker either runs it or has finished. Cleanup must never move to the
    // caller's thread.
  } catch (...) {
    // A mutex failure here is not recoverable; the join below still keeps the
    // object from being destroyed with a joinable thread in the normal case.
  }

  condition_.notify_all();
  try {
    if (thread_.joinable()) {
      thread_.join();
    }
  } catch (...) {
    // join is only expected to fail if the thread is not joinable.
  }
}

inline void CribraSerialWorker::run() noexcept {
  try {
    for (;;) {
      std::function<void()> job;
      {
        std::unique_lock<std::mutex> lock(mutex_);
        condition_.wait(lock, [this] { return stopping_ || !queue_.empty(); });
        if (stopping_) {
          break;
        }
        job = std::move(queue_.front());
        queue_.pop_front();
      }
      // Last-resort crash boundary. A job is responsible for settling its own
      // promise; this catch only proves the worker survives an unexpected
      // throw. It is not evidence that the affected promise was settled.
      try {
        job();
      } catch (...) {
      }
    }
  } catch (...) {
    // Fall through to shutdown cleanup if the synchronization primitives fail.
  }

  // Shutdown: destroy queued jobs on this thread without executing Core work,
  // then run the owning module's cleanup hook. Queued lambdas release copied
  // transformation inputs (zeroizing native keys) and scanner references here.
  std::deque<std::function<void()>> discarded;
  ShutdownHook hook{};
  try {
    std::lock_guard<std::mutex> lock(mutex_);
    discarded.swap(queue_);
    hook = hook_;
    hook_ = {};
  } catch (...) {
  }

  discarded.clear();

  if (hook.run != nullptr) {
    try {
      hook.run(hook.context);
    } catch (...) {
    }
  }
}

}  // namespace facebook::react
