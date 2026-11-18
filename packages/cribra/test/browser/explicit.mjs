/* Explicit WASM URL/bytes initialization from the installed browser entry. */

const params = new URLSearchParams(location.search);
const profile = params.get("profile");
const version = params.get("version");
const mode = params.get("mode");
const result = document.getElementById("result");

function equal(actual, expected, message) {
  if (!Object.is(actual, expected)) {
    throw new Error(`${message}: expected ${String(expected)}, got ${String(actual)}`);
  }
}

async function main() {
  if (mode !== "url" && mode !== "bytes") {
    throw new Error(`unknown explicit asset mode: ${mode}`);
  }
  // A query parameter creates a separate module instance and therefore a fresh
  // initialization lifecycle for this page.
  const api = await import(`/dist/index.browser.js?explicit=${mode}`);
  equal(api.isInitialized(), false, "not initialized at import");

  const wasmUrl = new URL("/dist/wasm/cribra_bg.wasm", location.href);
  let wasm;
  if (mode === "url") {
    wasm = wasmUrl;
  } else {
    const response = await fetch(wasmUrl);
    if (!response.ok) throw new Error(`failed to fetch WASM: ${response.status}`);
    wasm = new Uint8Array(await response.arrayBuffer());
  }

  await Promise.all([api.init({ wasm }), api.init({ wasm })]);
  equal(api.isInitialized(), true, "initialized");
  equal(api.getCapabilities().profile, profile, "profile");
  equal(api.getVersion().package, version, "package version");

  const scanner = api.createScanner({
    builtins: "none",
    rules: [
      {
        id: "explicit.asset",
        kind: "literal",
        value: "explicit-secret-123",
        severity: "high",
      },
    ],
  });
  const scan = scanner.scan("value=explicit-secret-123");
  equal(scan.findingCount, 1, "explicit asset scan");
  equal(scan.findings[0].ruleId, "explicit.asset", "explicit asset rule");
  scanner.dispose();
  result.textContent = "PASS";
}

main().catch((error) => {
  const message = error && error.message ? error.message : String(error);
  const code = error && error.code ? ` [${error.code}]` : "";
  result.textContent = `FAIL: ${message}${code}`;
});
