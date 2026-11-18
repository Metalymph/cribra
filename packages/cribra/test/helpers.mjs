import { createRequire } from "node:module";
import { dirname, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const require = createRequire(import.meta.url);

export const distDir = resolve(
  process.env.CRIBRA_PACKAGE_DIST ?? resolve(here, "..", "dist"),
);
export const profile = process.env.CRIBRA_PROFILE ?? "flow";

export const packageVersion = require("../package.json").version;
export const githubToken = "ghp_AbCdEf0123456789_AbCdEf0123456789";
export const candidateSource = "ABCD-EFGH-IJKL-MNOP";
export const privacySentinel = "SCANNED_SECRET_DO_NOT_EXPOSE";

export async function loadPackage() {
  return import(pathToFileURL(resolve(distDir, "index.node.js")).href);
}

export const customRules = [
  {
    id: "test.literal",
    kind: "literal",
    value: "custom-secret-123",
    severity: "high",
    remediation: "rotate_credential",
  },
  {
    id: "test.prefix",
    kind: "prefix",
    value: "PRE_",
    severity: "medium",
  },
  {
    id: "test.suffix",
    kind: "suffix",
    value: "_END",
    severity: "low",
    remediation: null,
  },
  {
    id: "test.pattern",
    kind: "pattern",
    pattern: "token-[0-9]{4}",
    severity: "critical",
    remediation: "remove_sensitive_value",
  },
];

export const customSource =
  "custom-secret-123\nPRE_beta\nalpha_END\ntoken-1234";

export function customScannerOptions() {
  return { builtins: "none", rules: customRules };
}

export function occurrenceSpan(source, needle) {
  const index = source.indexOf(needle);
  if (index < 0) throw new Error(`fixture does not contain ${needle}`);
  const start = new TextEncoder().encode(source.slice(0, index)).length;
  return { start, end: start + new TextEncoder().encode(needle).length };
}

export function plainData(value, path = "root") {
  if (value === null) return;
  const type = typeof value;
  if (
    type === "string" ||
    type === "number" ||
    type === "boolean" ||
    type === "bigint"
  ) {
    return;
  }
  if (type === "undefined") {
    throw new Error(`undefined at ${path}`);
  }
  if (Array.isArray(value)) {
    if (Object.getPrototypeOf(value) !== Array.prototype) {
      throw new Error(`non-plain array at ${path}`);
    }
    value.forEach((entry, index) => plainData(entry, `${path}[${index}]`));
    return;
  }
  if (type !== "object") {
    throw new Error(`non-plain value ${type} at ${path}`);
  }
  if (Object.getPrototypeOf(value) !== Object.prototype) {
    throw new Error(`non-plain object at ${path}`);
  }
  for (const [key, entry] of Object.entries(value)) {
    if (key === "free" || typeof entry === "function") {
      throw new Error(`native handle or function at ${path}.${key}`);
    }
    plainData(entry, `${path}.${key}`);
  }
}
