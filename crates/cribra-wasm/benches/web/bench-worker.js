const VARIANT_URLS = {
  base: "/target/wasm-bench/base/cribra.js",
  os: "/target/wasm-bench/os/cribra.js",
  oz: "/target/wasm-bench/oz/cribra.js",
  o3: "/target/wasm-bench/o3/cribra.js",
};

const MiB = 1024 * 1024;

function median(samples) {
  const sorted = [...samples].sort((a, b) => a - b);
  return sorted[Math.floor(sorted.length / 2)];
}

function p95(samples) {
  const sorted = [...samples].sort((a, b) => a - b);

  return sorted[
    Math.min(
      sorted.length - 1,
      Math.floor(sorted.length * 0.95),
    )
  ];
}

function summarize(
  samples,
  bytesPerOperation = null,
) {
  const med = median(samples);
  const sum = samples.reduce(
    (a, b) => a + b,
    0,
  );

  return {
    n: samples.length,
    median_ms: med,
    p95_ms: p95(samples),
    mean_ms: sum / samples.length,
    min_ms: Math.min(...samples),
    max_ms: Math.max(...samples),

    mib_per_s:
      bytesPerOperation == null ||
      med === 0
        ? null
        : (bytesPerOperation / MiB) /
          (med / 1000),
  };
}

function divideSummary(
  summary,
  divisor,
) {
  return {
    ...summary,

    median_ms:
      summary.median_ms / divisor,

    p95_ms:
      summary.p95_ms / divisor,

    mean_ms:
      summary.mean_ms / divisor,

    min_ms:
      summary.min_ms / divisor,

    max_ms:
      summary.max_ms / divisor,

    mib_per_s: null,
  };
}

function consume(value) {
  if (typeof value === "number") {
    return value;
  }

  if (typeof value === "string") {
    return value.length;
  }

  return 1;
}

function measure(
  samples,
  operation,
) {
  const values =
    new Array(samples);

  let sink = 0;

  for (
    let i = 0;
    i < samples;
    i++
  ) {
    const start =
      performance.now();

    sink +=
      consume(operation());

    values[i] =
      performance.now() -
      start;
  }

  if (
    sink ===
    Number.MIN_SAFE_INTEGER
  ) {
    console.log(sink);
  }

  return values;
}

/**
 * Measure very small operations without relying
 * on timer resolution for one individual call.
 *
 * Each recorded sample is normalized back to
 * milliseconds per operation.
 */
function measureBatched(
  samples,
  operationsPerSample,
  operation,
) {
  const values =
    new Array(samples);

  let sink = 0;

  for (
    let sample = 0;
    sample < samples;
    sample++
  ) {
    const start =
      performance.now();

    for (
      let operationIndex = 0;
      operationIndex <
      operationsPerSample;
      operationIndex++
    ) {
      sink +=
        consume(operation());
    }

    const elapsed =
      performance.now() -
      start;

    values[sample] =
      elapsed /
      operationsPerSample;
  }

  if (
    sink ===
    Number.MIN_SAFE_INTEGER
  ) {
    console.log(sink);
  }

  return values;
}

function warmup(
  iterations,
  operation,
) {
  let sink = 0;

  for (
    let i = 0;
    i < iterations;
    i++
  ) {
    sink +=
      consume(operation());
  }

  if (
    sink ===
    Number.MIN_SAFE_INTEGER
  ) {
    console.log(sink);
  }
}

async function loadVariant(
  variant,
  nonce = "",
) {
  const moduleUrl =
    `${VARIANT_URLS[variant]}?bench=${encodeURIComponent(
      nonce,
    )}`;

  const mod =
    await import(moduleUrl);

  const initStart =
    performance.now();

  await mod.default();

  const wasmInitMs =
    performance.now() -
    initStart;

  return {
    mod,
    wasmInitMs,
  };
}

function scanOnce(
  engine,
  source,
) {
  const result =
    engine.scan(source);

  try {
    return (
      result.findingCount() +
      result.candidateCount()
    );
  } finally {
    result.free();
  }
}

