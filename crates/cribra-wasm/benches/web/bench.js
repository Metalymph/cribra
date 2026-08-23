const VARIANTS = ["base", "os", "oz", "o3"];
const STARTUP_SAMPLES = 20;
const WORKER_URL = new URL("./bench-worker.js", import.meta.url);

const runButton = document.querySelector("#run");
const status = document.querySelector("#status");
const tables = document.querySelector("#tables");
const json = document.querySelector("#json");

function workerRequest(payload) {
  return new Promise((resolve, reject) => {
    const worker = new Worker(WORKER_URL, { type: "module", name: `cribra-bench-${payload.variant}` });

    const cleanup = () => worker.terminate();

    worker.addEventListener("message", (event) => {
      cleanup();
      if (event.data?.ok) resolve(event.data.value);
      else reject(new Error(event.data?.error ?? "benchmark worker failed"));
    }, { once: true });

    worker.addEventListener("error", (event) => {
      cleanup();
      reject(event.error ?? new Error(event.message));
    }, { once: true });

    worker.postMessage(payload);
  });
}

function stats(samples) {
  const sorted = [...samples].sort((a, b) => a - b);
  const sum = sorted.reduce((a, b) => a + b, 0);
  const quantile = (q) => sorted[Math.min(sorted.length - 1, Math.floor(q * sorted.length))];

  return {
    n: sorted.length,
    min_ms: sorted[0],
    median_ms: quantile(0.5),
    p95_ms: quantile(0.95),
    mean_ms: sum / sorted.length,
    max_ms: sorted.at(-1),
  };
}

function pct(value, baseline) {
  return ((value / baseline) - 1) * 100;
}

function fmt(value, digits = 3) {
  return Number(value).toFixed(digits);
}

function renderRuntime(results) {
  const metrics = Object.keys(results.base.runtime);
  const rows = metrics.map((metric) => {
    const baseline = results.base.runtime[metric];
    const cells = VARIANTS.map((variant) => {
      const current = results[variant].runtime[metric];
      const delta = variant === "base" ? "—" : `${pct(current.median_ms, baseline.median_ms) >= 0 ? "+" : ""}${fmt(pct(current.median_ms, baseline.median_ms), 2)}%`;
      const throughput = current.mib_per_s == null ? "" : ` / ${fmt(current.mib_per_s, 1)} MiB/s`;
      return `<td>${fmt(current.median_ms)} ms${throughput}<br><small>${delta}</small></td>`;
    }).join("");
    return `<tr><td>${metric}</td>${cells}</tr>`;
  }).join("");

  return `
    <h2>Runtime median</h2>
    <table>
      <thead><tr><th>Metric</th>${VARIANTS.map(v => `<th>${v}</th>`).join("")}</tr></thead>
      <tbody>${rows}</tbody>
    </table>`;
}

function renderStartup(results) {
  const rows = ["wasm_init_ms", "worker_to_ready_ms"].map((metric) => {
    const baseline = results.base.startup[metric];
    const cells = VARIANTS.map((variant) => {
      const current = results[variant].startup[metric];
      const delta = variant === "base" ? "—" : `${pct(current.median_ms, baseline.median_ms) >= 0 ? "+" : ""}${fmt(pct(current.median_ms, baseline.median_ms), 2)}%`;
      return `<td>${fmt(current.median_ms)} ms<br><small>p95 ${fmt(current.p95_ms)} / ${delta}</small></td>`;
    }).join("");
    return `<tr><td>${metric}</td>${cells}</tr>`;
  }).join("");

  return `
    <h2>Startup</h2>
    <table>
      <thead><tr><th>Metric</th>${VARIANTS.map(v => `<th>${v}</th>`).join("")}</tr></thead>
      <tbody>${rows}</tbody>
    </table>`;
}

async function run() {
  runButton.disabled = true;
  tables.innerHTML = "";
  json.textContent = "";
  const results = {};

  try {
    for (const variant of VARIANTS) {
      status.textContent = `Startup: ${variant}…`;
      const wasmInit = [];
      const workerReady = [];

      for (let i = 0; i < STARTUP_SAMPLES; i++) {
        const outerStart = performance.now();
        const sample = await workerRequest({ mode: "startup", variant, nonce: i });
        workerReady.push(performance.now() - outerStart);
        wasmInit.push(sample.wasm_init_ms);
      }

      status.textContent = `Runtime: ${variant}…`;
      const runtime = await workerRequest({ mode: "runtime", variant });

      results[variant] = {
        startup: {
          wasm_init_ms: stats(wasmInit),
          worker_to_ready_ms: stats(workerReady),
        },
        runtime,
      };
    }

    tables.innerHTML = renderStartup(results) + renderRuntime(results);
    json.textContent = JSON.stringify({
      generated_at: new Date().toISOString(),
      user_agent: navigator.userAgent,
      startup_samples: STARTUP_SAMPLES,
      results,
    }, null, 2);
    status.textContent = "Complete.";
  } catch (error) {
    status.textContent = `FAILED: ${error.message}`;
    console.error(error);
  } finally {
    runButton.disabled = false;
  }
}

runButton.addEventListener("click", run);
