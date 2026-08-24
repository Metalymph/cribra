import {
  readFile,
  readdir,
  stat,
} from "node:fs/promises";

import {
  resolve,
} from "node:path";

const root = process.cwd();

const productionDir =
  resolve(
    root,
    "target/wasm-production",
  );

const expectedFiles = [
  "cribra.js",
  "cribra.d.ts",
  "cribra_bg.wasm",
  "cribra_bg.wasm.d.ts",
  "package.json",
];

const requiredPublicDeclarations = [
  "export enum CandidateEvidenceKind",
  "export enum CandidateKind",
  "export class CandidateView",
  "export enum DetectionModeKind",
  "export enum ExplanationKind",
  "export class ExplanationView",
  "export enum FindingConfidence",
  "export enum FindingSeverity",
  "export class FindingView",
  "export enum RemediationKind",
  "export class ScanEngine",
  "export class ScanEngineBuilder",
  "export class ScanResult",
];

const requiredMethods = [
  "scan(source: string): ScanResult",
  "explainFinding(result: ScanResult, index: number): ExplanationView",
  "findingAt(index: number): FindingView",
  "candidateAt(index: number): CandidateView",
  "candidateExplanationAt(index: number): ExplanationView",
  "redact(source: string): string",
  "template(source: string): string",
  "pseudonymize(",
  "synthesize(",
  "addLiteral(",
  "addPrefix(",
  "addSuffix(",
  "addPattern(",
];

function fail(message) {
  throw new Error(
    `cribra WASM package validation failed: ${message}`,
  );
}

async function requireNonEmptyFile(
  name,
) {
  const path =
    resolve(
      productionDir,
      name,
    );

  let info;

  try {
    info =
      await stat(path);
  } catch {
    fail(
      `missing production artifact: ${name}`,
    );
  }

  if (!info.isFile()) {
    fail(
      `production artifact is not a file: ${name}`,
    );
  }

  if (info.size === 0) {
    fail(
      `production artifact is empty: ${name}`,
    );
  }

  return {
    path,
    size: info.size,
  };
}

async function validateExactArtifactSet() {
  const entries =
    (
      await readdir(
        productionDir,
        {
          withFileTypes: true,
        },
      )
    )
      .filter(
        (entry) =>
          entry.isFile(),
      )
      .map(
        (entry) =>
          entry.name,
      )
      .sort();

  const expected =
    [...expectedFiles].sort();

  if (
    JSON.stringify(entries) !==
    JSON.stringify(expected)
  ) {
    fail(
      [
        "unexpected production artifact set",
        `expected: ${expected.join(", ")}`,
        `actual: ${entries.join(", ")}`,
      ].join("\n"),
    );
  }
}

async function validateWasmMagic() {
  const wasm =
    await readFile(
      resolve(
        productionDir,
        "cribra_bg.wasm",
      ),
    );

  const expectedMagic = [
    0x00,
    0x61,
    0x73,
    0x6d,
    0x01,
    0x00,
    0x00,
    0x00,
  ];

  if (
    wasm.length <
    expectedMagic.length
  ) {
    fail(
      "WASM artifact is shorter than the WASM header",
    );
  }

  for (
    let index = 0;
    index < expectedMagic.length;
    index++
  ) {
    if (
      wasm[index] !==
      expectedMagic[index]
    ) {
      fail(
        "cribra_bg.wasm does not contain the expected WASM v1 header",
      );
    }
  }
}

async function validateModulePackage() {
  const packageJson =
    JSON.parse(
      await readFile(
        resolve(
          productionDir,
          "package.json",
        ),
        "utf8",
      ),
    );

  if (
    packageJson.type !==
    "module"
  ) {
    fail(
      'package.json must declare "type": "module"',
    );
  }

  if (
    Object.keys(packageJson)
      .length !== 1
  ) {
    fail(
      "generated package.json contains unexpected metadata",
    );
  }
}