function scanMany(
  engine,
  sources,
) {
  let sink = 0;

  for (
    const source of sources
  ) {
    sink +=
      scanOnce(
        engine,
        source,
      );
  }

  return sink;
}

function buildFindingFixture(
  mod,
  findingCount = 256,
) {
  const secret =
    "CRIBRA_BENCH_SECRET_0123456789ABCDEF";

  const builder =
    new mod.ScanEngineBuilder(
      false,
    );

  builder.addLiteral(
    "bench.literal",
    secret,
    mod.FindingSeverity.High,
  );

  /*
   * build() consumes the builder.
   */
  const engine =
    builder.build();

  const source =
    Array.from(
      {
        length:
          findingCount,
      },
      (_, index) =>
        `row=${index} token=${secret}\n`,
    ).join("");

  const result =
    engine.scan(source);

  if (
    result.findingCount() !==
    findingCount
  ) {
    result.free();
    engine.free();

    throw new Error(
      `finding fixture expected ${findingCount} findings, got ${result.findingCount()}`,
    );
  }

  return {
    engine,
    source,
    result,
  };
}

function buildCandidateFixture(
  mod,
) {
  const engine =
    new mod.ScanEngine();

  const source =
    "ABCD-EFGH-IJKL-MNOP";

  const result =
    engine.scan(source);

  if (
    result.candidateCount() ===
    0
  ) {
    result.free();
    engine.free();

    throw new Error(
      "candidate fixture did not produce a review candidate",
    );
  }

  return {
    engine,
    source,
    result,
  };
}

function traverseFindings(
  result,
  count,
) {
  let sink = 0;

  const available =
    result.findingCount();

  const limit =
    Math.min(
      count,
      available,
    );

  for (
    let index = 0;
    index < limit;
    index++
  ) {
    const view =
      result.findingAt(
        index,
      );

    try {
      sink += view.start;
      sink += view.end;
      sink += view.line;
      sink += view.column;
      sink +=
        view.ruleId.length;
      sink += view.severity;
      sink +=
        view.confidence;
      sink +=
        view.remediation;
    } finally {
      view.free();
    }
  }

  return sink;
}

function explainFindings(
  engine,
  result,
  count,
) {
  let sink = 0;

  const available =
    result.findingCount();

  const limit =
    Math.min(
      count,
      available,
    );

  for (
    let index = 0;
    index < limit;
    index++
  ) {
    const explanation =
      engine.explainFinding(
        result,
        index,
      );

    try {
      sink +=
        explanation.kind;

      if (
        explanation
          .detectionMode !=
        null
      ) {
        sink +=
          explanation
            .detectionMode;
      }

      if (
        explanation
          .candidateEvidence !=
        null
      ) {
        sink +=
          explanation
            .candidateEvidence;
      }
    } finally {
      explanation.free();
    }
  }

  return sink;
}

function readCandidate(
  result,
) {
  const view =
    result.candidateAt(0);

  try {
    return (
      view.start +
      view.end +
      view.line +
      view.column +
      view.kind +
      view.evidence
    );
  } finally {
    view.free();
  }
}

function explainCandidate(
  result,
) {
  const explanation =
    result
      .candidateExplanationAt(
        0,
      );

  try {
    let sink =
      explanation.kind;

    if (
      explanation
        .detectionMode !=
      null
    ) {
      sink +=
        explanation
          .detectionMode;
    }

    if (
      explanation
        .candidateEvidence !=
      null
    ) {
      sink +=
        explanation
          .candidateEvidence;
    }

    return sink;
  } finally {
    explanation.free();
  }
}

function buildFourRuleEngine(
  mod,
) {
  const builder =
    new mod.ScanEngineBuilder(
      false,
    );

  builder.addLiteral(
    "bench.literal",
    "BENCH_LITERAL",
    mod.FindingSeverity.High,
  );

  builder.addPrefix(
    "bench.prefix",
    "bench_",
    mod.FindingSeverity.Medium,
  );

  builder.addSuffix(
    "bench.suffix",
    "_bench",
    mod.FindingSeverity.Low,
  );

  builder.addPattern(
    "bench.pattern",
    "\\bBENCH_[A-Z0-9]{8}\\b",
    mod.FindingSeverity.High,
  );

  /*
   * build() consumes builder.
   */
  return builder.build();
}

