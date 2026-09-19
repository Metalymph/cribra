import init, {
  FindingConfidence,
  FindingSeverity,
  ScanEngineBuilder,
} from "../../target/wasm-production/cribra.js";

await init();

const sourceElement = document.querySelector("#source");
const scanButton = document.querySelector("#scan");
const findingsElement = document.querySelector("#findings");
const redactedElement = document.querySelector("#redacted");

const builder = new ScanEngineBuilder(true);
builder.addFinancialBuiltins();

const engine = builder.build();

scanButton.addEventListener("click", () => {
  const source = sourceElement.value;
  const result = engine.scan(source);

  try {
    const findings = [];

    for (let index = 0; index < result.findingCount(); index += 1) {
      const finding = result.findingAt(index);

      try {
        findings.push(
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

    findingsElement.textContent =
      findings.length === 0 ? "No classified findings." : findings.join("\n");

    redactedElement.textContent = result.redact(source);
  } finally {
    result.free();
  }
});