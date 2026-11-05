/** Isolated consumer creation and child-process helpers. */

import { spawn, spawnSync } from "node:child_process";
import { cp, mkdir, mkdtemp, readdir, readFile, rm, writeFile } from "node:fs/promises";
import { basename, dirname, resolve } from "node:path";

import { consumersRoot } from "./artifacts.mjs";

export async function createConsumer({ label, tarball }) {
  await mkdir(consumersRoot, { recursive: true });
  const directory = await mkdtemp(resolve(consumersRoot, `${label}-`));
  await writeFile(
    resolve(directory, "package.json"),
    `${JSON.stringify(
      {
        name: `cribra-consumer-${label.replace(/[^a-z0-9-]/gi, "-")}`,
        private: true,
        type: "module",
        version: "0.0.0",
      },
      null,
      2,
    )}\n`,
    "utf8",
  );
  const install = spawnSync(
    "npm",
    [
      "install",
      "--ignore-scripts",
      "--no-audit",
      "--no-fund",
      "--no-package-lock",
      tarball,
    ],
    {
      cwd: directory,
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
      maxBuffer: 64 * 1024 * 1024,
    },
  );
  if (install.status !== 0) {
    throw new Error(
      `npm install failed for ${label}: ${install.stderr || install.stdout}`,
    );
  }
  const installed = resolve(directory, "node_modules/cribra-typescript");
  const manifest = JSON.parse(
    await readFile(resolve(installed, "package.json"), "utf8"),
  );
  if (manifest.name !== "cribra-typescript") {
    throw new Error(`unexpected installed package ${manifest.name}`);
  }
  return { directory, installed, manifest };
}

export async function copyFixture(source, destination) {
  await cp(source, destination, { recursive: true });
}

export async function removeConsumer(directory) {
  if (process.env.CRIBRA_KEEP_CONSUMERS === "1") {
    process.stdout.write(`kept consumer: ${directory}\n`);
    return;
  }
  await rm(directory, { recursive: true, force: true });
}

export function nodePathWithoutRust() {
  const nodeDir = dirname(process.execPath);
  return [nodeDir, "/usr/bin", "/bin"].join(":");
}

export function runChild({
  command = process.execPath,
  args,
  cwd,
  env = {},
  timeout = 120_000,
  label,
}) {
  return new Promise((resolvePromise, rejectPromise) => {
    const child = spawn(command, args, {
      cwd,
      env: {
        ...process.env,
        PATH: nodePathWithoutRust(),
        CRIBRA_EXPECT_NO_RUST: "1",
        ...env,
      },
      stdio: ["ignore", "pipe", "pipe"],
    });
    let stdout = "";
    let stderr = "";
    let settled = false;
    const timer = setTimeout(() => {
      if (settled) return;
      settled = true;
      child.kill("SIGKILL");
      rejectPromise(new Error(`${label ?? command} timed out after ${timeout} ms`));
    }, timeout);
    child.stdout.on("data", (chunk) => {
      stdout += chunk;
    });
    child.stderr.on("data", (chunk) => {
      stderr += chunk;
    });
    child.once("error", (error) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      rejectPromise(error);
    });
    child.once("close", (code) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      if (code !== 0) {
        rejectPromise(
          new Error(
            `${label ?? command} exited with ${code}\nstdout:\n${stdout}\nstderr:\n${stderr}`,
          ),
        );
        return;
      }
      resolvePromise({ stdout, stderr });
    });
  });
}

export async function listInstalledFiles(root) {
  const result = [];
  async function walk(current) {
    const entries = await readdir(current, { withFileTypes: true });
    entries.sort((left, right) => left.name.localeCompare(right.name));
    for (const entry of entries) {
      const path = resolve(current, entry.name);
      if (entry.isDirectory()) await walk(path);
      else if (entry.isFile()) result.push(path);
    }
  }
  await walk(root);
  return result;
}

export function tarballName(tarball) {
  return basename(tarball);
}