async function runtimeBench(
  variant,
) {
  const {
    mod,
    wasmInitMs,
  } =
    await loadVariant(
      variant,
      "runtime",
    );

  const engine =
    new mod.ScanEngine();

  /*
   * Zero-rule scanner used to estimate the practical
   * JS -> WASM source-copy and boundary floor.
   */
  const emptyBuilder =
    new mod.ScanEngineBuilder(
      false,
    );

  /*
   * build() consumes the builder.
   */
  const emptyEngine =
    emptyBuilder.build();

  const clean64 =
    "#".repeat(64);

  const clean4k =
    "#".repeat(
      4 * 1024,
    );

  const clean64k =
    "#".repeat(
      64 * 1024,
    );

  const clean1m =
    "#".repeat(
      1024 * 1024,
    );

  /*
   * Minimal WASM boundary call.
   */
  warmup(
    10_000,
    () =>
      engine.rulesCount(),
  );

  const minimalCall =
    summarize(
      measureBatched(
        200,
        10_000,
        () =>
          engine.rulesCount(),
      ),
    );

  /*
   * 64-byte scan.
   */
  warmup(
    2_000,
    () =>
      scanOnce(
        engine,
        clean64,
      ),
  );

  const scan64 =
    summarize(
      measureBatched(
        150,
        1_000,
        () =>
          scanOnce(
            engine,
            clean64,
          ),
      ),
      clean64.length,
    );

  /*
   * 4 KiB scan.
   */
  warmup(
    500,
    () =>
      scanOnce(
        engine,
        clean4k,
      ),
  );

  const scan4k =
    summarize(
      measureBatched(
        150,
        100,
        () =>
          scanOnce(
            engine,
            clean4k,
          ),
      ),
      clean4k.length,
    );

  /*
   * 64 KiB scan.
   */
  warmup(
    30,
    () =>
      scanOnce(
        engine,
        clean64k,
      ),
  );

  const scan64k =
    summarize(
      measure(
        120,
        () =>
          scanOnce(
            engine,
            clean64k,
          ),
      ),
      clean64k.length,
    );

  /*
   * 1 MiB scan.
   */
  warmup(
    5,
    () =>
      scanOnce(
        engine,
        clean1m,
      ),
  );

  const scan1m =
    summarize(
      measure(
        30,
        () =>
          scanOnce(
            engine,
            clean1m,
          ),
      ),
      clean1m.length,
    );

  /*
   * Practical JS -> WASM boundary/source-copy floor.
   *
   * The zero-rule scanner crosses the same public
   * scan(String) boundary and constructs a ScanResult,
   * but performs no matcher work.
   */

  warmup(
    500,
    () =>
      scanOnce(
        emptyEngine,
        clean4k,
      ),
  );

  const boundary4k =
    summarize(
      measureBatched(
        150,
        100,
        () =>
          scanOnce(
            emptyEngine,
            clean4k,
          ),
      ),
      clean4k.length,
    );

  warmup(
    30,
    () =>
      scanOnce(
        emptyEngine,
        clean64k,
      ),
  );

  const boundary64k =
    summarize(
      measure(
        120,
        () =>
          scanOnce(
            emptyEngine,
            clean64k,
          ),
      ),
      clean64k.length,
    );

  warmup(
    5,
    () =>
      scanOnce(
        emptyEngine,
        clean1m,
      ),
  );

  const boundary1m =
    summarize(
      measure(
        30,
        () =>
          scanOnce(
            emptyEngine,
            clean1m,
          ),
      ),
      clean1m.length,
    );

  /*
   * Serial multi-source amortization.
   *
   * This deliberately uses the current public scan()
   * API repeatedly instead of inventing a scanBatch()
   * API before measurement justifies one.
   */

  const batchSource =
    "#".repeat(
      64 * 1024,
    );

  const batch1 = [
    batchSource,
  ];

  const batch8 =
    Array.from(
      { length: 8 },
      () => batchSource,
    );

  const batch32 =
    Array.from(
      { length: 32 },
      () => batchSource,
    );

  warmup(
    20,
    () =>
      scanMany(
        engine,
        batch1,
      ),
  );

  const serialBatch1 =
    summarize(
      measure(
        100,
        () =>
          scanMany(
            engine,
            batch1,
          ),
      ),
    );

  warmup(
    10,
    () =>
      scanMany(
        engine,
        batch8,
      ),
  );

  const serialBatch8 =
    summarize(
      measure(
        50,
        () =>
          scanMany(
            engine,
            batch8,
          ),
      ),
    );

  warmup(
    3,
    () =>
      scanMany(
        engine,
        batch32,
      ),
  );

  const serialBatch32 =
    summarize(
      measure(
        20,
        () =>
          scanMany(
            engine,
            batch32,
          ),
      ),
    );

  /*
   * Typed projection / finding traversal.
   */

  const findingFixture =
    buildFindingFixture(
      mod,
      256,
    );

  warmup(
    2_000,
    () =>
      traverseFindings(
        findingFixture.result,
        1,
      ),
  );

  const findingAt1 =
    summarize(
      measureBatched(
        150,
        1_000,
        () =>
          traverseFindings(
            findingFixture.result,
            1,
          ),
      ),
    );

  warmup(
    500,
    () =>
      traverseFindings(
        findingFixture.result,
        16,
      ),
  );

  const findingAt16 =
    summarize(
      measureBatched(
        150,
        100,
        () =>
          traverseFindings(
            findingFixture.result,
            16,
          ),
      ),
    );

  warmup(
    100,
    () =>
      traverseFindings(
        findingFixture.result,
        256,
      ),
  );

  const findingAt256 =
    summarize(
      measure(
        300,
        () =>
          traverseFindings(
            findingFixture.result,
            256,
          ),
      ),
    );

  /*
   * Finding explanation boundary.
   */

  warmup(
    1_000,
    () =>
      explainFindings(
        findingFixture.engine,
        findingFixture.result,
        1,
      ),
  );

  const explainFinding1 =
    summarize(
      measureBatched(
        150,
        500,
        () =>
          explainFindings(
            findingFixture.engine,
            findingFixture.result,
            1,
          ),
      ),
    );

  warmup(
    100,
    () =>
      explainFindings(
        findingFixture.engine,
        findingFixture.result,
        256,
      ),
  );

  const explainFinding256 =
    summarize(
      measure(
        200,
        () =>
          explainFindings(
            findingFixture.engine,
            findingFixture.result,
            256,
          ),
      ),
    );

  /*
   * Candidate typed projection and explanation.
   */

  const candidateFixture =
    buildCandidateFixture(
      mod,
    );

  warmup(
    2_000,
    () =>
      readCandidate(
        candidateFixture.result,
      ),
  );

  const candidateAt1 =
    summarize(
      measureBatched(
        150,
        1_000,
        () =>
          readCandidate(
            candidateFixture.result,
          ),
      ),
    );

  warmup(
    2_000,
    () =>
      explainCandidate(
        candidateFixture.result,
      ),
  );

  const candidateExplanation1 =
    summarize(
      measureBatched(
        150,
        1_000,
        () =>
          explainCandidate(
            candidateFixture.result,
          ),
      ),
    );

  /*
   * Transform boundary.
   */

  const pseudonymKey =
    new Uint8Array(
      32,
    ).fill(0x31);

  const synthesisKey =
    new Uint8Array(
      32,
    ).fill(0x53);

  warmup(
    100,
    () =>
      findingFixture
        .result
        .redact(
          findingFixture.source,
        ),
  );

  const redact =
    summarize(
      measure(
        200,
        () =>
          findingFixture
            .result
            .redact(
              findingFixture.source,
            ),
      ),
      findingFixture
        .source.length,
    );

  warmup(
    100,
    () =>
      findingFixture
        .result
        .template(
          findingFixture.source,
        ),
  );

  const template =
    summarize(
      measure(
        200,
        () =>
          findingFixture
            .result
            .template(
              findingFixture.source,
            ),
      ),
      findingFixture
        .source.length,
    );

  warmup(
    50,
    () =>
      findingFixture
        .result
        .pseudonymize(
          findingFixture.source,
          pseudonymKey,
          "cribra_pseudo_",
          16,
        ),
  );

  const pseudonymize =
    summarize(
      measure(
        150,
        () =>
          findingFixture
            .result
            .pseudonymize(
              findingFixture.source,
              pseudonymKey,
              "cribra_pseudo_",
              16,
            ),
      ),
      findingFixture
        .source.length,
    );

  warmup(
    50,
    () =>
      findingFixture
        .result
        .synthesize(
          findingFixture.source,
          synthesisKey,
          "cribra_synthetic",
        ),
  );

  const synthesize =
    summarize(
      measure(
        150,
        () =>
          findingFixture
            .result
            .synthesize(
              findingFixture.source,
              synthesisKey,
              "cribra_synthetic",
            ),
      ),
      findingFixture
        .source.length,
    );

  /*
   * Generic custom scanner construction.
   */

  warmup(
    100,
    () => {
      const built =
        buildFourRuleEngine(
          mod,
        );

      const count =
        built.rulesCount();

      built.free();

      return count;
    },
  );

  const builderBuild =
    summarize(
      measure(
        200,
        () => {
          const built =
            buildFourRuleEngine(
              mod,
            );

          const count =
            built.rulesCount();

          built.free();

          return count;
        },
      ),
    );

  candidateFixture
    .result
    .free();

  candidateFixture
    .engine
    .free();

  findingFixture
    .result
    .free();

  findingFixture
    .engine
    .free();

  emptyEngine.free();
  engine.free();

  return {
    wasm_init_ms:
      wasmInitMs,

    runtime: {
      minimal_call_rulesCount:
        minimalCall,

      scan_64B_clean:
        scan64,

      scan_4KiB_clean:
        scan4k,

      scan_64KiB_clean:
        scan64k,

      scan_1MiB_clean:
        scan1m,

      boundary_zero_rules_4KiB:
        boundary4k,

      boundary_zero_rules_64KiB:
        boundary64k,

      boundary_zero_rules_1MiB:
        boundary1m,

      serial_64KiB_x1_per_source:
        divideSummary(
          serialBatch1,
          1,
        ),

      serial_64KiB_x8_per_source:
        divideSummary(
          serialBatch8,
          8,
        ),

      serial_64KiB_x32_per_source:
        divideSummary(
          serialBatch32,
          32,
        ),

      findingAt_1:
        findingAt1,

      findingAt_16:
        findingAt16,

      findingAt_256:
        findingAt256,

      explainFinding_1:
        explainFinding1,

      explainFinding_256:
        explainFinding256,

      candidateAt_1:
        candidateAt1,

      candidateExplanationAt_1:
        candidateExplanation1,

      redact_256_findings:
        redact,

      template_256_findings:
        template,

      pseudonymize_256_findings:
        pseudonymize,

      synthesize_256_findings:
        synthesize,

      custom_builder_4_rules:
        builderBuild,
    },
  };
}

self.addEventListener(
  "message",
  async (event) => {
    const {
      mode,
      variant,
      nonce = "",
    } =
      event.data ?? {};

    try {
      if (
        !(variant in
          VARIANT_URLS)
      ) {
        throw new Error(
          `unknown variant: ${variant}`,
        );
      }

      if (
        mode === "startup"
      ) {
        const {
          wasmInitMs,
        } =
          await loadVariant(
            variant,
            `startup-${nonce}-${crypto.randomUUID()}`,
          );

        self.postMessage({
          ok: true,

          value: {
            wasm_init_ms:
              wasmInitMs,
          },
        });

        return;
      }

      if (
        mode === "runtime"
      ) {
        const value =
          await runtimeBench(
            variant,
          );

        self.postMessage({
          ok: true,
          value:
            value.runtime,
        });

        return;
      }

      throw new Error(
        `unknown benchmark mode: ${mode}`,
      );
    } catch (error) {
      self.postMessage({
        ok: false,

        error:
          error instanceof Error
            ? `${error.name}: ${error.message}`
            : String(error),
      });
    }
  },
);