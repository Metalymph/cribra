const VARIANT_URLS = {
  base: "/target/wasm-bench/base/cribra.js",
  os: "/target/wasm-bench/os/cribra.js",
  oz: "/target/wasm-bench/oz/cribra.js",
  o3: "/target/wasm-bench/o3/cribra.js",
};

const MiB = 1024 * 1024;

function median(samples) {
  const sorted = [...samples].sort((a, b) => a - b);
  return sorted[Math.floor(sorted.length / 2)];
}

function p95(samples) {
  const sorted = [...samples].sort((a, b) => a - b);
  return sorted[Math.min(sorted.length - 1, Math.floor(sorted.length * 0.95))];
}

function summarize(samples, bytes = null) {
  const med = median(samples);
  const sum = samples.reduce((a, b) => a + b, 0);
  return {
    n: samples.length,
    median_ms: med,
    p95_ms: p95(samples),
    mean_ms: sum / samples.length,
    min_ms: Math.min(...samples),
    max_ms: Math.max(...samples),
    mib_per_s: bytes == null ? null : (bytes / MiB) / (med / 1000),
  };
}

function consume(value) {
  if (typeof value === "number") return value;
  if (typeof value === "string") return value.length;
  return 1;
}

function measure(iterations, op) {
  const samples = new Array(iterations);
  let sink = 0;

  for (let i = 0; i < iterations; i++) {
    const start = performance.now();
    sink += consume(op());
    samples[i] = performance.now() - start;
  }

  if (sink === Number.MIN_SAFE_INTEGER) console.log(sink);
  return samples;
}

function warmup(iterations, op) {
  let sink = 0;
  for (let i = 0; i < iterations; i++) sink += consume(op());
  if (sink === Number.MIN_SAFE_INTEGER) console.log(sink);
}

async function loadVariant(variant, nonce = "") {
  const moduleUrl = `${VARIANT_URLS[variant]}?bench=${encodeURIComponent(nonce)}`;
  const mod = await import(moduleUrl);

  const initStart = performance.now();
  await mod.default();
  const wasmInitMs = performance.now() - initStart;

  return { mod, wasmInitMs };
}

function scanOnce(engine, source) {
  const result = engine.scan(source);
  const count = result.findingCount() + result.candidateCount();
  result.free();
  return count;
}

function buildFindingFixture(mod) {
  const secret = "CRIBRA_BENCH_SECRET_0123456789ABCDEF";
  const builder = new mod.ScanEngineBuilder(false);
  builder.addLiteral("bench.literal", secret, mod.FindingSeverity.High);
  const engine = builder.build();

  const source = Array.from({ length: 256 }, (_, i) => `row=${i} token=${secret}\n`).join("");
  const result = engine.scan(source);
  return { engine, source, result };
}

async function runtimeBench(variant) {
  const { mod, wasmInitMs } = await loadVariant(variant, "runtime");
  const engine = new mod.ScanEngine();

  const clean64 = "#".repeat(64);
  const clean64k = "#".repeat(64 * 1024);
  const clean1m = "#".repeat(1024 * 1024);

  warmup(200, () => scanOnce(engine, clean64));
  warmup(20, () => scanOnce(engine, clean64k));
  warmup(3, () => scanOnce(engine, clean1m));

  const scan64 = summarize(measure(1000, () => scanOnce(engine, clean64)), clean64.length);
  const scan64k = summarize(measure(120, () => scanOnce(engine, clean64k)), clean64k.length);
  const scan1m = summarize(measure(30, () => scanOnce(engine, clean1m)), clean1m.length);

  const fixture = buildFindingFixture(mod);

  warmup(100, () => {
    let sink = 0;
    const count = fixture.result.findingCount();
    for (let i = 0; i < count; i++) {
      const view = fixture.result.findingAt(i);
      sink += view.start + view.end + view.line + view.column;
      sink += view.ruleId.length;
      view.free();
    }
    return sink;
  });

  const traversal = summarize(measure(300, () => {
    let sink = 0;
    const count = fixture.result.findingCount();
    for (let i = 0; i < count; i++) {
      const view = fixture.result.findingAt(i);
      sink += view.start + view.end + view.line + view.column;
      sink += view.ruleId.length;
      view.free();
    }
    return sink;
  }));

  warmup(50, () => fixture.result.redact(fixture.source));
  const redact = summarize(measure(200, () => fixture.result.redact(fixture.source)), fixture.source.length);

  warmup(50, () => {
    const builder = new mod.ScanEngineBuilder(false);
    builder.addLiteral("bench.literal", "BENCH_LITERAL", mod.FindingSeverity.High);
    builder.addPrefix("bench.prefix", "bench_", mod.FindingSeverity.Medium);
    builder.addSuffix("bench.suffix", "_bench", mod.FindingSeverity.Low);
    builder.addPattern("bench.pattern", "\\bBENCH_[A-Z0-9]{8}\\b", mod.FindingSeverity.High);
    const built = builder.build();
    const n = built.rulesCount();
    built.free();
    return n;
  });

  const builderBuild = summarize(measure(200, () => {
    const builder = new mod.ScanEngineBuilder(false);
    builder.addLiteral("bench.literal", "BENCH_LITERAL", mod.FindingSeverity.High);
    builder.addPrefix("bench.prefix", "bench_", mod.FindingSeverity.Medium);
    builder.addSuffix("bench.suffix", "_bench", mod.FindingSeverity.Low);
    builder.addPattern("bench.pattern", "\\bBENCH_[A-Z0-9]{8}\\b", mod.FindingSeverity.High);
    const built = builder.build();
    const n = built.rulesCount();
    built.free();
    return n;
  }));

  fixture.result.free();
  fixture.engine.free();
  engine.free();

  return {
    wasm_init_ms: wasmInitMs,
    runtime: {
      "scan_64B_clean": scan64,
      "scan_64KiB_clean": scan64k,
      "scan_1MiB_clean": scan1m,
      "finding_traversal_256": traversal,
      "redact_256_findings": redact,
      "custom_builder_4_rules": builderBuild,
    },
  };
}

self.addEventListener("message", async (event) => {
  const { mode, variant, nonce = "" } = event.data ?? {};

  try {
    if (!(variant in VARIANT_URLS)) throw new Error(`unknown variant: ${variant}`);

    if (mode === "startup") {
      const { wasmInitMs } = await loadVariant(variant, `startup-${nonce}-${crypto.randomUUID()}`);
      self.postMessage({ ok: true, value: { wasm_init_ms: wasmInitMs } });
      return;
    }

    if (mode === "runtime") {
      const value = await runtimeBench(variant);
      self.postMessage({ ok: true, value: value.runtime });
      return;
    }

    throw new Error(`unknown benchmark mode: ${mode}`);
  } catch (error) {
    self.postMessage({
      ok: false,
      error: error instanceof Error ? `${error.name}: ${error.message}` : String(error),
    });
  }
});
