// TypeScript SDK interoperability qualification for the real cribra-mcp binary.
//
// Uses the official @modelcontextprotocol/client (pinned 2.4.0) and its stdio
// transport. Nothing here hand-crafts JSON-RPC: every request goes through the
// published client API, and every tool result is additionally validated against
// the tool's advertised output schema with AJV (draft 2020-12).
//
// Run with: CRIBRA_MCP_BIN=/abs/path/to/cribra-mcp node run.mjs

import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";
import Ajv2020 from "ajv/dist/2020.js";
import { Client } from "@modelcontextprotocol/client";
import { StdioClientTransport } from "@modelcontextprotocol/client/stdio";

const HERE = dirname(fileURLToPath(import.meta.url));
const BIN =
  process.env.CRIBRA_MCP_BIN || resolve(HERE, "../../../target/release/cribra-mcp");

const SCHEMAS = {
  cribra_scan: JSON.parse(
    readFileSync(resolve(HERE, "../../schemas/cribra_scan.output.json"), "utf8"),
  ),
  cribra_transform: JSON.parse(
    readFileSync(resolve(HERE, "../../schemas/cribra_transform.output.json"), "utf8"),
  ),
};

const ajv = new Ajv2020({ allErrors: true, strict: false });
const validators = {
  cribra_scan: ajv.compile(SCHEMAS.cribra_scan),
  cribra_transform: ajv.compile(SCHEMAS.cribra_transform),
};

// Synthetic secret. Not a real credential.
const SECRET = "ghp_syntheticQualificationToken0000000000";

const failures = [];
const scenarios = [];

function check(condition, message) {
  if (!condition) throw new Error(message);
}

function assertSchema(tool, result) {
  const structured = result.structuredContent;
  check(structured !== undefined, `${tool}: structuredContent is missing`);
  const valid = validators[tool](structured);
  if (!valid) {
    throw new Error(
      `${tool}: structuredContent violates the advertised output schema: ` +
        JSON.stringify(validators[tool].errors),
    );
  }
  check(result.content?.[0]?.type === "text", `${tool}: first content block is not text`);
}

function withDeadline(promise, milliseconds, label) {
  let timer;
  const deadline = new Promise((_, reject) => {
    timer = setTimeout(() => reject(new Error(`${label} did not settle in ${milliseconds}ms`)), milliseconds);
  });
  return Promise.race([promise, deadline]).finally(() => clearTimeout(timer));
}

function newClient(mode, label) {
  const transport = new StdioClientTransport({
    command: BIN,
    args: [],
    stderr: "pipe",
  });
  const client = new Client(
    { name: `cribra-qualification-${label}`, version: "1.0.0" },
    { capabilities: {}, versionNegotiation: { mode } },
  );
  return { client, transport };
}

async function scenario(name, body) {
  const started = Date.now();
  try {
    const details = await body();
    scenarios.push({ scenario: name, status: "pass", durationMs: Date.now() - started, details });
    console.log(`PASS ${name}`);
  } catch (error) {
    failures.push({ scenario: name, error: error.message });
    scenarios.push({ scenario: name, status: "fail", durationMs: Date.now() - started, error: error.message });
    console.error(`FAIL ${name}: ${error.message}`);
  }
}

await scenario("modern-pin-discovery-and-tools", async () => {
  const { client, transport } = newClient({ pin: "2026-07-28" }, "modern");

  // `pin` performs a connect-time server/discover and fails loudly if the
  // server does not offer the pinned revision.
  await client.connect(transport);
  const era = client.getProtocolEra();
  const negotiated = client.getNegotiatedProtocolVersion();
  check(negotiated === "2026-07-28", `negotiated ${negotiated}, expected 2026-07-28`);
  check(era === "modern", `era ${era}, expected modern`);

  const discover = await client.discover();
  check(
    Array.isArray(discover.supportedVersions) && discover.supportedVersions.includes("2026-07-28"),
    `supportedVersions ${JSON.stringify(discover.supportedVersions)}`,
  );
  check(
    discover.capabilities?.tools !== undefined,
    "optimistic: server must advertise the tools capability",
  );
  for (const capability of ["prompts", "resources", "logging", "sampling", "elicitation"]) {
    check(
      discover.capabilities[capability] === undefined,
      `unadvertised capability ${capability} must not appear`,
    );
  }

  const tools = await client.listTools();
  const names = tools.tools.map((tool) => tool.name).sort();
  check(
    JSON.stringify(names) === JSON.stringify(["cribra_scan", "cribra_transform"]),
    `advertised tools ${JSON.stringify(names)}`,
  );
  for (const tool of tools.tools) {
    check(typeof tool.description === "string" && tool.description.length > 0, `${tool.name}: description`);
    check(tool.inputSchema?.type === "object", `${tool.name}: inputSchema`);
    check(tool.outputSchema?.type === "object", `${tool.name}: outputSchema`);
  }

  const scan = await client.callTool({
    name: "cribra_scan",
    arguments: { source: `before ${SECRET} after` },
  });
  check(scan.isError === false, "scan isError");
  assertSchema("cribra_scan", scan);
  check(scan.structuredContent.findings.length === 1, "scan finding count");
  check(!JSON.stringify(scan).includes(SECRET), "scan leaked the matched secret");

  const scanError = await client.callTool({
    name: "cribra_scan",
    arguments: { source: "x", unexpectedField: true },
  });
  check(scanError.isError === true, "invalid arguments must be a tool error");
  assertSchema("cribra_scan", scanError);
  check(
    scanError.structuredContent.error.code === "invalid_arguments",
    `error code ${scanError.structuredContent.error.code}`,
  );

  const redacted = await client.callTool({
    name: "cribra_transform",
    arguments: {
      source: `before ${SECRET} after`,
      mode: "redact",
      options: { replacement: "[x]" },
    },
  });
  check(redacted.isError === false, "redact isError");
  assertSchema("cribra_transform", redacted);
  check(redacted.structuredContent.output === "before [x] after", `redact output ${redacted.structuredContent.output}`);
  check(redacted.content[0].text === redacted.structuredContent.output, "redact text/structured mismatch");

  const templated = await client.callTool({
    name: "cribra_transform",
    arguments: {
      source: `before ${SECRET} after`,
      mode: "template",
      rules: [{ id: "custom.synthetic", pattern: "ghp_[A-Za-z0-9]+", severity: "high" }],
      options: { namespace: "probe" },
    },
  });
  check(templated.isError === false, "template isError");
  assertSchema("cribra_transform", templated);
  check(!templated.structuredContent.output.includes(SECRET), "template leaked the secret");
  check(templated.content[0].text === templated.structuredContent.output, "template text/structured mismatch");

  const pid = transport.pid;
  await client.close();

  return { negotiated, era, tools: names, pid };
});