async function validateGlue() {
  const glue =
    await readFile(
      resolve(
        productionDir,
        "cribra.js",
      ),
      "utf8",
    );

  const required = [
    "export class ScanEngine",
    "export class ScanEngineBuilder",
    "export class ScanResult",
    "export class FindingView",
    "export class CandidateView",
    "export class ExplanationView",
    "new URL('cribra_bg.wasm', import.meta.url)",
    "WebAssembly.instantiate",
  ];

  for (
    const token of required
  ) {
    if (
      !glue.includes(token)
    ) {
      fail(
        `generated JS glue is missing expected token: ${token}`,
      );
    }
  }

  const forbidden = [
    "serde_json",
    "JSON.stringify",
    "JSON.parse",
    "cribra_scanner_",
    "cribra_builder_",
    "cribra_report_",
    "cribra_transform_",
    "cribra_error_",
  ];

  for (
    const token of forbidden
  ) {
    if (
      glue.includes(token)
    ) {
      fail(
        `generated JS glue contains forbidden transport/ABI token: ${token}`,
      );
    }
  }

  if (
    glue.includes(root)
  ) {
    fail(
      "generated JS glue leaks the local repository path",
    );
  }
}

async function validatePublicDeclarations() {
  const declarations =
    await readFile(
      resolve(
        productionDir,
        "cribra.d.ts",
      ),
      "utf8",
    );

  for (
    const declaration of
      requiredPublicDeclarations
  ) {
    if (
      !declarations.includes(
        declaration,
      )
    ) {
      fail(
        `cribra.d.ts is missing public declaration: ${declaration}`,
      );
    }
  }

  for (
    const method of
      requiredMethods
  ) {
    if (
      !declarations.includes(
        method,
      )
    ) {
      fail(
        `cribra.d.ts is missing expected method: ${method}`,
      );
    }
  }

  const forbidden = [
    "serde_json",
    "JSONValue",
    "cribra_scanner_",
    "cribra_builder_",
    "cribra_report_",
    "cribra_transform_",
    "cribra_error_",
  ];

  for (
    const token of forbidden
  ) {
    if (
      declarations.includes(
        token,
      )
    ) {
      fail(
        `cribra.d.ts contains forbidden transport/ABI token: ${token}`,
      );
    }
  }

  if (
    declarations.includes(root)
  ) {
    fail(
      "cribra.d.ts leaks the local repository path",
    );
  }
}

async function validateRawDeclarations() {
  const declarations =
    await readFile(
      resolve(
        productionDir,
        "cribra_bg.wasm.d.ts",
      ),
      "utf8",
    );

  if (
    !declarations.includes(
      "__wbindgen_malloc",
    )
  ) {
    fail(
      "raw WASM declarations do not contain expected wasm-bindgen allocator export",
    );
  }

  const forbidden = [
    "cribra_scanner_",
    "cribra_builder_",
    "cribra_report_",
    "cribra_transform_",
    "cribra_error_",
  ];

  for (
    const token of forbidden
  ) {
    if (
      declarations.includes(
        token,
      )
    ) {
      fail(
        `raw WASM declarations expose C ABI token: ${token}`,
      );
    }
  }

  if (
    declarations.includes(root)
  ) {
    fail(
      "raw WASM declarations leak the local repository path",
    );
  }
}

async function main() {
  await validateExactArtifactSet();

  const artifacts = [];

  for (
    const name of
      expectedFiles
  ) {
    artifacts.push(
      await requireNonEmptyFile(
        name,
      ),
    );
  }

  await validateWasmMagic();
  await validateModulePackage();
  await validateGlue();
  await validatePublicDeclarations();
  await validateRawDeclarations();

  const totalBytes =
    artifacts.reduce(
      (total, artifact) =>
        total +
        artifact.size,
      0,
    );

  console.log(
    "cribra WASM production package: ok",
  );

  for (
    let index = 0;
    index <
      expectedFiles.length;
    index++
  ) {
    console.log(
      `  ${expectedFiles[index]}: ${artifacts[index].size} B`,
    );
  }

  console.log(
    `  total: ${totalBytes} B`,
  );
}

await main();