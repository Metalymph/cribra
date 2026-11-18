/** Tiny `--flag value` parser for qualification scripts. */

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

export function requiredFlag(flags, name) {
  const value = flags.get(name);
  if (typeof value !== "string" || value.length === 0) {
    throw new Error(`missing required --${name}`);
  }
  return value;
}

export function flagString(flags, name, fallback) {
  const value = flags.get(name);
  if (value === undefined) return fallback;
  if (typeof value !== "string") throw new Error(`--${name} requires a value`);
  return value;
}
