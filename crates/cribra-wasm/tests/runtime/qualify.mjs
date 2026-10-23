// Build and qualify generated web bindings without browser-specific APIs.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../../../../", import.meta.url));
process.chdir(root);

function run(command, args, options = {}) {
  const result = spawnSync(command, args, { stdio: "inherit", ...options });
  if (result.error) throw result.error;
  assert.equal(result.status, 0, `${command} ${args.join(" ")} failed`);
  return result.stdout;
}

const args = process.argv.slice(2);
const install = args.includes("--install-bindgen");
const variants = args.filter((arg) => arg !== "--install-bindgen");
assert(variants.length <= 1, "Usage: node qualify.mjs [core|flow] [--install-bindgen]");
assert(variants.length === 0 || ["core", "flow"].includes(variants[0]), "Unknown variant");

// Publishing keeps Cargo defaults, so reject a reduced default API independently
// of the explicit features used for the runtime qualification below.
const metadata = JSON.parse(run("cargo", ["metadata", "--locked", "--no-deps", "--format-version", "1"], {
  stdio: ["ignore", "pipe", "inherit"], encoding: "utf8",
}));
const adapter = metadata.packages.find((pkg) => pkg.name === "cribra-wasm");
assert(adapter.features.default.includes("transform"), "Official WASM defaults must enable transform");
assert.deepEqual(adapter.features.transform, ["cribra/transform"]);

const lock = await readFile("Cargo.lock", "utf8");
const bindgen = lock.split("[[package]]").filter((entry) => /^name = "wasm-bindgen"$/m.test(entry));
assert.equal(bindgen.length, 1, "Expected exactly one locked wasm-bindgen version");
const version = bindgen[0].match(/^version = "([^"]+)"$/m)[1];
const installed = spawnSync("wasm-bindgen", ["--version"], { encoding: "utf8" });
if (installed.status !== 0 || installed.stdout.trim() !== `wasm-bindgen ${version}`) {
  assert(install, `Install matching CLI: cargo install wasm-bindgen-cli --version ${version} --locked`);
  run("cargo", ["install", "wasm-bindgen-cli", "--version", version, "--locked", "--force"]);
}
run("wasm-bindgen", ["--version"]);

// Use Core directly for the streaming oracle, independently of adapter DTOs.
const shared = resolve(metadata.target_directory, "wasm-qualification");
await mkdir(shared, { recursive: true });
const streamOracle = run("cargo", ["run", "--locked", "-p", "cribra-wasm", "--no-default-features", "--example", "stream_oracle"], {
  stdio: ["ignore", "pipe", "inherit"], encoding: "utf8",
});
await writeFile(resolve(shared, "stream-oracle.json"), streamOracle);
const incrementalOracle = run("cargo", ["run", "--locked", "-p", "cribra-wasm", "--no-default-features", "--example", "incremental_oracle"], {
  stdio: ["ignore", "pipe", "inherit"], encoding: "utf8",
});
await writeFile(resolve(shared, "incremental-oracle.json"), incrementalOracle);
const batchOracle = run("cargo", ["run", "--locked", "-p", "cribra-wasm", "--no-default-features", "--example", "batch_oracle"], {
  stdio: ["ignore", "pipe", "inherit"], encoding: "utf8",
});
await writeFile(resolve(shared, "batch-oracle.json"), batchOracle);
const configurationOracle = run("cargo", ["run", "--locked", "-p", "cribra-wasm", "--no-default-features", "--example", "configuration_oracle"], {
  stdio: ["ignore", "pipe", "inherit"], encoding: "utf8",
});
await writeFile(resolve(shared, "configuration-oracle.json"), configurationOracle);
const metadataOracle = run("cargo", ["run", "--locked", "-p", "cribra-wasm", "--no-default-features", "--example", "metadata_oracle"], {
  stdio: ["ignore", "pipe", "inherit"], encoding: "utf8",
});
await writeFile(resolve(shared, "metadata-oracle.json"), metadataOracle);
const selected = variants.length ? variants : ["core", "flow"];
if (selected.includes("flow")) {
  const shareOracle = run("cargo", ["run", "--locked", "-p", "cribra-wasm", "--no-default-features", "--features", "transform", "--example", "share_bundle_oracle"], {
    stdio: ["ignore", "pipe", "inherit"], encoding: "utf8",
  });
  await writeFile(resolve(shared, "share-oracle.json"), shareOracle);
}

for (const variant of selected) {
  run("cargo", [
    "build", "-p", "cribra-wasm", "--locked", "--release",
    "--target", "wasm32-unknown-unknown", "--no-default-features",
    ...(variant === "flow" ? ["--features", "transform"] : []),
  ]);
  const output = resolve(metadata.target_directory, "wasm-qualification", variant);
  await mkdir(output, { recursive: true });
  run("wasm-bindgen", [
    "--target", "web", "--out-dir", output, "--out-name", "cribra",
    resolve(metadata.target_directory, "wasm32-unknown-unknown/release/cribra_wasm.wasm"),
  ]);
  await writeFile(resolve(output, "package.json"), '{"type":"module"}\n');
  run(process.execPath, ["crates/cribra-wasm/tests/runtime/smoke.mjs", variant, output]);
}
