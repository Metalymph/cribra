// Host lifecycle tests for the JSI-free native worker and ownership surface.
//
// These tests deliberately avoid JSI, React Native and the generated
// TurboModule spec. They link the real host `libcribra_capi.a` so scanner
// creation, disposal and the C ABI free path are exercised for real.
//
// Synchronization is explicit: condition variables and atomic counters, never
// timing sleeps. Run through `tools/host-tests/run.sh`, which also supports
// ASan, UBSan and TSan builds.

#include "CribraOwnership.h"
#include "CribraSerialWorker.h"

#include <atomic>
#include <chrono>
#include <condition_variable>
#include <cstddef>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <memory>
#include <mutex>
#include <new>
#include <stdexcept>
#include <string>
#include <thread>
#include <unordered_map>
#include <vector>

// ---------------------------------------------------------------------------
// Allocation watch used only by the best-effort transformation-source wipe
// test. Replacing the global allocation functions keeps the freed block alive
// long enough to observe that the destructor zeroed it before deallocation.
// ---------------------------------------------------------------------------

namespace {

std::atomic<void*> gWatchTarget{nullptr};
std::atomic<std::size_t> gWatchSize{0};
std::atomic<int> gWatchSeen{0};
std::atomic<bool> gWatchZeroed{false};

}  // namespace

void* operator new(std::size_t size) {
  if (void* memory = std::malloc(size == 0 ? 1 : size)) {
    return memory;
  }
  throw std::bad_alloc();
}

void* operator new[](std::size_t size) { return ::operator new(size); }

void operator delete(void* memory) noexcept {
  if (memory != nullptr) {
    if (memory == gWatchTarget.load(std::memory_order_acquire)) {
      const auto size = gWatchSize.load(std::memory_order_relaxed);
      const auto* bytes = static_cast<const unsigned char*>(memory);
      bool zeroed = true;
      for (std::size_t index = 0; index < size; ++index) {
        if (bytes[index] != 0) {
          zeroed = false;
          break;
        }
      }
      gWatchZeroed.store(zeroed, std::memory_order_release);
      gWatchSeen.fetch_add(1, std::memory_order_acq_rel);
      gWatchTarget.store(nullptr, std::memory_order_release);
    }
  }
  std::free(memory);
}

void operator delete(void* memory, std::size_t) noexcept { ::operator delete(memory); }
void operator delete[](void* memory) noexcept { ::operator delete(memory); }
void operator delete[](void* memory, std::size_t) noexcept { ::operator delete(memory); }

namespace {

using cribra::rn::KeyBytes;
using cribra::rn::SecureZero;
using cribra::rn::TransformInput;
using facebook::react::CribraSerialWorker;
using facebook::react::PostFailureMessage;
using facebook::react::PostResult;
using facebook::react::RejectNotAccepted;
using ShutdownHook = CribraSerialWorker::ShutdownHook;

std::atomic<int> gFailures{0};
std::atomic<int> gChecks{0};

void Check(bool ok, const char* expression, int line) {
  gChecks.fetch_add(1, std::memory_order_relaxed);
  if (!ok) {
    gFailures.fetch_add(1, std::memory_order_relaxed);
    std::fprintf(stderr, "FAIL line %d: %s\n", line, expression);
  }
}

#define CHECK(expression) Check((expression), #expression, __LINE__)

/// A reusable one-shot or multi-use gate built on a condition variable.
class Gate {
 public:
  void Open() {
    {
      std::lock_guard<std::mutex> lock(mutex_);
      open_ = true;
    }
    condition_.notify_all();
  }

  void Wait() {
    std::unique_lock<std::mutex> lock(mutex_);
    condition_.wait(lock, [this] { return open_; });
  }

  [[nodiscard]] bool IsOpen() const {
    std::lock_guard<std::mutex> lock(mutex_);
    return open_;
  }

