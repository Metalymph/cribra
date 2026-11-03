import { dirname, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

export const scriptsDir = dirname(fileURLToPath(import.meta.url));
export const packageRoot = resolve(scriptsDir, "..");
export const repoRoot = resolve(packageRoot, "..", "..");

export function isMain(moduleUrl) {
  const entry = process.argv[1];
  if (entry === undefined) return false;
  return pathToFileURL(resolve(entry)).href === moduleUrl;
}

export function parseArgs(argv) {
  const flags = new Map();
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (!argument.startsWith("--")) continue;
    const name = argument.slice(2);
    const next = argv[index + 1];
    if (next !== undefined && !next.startsWith("--")) {
      flags.set(name, next);
      index += 1;
    } else {
      flags.set(name, true);
    }
  }
  return flags;
}

export function fail(message) {
  process.stderr.write(`error: ${message}\n`);
  process.exitCode = 1;
}
