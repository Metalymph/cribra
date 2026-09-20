import { readFile } from "node:fs/promises";

import init, {
  FindingConfidence,
  FindingSeverity,
  ScanEngineBuilder,
} from "../../target/wasm-production/cribra.js";

const wasmUrl = new URL(
  "../../target/wasm-production/cribra_bg.wasm",
  import.meta.url,
);

const wasm = await readFile(wasmUrl);
await init({ module_or_path: wasm });

const builder = new ScanEngineBuilder(true);
builder.addFinancialBuiltins();
const engine = builder.build();

const source = [
  "service=checkout",
  "card_number=1234567890123452",
  "iban=IT60X0542811101000000123456",
  "log_level=info",
].join("\n");

const result = engine.scan(source);

try {
  console.log(`classified ${result.findingCount()} finding(s)`);

  for (let index = 0; index < result.findingCount(); index += 1) {
    const finding = result.findingAt(index);

    try {
      console.log(
        [
          finding.ruleId,
          `line=${finding.line}`,
          `column=${finding.column}`,
          `severity=${FindingSeverity[finding.severity]}`,
          `confidence=${FindingConfidence[finding.confidence]}`,
        ].join(" "),
      );
    } finally {
      finding.free();
    }
  }

  console.log("\nSafe derivative:");
  console.log(result.redact(source));
} finally {
  result.free();
  engine.free();
}