await scenario("auto-negotiation-selects-modern", async () => {
  const { client, transport } = newClient("auto", "auto");
  await client.connect(transport);
  const negotiated = client.getNegotiatedProtocolVersion();
  const era = client.getProtocolEra();
  // `auto` probes with server/discover; this server answers it, so the modern
  // era is selected. The legacy FALLBACK path is therefore not exercised here
  // and is not claimed.
  check(negotiated === "2026-07-28", `auto negotiated ${negotiated}`);
  check(era === "modern", `auto era ${era}`);
  const tools = await client.listTools();
  check(tools.tools.length === 2, "auto tools/list");
  const scan = await client.callTool({ name: "cribra_scan", arguments: { source: "hello" } });
  check(scan.isError === false, "auto scan");
  assertSchema("cribra_scan", scan);
  await client.close();
  return { negotiated, era };
});

await scenario("legacy-handshake-and-tools", async () => {
  const { client, transport } = newClient("legacy", "legacy");
  await client.connect(transport);
  const negotiated = client.getNegotiatedProtocolVersion();
  const era = client.getProtocolEra();
  check(negotiated === "2025-11-25", `legacy negotiated ${negotiated}`);
  check(era === "legacy", `legacy era ${era}`);

  const tools = await client.listTools();
  check(tools.tools.length === 2, "legacy tools/list");
  const scan = await client.callTool({ name: "cribra_scan", arguments: { source: "hello" } });
  check(scan.isError === false, "legacy scan");
  assertSchema("cribra_scan", scan);
  const transform = await client.callTool({
    name: "cribra_transform",
    arguments: { source: "hello", mode: "redact" },
  });
  check(transform.isError === false, "legacy transform");
  assertSchema("cribra_transform", transform);
  await client.close();
  return { negotiated, era };
});

await scenario("unexpected-server-exit-surfaces-as-error", async () => {
  const { client, transport } = newClient({ pin: "2026-07-28" }, "server-death");
  await client.connect(transport);
  const pid = transport.pid;
  check(typeof pid === "number", "transport did not expose a pid");
  process.kill(pid, "SIGKILL");

  let rejected = false;
  try {
    await withDeadline(client.callTool({ name: "cribra_scan", arguments: { source: "x" } }), 15000, "callTool after server death");
  } catch {
    rejected = true;
  } finally {
    await client.close().catch(() => {});
  }
  check(rejected, "a tool call after an unexpected server exit must reject, not hang");
  return { pid };
});

await scenario("clean-close-terminates-the-child", async () => {
  const { client, transport } = newClient({ pin: "2026-07-28" }, "close");
  await client.connect(transport);
  const pid = transport.pid;
  await client.close();

  // After close, further calls must fail fast rather than hang. `listTools` may
  // be served from the SDK's response cache, so the non-cacheable tool call is
  // the meaningful probe.
  let rejected = false;
  try {
    await withDeadline(
      client.callTool({ name: "cribra_scan", arguments: { source: "x" } }),
      5000,
      "callTool after close",
    );
  } catch {
    rejected = true;
  }
  check(rejected, "a call after close must reject");

  // The child must be gone.
  let alive = true;
  try {
    process.kill(pid, 0);
  } catch {
    alive = false;
  }
  check(!alive, `child process ${pid} survived client.close()`);
  return { pid };
});

const summary = {
  client: "@modelcontextprotocol/client",
  version: "2.4.0",
  node: process.version,
  binary: BIN,
  scenarios,
  failures,
};
console.log(JSON.stringify(summary, null, 2));
process.exit(failures.length === 0 ? 0 : 1);