 private:
  mutable std::mutex mutex_;
  std::condition_variable condition_;
  bool open_ = false;
};

struct HookProbe {
  std::atomic<int> calls{0};
  std::atomic<int> discardedWhenRun{-1};
  std::atomic<int>* discarded = nullptr;
  std::thread::id thread;
};

void RecordHook(void* context) noexcept {
  auto* probe = static_cast<HookProbe*>(context);
  try {
    probe->thread = std::this_thread::get_id();
    probe->discardedWhenRun.store(
        probe->discarded != nullptr ? probe->discarded->load() : -1);
    probe->calls.fetch_add(1);
  } catch (...) {
  }
}

/// Records the worker's own thread so every later job can prove it ran there.
std::thread::id CaptureWorkerThread(CribraSerialWorker& worker) {
  struct State {
    Gate gate;
    std::thread::id thread;
  } state;
  const auto posted = worker.post([&state] {
    state.thread = std::this_thread::get_id();
    state.gate.Open();
  });
  Check(posted == PostResult::Accepted, "capture worker thread", __LINE__);
  state.gate.Wait();
  return state.thread;
}

// ---------------------------------------------------------------------------
// 1. Shutdown ordering: the running job finishes, the queue is discarded on the
//    worker, the hook observes the discard, and the call joins.
// ---------------------------------------------------------------------------
void TestShutdownOrder() {
  CribraSerialWorker worker;
  const auto workerThread = CaptureWorkerThread(worker);

  Gate runningStarted;
  Gate releaseRunning;
  std::atomic<int> runningCompleted{0};
  std::atomic<int> discarded{0};
  std::atomic<bool> stopReturned{false};
  std::thread::id runningThread;

  struct DiscardProbe {
    explicit DiscardProbe(std::atomic<int>* counter) : discarded(counter) {}
    DiscardProbe(const DiscardProbe&) = delete;
    DiscardProbe& operator=(const DiscardProbe&) = delete;
    ~DiscardProbe() { discarded->fetch_add(1); }

    std::atomic<int>* discarded;
  };

  constexpr int kQueued = 64;

  CHECK(worker.post([&] {
          runningThread = std::this_thread::get_id();
          runningStarted.Open();
          releaseRunning.Wait();
          runningCompleted.fetch_add(1);
        }) == PostResult::Accepted);

  runningStarted.Wait();

  for (int index = 0; index < kQueued; ++index) {
    auto probe = std::make_shared<DiscardProbe>(&discarded);
    // The captured probe must be released on the worker, without running.
    CHECK(worker.post([probe] {}) == PostResult::Accepted);
  }

  HookProbe hook;
  hook.discarded = &discarded;
  std::thread stopper([&] {
    worker.stop(ShutdownHook{&RecordHook, &hook});
    stopReturned.store(true, std::memory_order_release);
  });

  // Deterministic "stop has begun": posts stop being accepted. The probe posts
  // that were accepted before this point are ordinary queued jobs.
  while (worker.post([] {}) != PostResult::Refused) {
    std::this_thread::yield();
  }

  CHECK(!stopReturned.load(std::memory_order_acquire));
  CHECK(runningCompleted.load() == 0);
  CHECK(discarded.load() == 0);

  releaseRunning.Open();
  stopper.join();

  CHECK(runningCompleted.load() == 1);
  CHECK(discarded.load() == kQueued);
  CHECK(stopReturned.load(std::memory_order_acquire));
  CHECK(hook.calls.load() == 1);
  CHECK(hook.thread == workerThread);
  CHECK(hook.thread == runningThread);
  CHECK(hook.discardedWhenRun.load() == kQueued);
  CHECK(worker.post([] {}) == PostResult::Refused);
}

// ---------------------------------------------------------------------------
// 2. Post-after-stop refusal and the fixed diagnostic.
// ---------------------------------------------------------------------------
void TestPostAfterStop() {
  CribraSerialWorker worker;
  worker.stop();

  const auto first = worker.post([] {});
  const auto second = worker.post([] {});
  CHECK(first == PostResult::Refused);
  CHECK(second == PostResult::Refused);

  struct FakePromise {
    int rejections = 0;
    std::string message;
  } promise;

  const bool rejected = RejectNotAccepted(first, [&](const char* message) {
    ++promise.rejections;
    promise.message = message;
  });
  CHECK(rejected);
  CHECK(promise.rejections == 1);
  CHECK(promise.message == "cribra_native_error:worker_stopped");
  CHECK(std::strcmp(PostFailureMessage(first), "cribra_native_error:worker_stopped") == 0);
}

// ---------------------------------------------------------------------------
// 3. Enqueue failure: a real allocation/copy failure is reported, the job never
//    runs, the associated promise is rejected exactly once with the fixed
//    message, and the worker keeps accepting work.
// ---------------------------------------------------------------------------
namespace {

struct ThrowingCopyJob {
  explicit ThrowingCopyJob(std::atomic<int>* counter) : ran(counter) {}
  ThrowingCopyJob(const ThrowingCopyJob&) {
    throw std::runtime_error("synthetic enqueue copy failure");
  }
  ThrowingCopyJob& operator=(const ThrowingCopyJob&) = delete;
  ~ThrowingCopyJob() = default;

  void operator()() const { ran->fetch_add(1); }

  std::atomic<int>* ran;
};

}  // namespace

void TestEnqueueFailure() {
  CribraSerialWorker worker;
  std::atomic<int> ran{0};

  ThrowingCopyJob job(&ran);
  const auto result = worker.post(job);
  CHECK(result == PostResult::EnqueueFailed);

  struct FakePromise {
    int rejections = 0;
    std::string message;
  } promise;

  CHECK(RejectNotAccepted(result, [&](const char* message) {
    ++promise.rejections;
    promise.message = message;
  }));
  CHECK(promise.rejections == 1);
  CHECK(promise.message == "cribra_native_error:worker_unavailable");
  CHECK(PostFailureMessage(result) != nullptr);

  // An accepted job must not be settled by the rejection helper.
  CHECK(!RejectNotAccepted(PostResult::Accepted, [&](const char*) {
    Check(false, "accepted job must not be rejected", __LINE__);
  }));

  // The worker survives the failed enqueue and still runs later jobs.
  Gate done;
  CHECK(worker.post([&done] { done.Open(); }) == PostResult::Accepted);
  done.Wait();
  CHECK(ran.load() == 0);
}

// ---------------------------------------------------------------------------
// 4. A throwing job does not terminate the worker and later jobs still run.
//    The worker-level catch is a crash boundary, not a settlement guarantee.
// ---------------------------------------------------------------------------
void TestJobExceptionRecovery() {
  CribraSerialWorker worker;

  CHECK(worker.post([] { throw std::runtime_error("synthetic job failure"); }) ==
        PostResult::Accepted);
  CHECK(worker.post([] { throw 7; }) == PostResult::Accepted);

  Gate done;
  std::atomic<int> executed{0};
  CHECK(worker.post([&] {
          executed.fetch_add(1);
          done.Open();
        }) == PostResult::Accepted);
  done.Wait();
  CHECK(executed.load() == 1);

  // Still healthy after the recovery.
  Gate second;
  CHECK(worker.post([&second] { second.Open(); }) == PostResult::Accepted);
  second.Wait();
}

// ---------------------------------------------------------------------------
// 5. Registry cleanup runs on the worker after the queue is discarded, and real
//    C ABI scanner handles are released there.
// ---------------------------------------------------------------------------
namespace {

struct Registry {
  std::mutex mutex;
  std::unordered_map<uint64_t, std::shared_ptr<CribraScanner>> scanners;
};

struct RegistryState {
  Registry registry;
  std::atomic<int> created{0};
  std::atomic<int> freed{0};
  std::atomic<int> offThreadFrees{0};
  std::thread::id workerThread;
  std::atomic<int> hookCalls{0};
  std::thread::id hookThread;
  std::atomic<bool> registryEmptyAfterCleanup{false};
};

void CleanupRegistry(void* context) noexcept {
  auto* state = static_cast<RegistryState*>(context);
  try {
    std::unordered_map<uint64_t, std::shared_ptr<CribraScanner>> dropped;
    {
      std::lock_guard<std::mutex> lock(state->registry.mutex);
      dropped.swap(state->registry.scanners);
    }
    // Last references are released here, on the worker.
    dropped.clear();
    state->registryEmptyAfterCleanup.store(state->registry.scanners.empty());
    state->hookThread = std::this_thread::get_id();
    state->hookCalls.fetch_add(1);
  } catch (...) {
  }
}

}  // namespace

void TestRegistryCleanupOnWorker() {
  CribraSerialWorker worker;
  RegistryState state;
  state.workerThread = CaptureWorkerThread(worker);

  constexpr int kScanners = 200;
  Gate created;
  std::atomic<int> registered{0};

  for (int index = 0; index < kScanners; ++index) {
    CHECK(worker.post([&state, index] {
      CribraScanner* scanner = nullptr;
      const auto status = cribra_scanner_new_current(&scanner);
      Check(status == CRIBRA_OK && scanner != nullptr, "scanner creation", __LINE__);
      if (scanner == nullptr) {
        return;
      }
      state.created.fetch_add(1);
      std::shared_ptr<CribraScanner> handle(
          scanner, [&state](CribraScanner* owned) {
            if (std::this_thread::get_id() != state.workerThread) {
              state.offThreadFrees.fetch_add(1);
            }
            cribra_scanner_free(owned);
            state.freed.fetch_add(1);
          });
      {
        std::lock_guard<std::mutex> lock(state.registry.mutex);
        state.registry.scanners.emplace(static_cast<uint64_t>(index) + 1, std::move(handle));
      }
    }) == PostResult::Accepted);
  }

  CHECK(worker.post([&] {
          {
            std::lock_guard<std::mutex> lock(state.registry.mutex);
            registered.store(static_cast<int>(state.registry.scanners.size()));
          }
          created.Open();
        }) == PostResult::Accepted);
  created.Wait();
  CHECK(registered.load() == kScanners);

  worker.stop(ShutdownHook{&CleanupRegistry, &state});

  CHECK(state.hookCalls.load() == 1);
  CHECK(state.hookThread == state.workerThread);
  CHECK(state.registryEmptyAfterCleanup.load());
  CHECK(state.created.load() == kScanners);
  CHECK(state.freed.load() == kScanners);
  CHECK(state.offThreadFrees.load() == 0);

  // A stopped worker cannot resurrect work.
  CHECK(worker.post([&state] { state.created.fetch_add(1); }) == PostResult::Refused);
  CHECK(state.created.load() == kScanners);
}

// ---------------------------------------------------------------------------
// 6. Zeroization of sensitive native buffers.
// ---------------------------------------------------------------------------
void TestSecureZeroAndKeyBytes() {
  unsigned char buffer[64];
  std::memset(buffer, 0xA5, sizeof(buffer));
  SecureZero(buffer, sizeof(buffer));
  bool allZero = true;
  for (const auto byte : buffer) {
    if (byte != 0) {
      allZero = false;
    }
  }
  CHECK(allZero);
  SecureZero(nullptr, 0);  // no-op, must not crash

  alignas(KeyBytes) unsigned char slab[sizeof(KeyBytes)];
  auto* key = new (slab) KeyBytes();
  for (auto& byte : key->bytes) {
    byte = 0xC3;
  }
  key->~KeyBytes();
  bool keyZeroed = true;
  for (std::size_t index = 0; index < sizeof(KeyBytes); ++index) {
    if (slab[index] != 0) {
      keyZeroed = false;
    }
  }
  CHECK(keyZeroed);
}

void TestQueuedKeyDiscard() {
  CribraSerialWorker worker;
  const auto workerThread = CaptureWorkerThread(worker);

  alignas(KeyBytes) unsigned char slab[sizeof(KeyBytes)];
  auto* key = new (slab) KeyBytes();
  for (auto& byte : key->bytes) {
    byte = 0x5A;
  }

  struct DiscardState {
    std::atomic<int> destroyed{0};
    std::atomic<int> ran{0};
    std::atomic<bool> zeroedOnWorker{false};
    std::thread::id thread;
  } state;

  auto shared = std::shared_ptr<KeyBytes>(key, [&state, &slab](KeyBytes* owned) {
    state.thread = std::this_thread::get_id();
    owned->~KeyBytes();
    bool zeroed = true;
    for (std::size_t index = 0; index < sizeof(KeyBytes); ++index) {
      if (slab[index] != 0) {
        zeroed = false;
      }
    }
    state.zeroedOnWorker.store(zeroed);
    state.destroyed.fetch_add(1);
  });

  // Park the worker so the key job stays queued and cannot run before shutdown.
  Gate started;
  Gate release;
  CHECK(worker.post([&] {
          started.Open();
          release.Wait();
        }) == PostResult::Accepted);
  started.Wait();

  CHECK(worker.post([shared, &state] { state.ran.fetch_add(1); }) ==
        PostResult::Accepted);
  std::weak_ptr<KeyBytes> observer = shared;
  // The queued job now owns the only strong reference.
  shared.reset();

  std::thread stopper([&] { worker.stop(); });
  while (worker.post([] {}) != PostResult::Refused) {
    std::this_thread::yield();
  }
  release.Open();
  stopper.join();

  CHECK(observer.expired());
  CHECK(state.ran.load() == 0);
  CHECK(state.destroyed.load() == 1);
  CHECK(state.thread == workerThread);
  CHECK(state.zeroedOnWorker.load());
}

void TestTransformSourceWipe() {
  auto input = std::make_unique<TransformInput>();
  input->text = std::string(96, 'x');
  input->option = std::string(96, 'y');

  const auto* data = input->text.data();
  const auto size = input->text.size();
  CHECK(size == 96);

  gWatchSize.store(size);
  gWatchZeroed.store(false);
  gWatchSeen.store(0);
  gWatchTarget.store(const_cast<void*>(static_cast<const void*>(data)));
  input.reset();

  CHECK(gWatchSeen.load() == 1);
  CHECK(gWatchZeroed.load());
  gWatchTarget.store(nullptr);
}

// ---------------------------------------------------------------------------
// 7. 10,000-job burst in FIFO order without a single loss.
// ---------------------------------------------------------------------------
void TestBurst() {
  CribraSerialWorker worker;
  const auto workerThread = CaptureWorkerThread(worker);

  constexpr int kJobs = 10000;
  std::mutex mutex;
  std::condition_variable condition;
  std::atomic<int> completed{0};
  std::atomic<bool> offThread{false};

  const auto start = std::chrono::steady_clock::now();
  for (int index = 0; index < kJobs; ++index) {
    CHECK(worker.post([&] {
            if (std::this_thread::get_id() != workerThread) {
              offThread.store(true);
            }
            const auto done = completed.fetch_add(1) + 1;
            if (done == kJobs) {
              // Notify under the waiter's mutex so the final transition cannot
              // be lost between its predicate check and its sleep.
              std::lock_guard<std::mutex> lock(mutex);
              condition.notify_all();
            }
          }) == PostResult::Accepted);
  }

  {
    std::unique_lock<std::mutex> lock(mutex);
    condition.wait(lock, [&] { return completed.load() == kJobs; });
  }
  const auto elapsed = std::chrono::steady_clock::now() - start;

  CHECK(completed.load() == kJobs);
  CHECK(!offThread.load());
  std::printf(
      "burst: %d jobs completed in %.1f ms\n",
      kJobs,
      std::chrono::duration<double, std::milli>(elapsed).count());
}

// ---------------------------------------------------------------------------
// 8. Repeated real scanner creation/disposal on the worker.
// ---------------------------------------------------------------------------
void TestRepeatedCreateDispose() {
  CribraSerialWorker worker;
  const auto workerThread = CaptureWorkerThread(worker);

  constexpr int kRounds = 200;
  std::mutex mutex;
  std::condition_variable condition;
  std::atomic<int> disposed{0};
  std::atomic<int> created{0};
  std::atomic<int> freed{0};
  std::atomic<bool> offThread{false};

  for (int index = 0; index < kRounds; ++index) {
    CHECK(worker.post([&] {
            try {
              if (std::this_thread::get_id() != workerThread) {
                offThread.store(true);
              }
              CribraScanner* scanner = nullptr;
              if (cribra_scanner_new_current(&scanner) != CRIBRA_OK ||
                  scanner == nullptr) {
                Check(false, "repeated scanner creation", __LINE__);
                return;
              }
              created.fetch_add(1);
              std::shared_ptr<CribraScanner> handle(
                  scanner, [&](CribraScanner* owned) {
                    if (std::this_thread::get_id() != workerThread) {
                      offThread.store(true);
                    }
                    cribra_scanner_free(owned);
                    freed.fetch_add(1);
                  });
              handle.reset();
              const auto done = disposed.fetch_add(1) + 1;
              if (done == kRounds) {
                std::lock_guard<std::mutex> lock(mutex);
                condition.notify_all();
              }
            } catch (...) {
              Check(false, "repeated create/dispose threw", __LINE__);
            }
          }) == PostResult::Accepted);
  }

  {
    std::unique_lock<std::mutex> lock(mutex);
    condition.wait(lock, [&] { return disposed.load() == kRounds; });
  }

  CHECK(created.load() == kRounds);
  CHECK(freed.load() == kRounds);
  CHECK(!offThread.load());
}

// ---------------------------------------------------------------------------
// 9. Concurrent post/stop. The TSan build is the meaningful qualification here;
//    the functional assertions prove the worker never runs work off-thread and
//    never accepts work after stop returned.
// ---------------------------------------------------------------------------
void TestConcurrentPostStop() {
  CribraSerialWorker worker;
  const auto workerThread = CaptureWorkerThread(worker);

  std::atomic<int> executed{0};
  std::atomic<bool> offThread{false};
  std::atomic<bool> go{false};
  std::atomic<bool> stopped{false};

  auto poster = [&](int count) {
    while (!go.load(std::memory_order_acquire)) {
      std::this_thread::yield();
    }
    for (int index = 0; index < count; ++index) {
      if (worker.post([&] {
            if (std::this_thread::get_id() != workerThread) {
              offThread.store(true);
            }
            executed.fetch_add(1);
          }) != PostResult::Accepted &&
          stopped.load(std::memory_order_acquire)) {
        break;
      }
    }
  };

  std::vector<std::thread> posters;
  posters.reserve(4);
  for (int index = 0; index < 4; ++index) {
    posters.emplace_back(poster, 500);
  }
  go.store(true, std::memory_order_release);

  while (executed.load() == 0) {
    std::this_thread::yield();
  }

  worker.stop();
  stopped.store(true, std::memory_order_release);
  for (auto& thread : posters) {
    thread.join();
  }

  CHECK(!offThread.load());
  const auto snapshot = executed.load();
  CHECK(worker.post([&] { executed.fetch_add(1); }) == PostResult::Refused);
  CHECK(executed.load() == snapshot);
}

// ---------------------------------------------------------------------------
// 10. Double stop is idempotent and never moves the hook to the caller thread.
// ---------------------------------------------------------------------------
void TestDoubleStop() {
  CribraSerialWorker worker;
  std::atomic<int> discarded{0};
  HookProbe hook;
  hook.discarded = &discarded;

  CHECK(worker.post([] {}) == PostResult::Accepted);
  worker.stop(ShutdownHook{&RecordHook, &hook});
  CHECK(hook.calls.load() == 1);

  worker.stop(ShutdownHook{&RecordHook, &hook});
  CHECK(hook.calls.load() == 1);
}

// ---------------------------------------------------------------------------
// 11. Shutdown latency and queue-disposal accounting.
// ---------------------------------------------------------------------------
void TestShutdownMeasurement() {
  constexpr int kQueued = 5000;

  CribraSerialWorker idle;
  auto start = std::chrono::steady_clock::now();
  idle.stop();
  const auto idleLatency = std::chrono::steady_clock::now() - start;

  CribraSerialWorker loaded;
  std::atomic<int> queuedExecuted{0};

  // Keep the worker parked so every posted job is still queued when shutdown
  // starts. The blocked job performs no core work and is released as soon as
  // the worker has stopped accepting jobs.
  Gate started;
  Gate release;
  CHECK(loaded.post([&] {
          started.Open();
          release.Wait();
        }) == PostResult::Accepted);
  started.Wait();

  for (int index = 0; index < kQueued; ++index) {
    auto input = std::make_shared<TransformInput>();
    input->text = std::string(4096, 'z');
    input->key = std::make_shared<KeyBytes>();
    for (auto& byte : input->key->bytes) {
      byte = 0x77;
    }
    CHECK(loaded.post([input, &queuedExecuted] { queuedExecuted.fetch_add(1); }) ==
          PostResult::Accepted);
  }

  start = std::chrono::steady_clock::now();
  std::thread stopper([&] { loaded.stop(); });
  while (loaded.post([] {}) != PostResult::Refused) {
    std::this_thread::yield();
  }
  release.Open();
  stopper.join();
  const auto discardLatency = std::chrono::steady_clock::now() - start;

  CHECK(queuedExecuted.load() == 0);

  std::printf(
      "shutdown: idle %.3f ms; in-flight+%d queued transform jobs released in %.3f ms\n",
      std::chrono::duration<double, std::milli>(idleLatency).count(),
      kQueued,
      std::chrono::duration<double, std::milli>(discardLatency).count());
}

}  // namespace

int main() {
  std::printf("cribra host lifecycle tests\n");

  TestShutdownOrder();
  TestPostAfterStop();
  TestEnqueueFailure();
  TestJobExceptionRecovery();
  TestRegistryCleanupOnWorker();
  TestSecureZeroAndKeyBytes();
  TestQueuedKeyDiscard();
  TestTransformSourceWipe();
  TestBurst();
  TestRepeatedCreateDispose();
  TestConcurrentPostStop();
  TestDoubleStop();
  TestShutdownMeasurement();

  const auto failures = gFailures.load();
  std::printf(
      "host lifecycle tests: %s (%d checks, %d failures)\n",
      failures == 0 ? "PASS" : "FAIL",
      gChecks.load(),
      failures);
  return failures == 0 ? 0 : 1;
}
