# Cribra WASM — Node, Bun, and Deno

This example runs Cribra's production WebAssembly artifact directly from a
server-side JavaScript runtime.

The same `wasm-bindgen --target web` artifact used by browser integrations is
reused here. No Node-, Bun-, or Deno-specific Cribra build is required.

The runtime reads the Wasm module from the local filesystem and passes its bytes
explicitly to Cribra's initialization function.

## Build the production Wasm artifact

From the repository root:

```sh
just wasm-production
```

This creates the production artifact under:

```text
target/wasm-production/
├── cribra.js
├── cribra.d.ts
└── cribra_bg.wasm
```

## Run with Node.js

```sh
node examples/wasm-runtime/scan.mjs
```

Verified with Node.js 26.9.0.

## Run with Bun

```sh
bun examples/wasm-runtime/scan.mjs
```

Verified with Bun 1.4.2.

## Run with Deno

```sh
deno run --allow-read examples/wasm-runtime/scan.mjs
```

Verified with Deno 2.9.7.

`--allow-read` grants the example permission to read the local Wasm artifact.

## Expected result

The example enables both Cribra's current built-ins and the opt-in financial
pack. It scans synthetic PAN and IBAN values and produces two classified
findings.

It then creates a safe derivative using Cribra's generic redaction transform:

```text
service=checkout
card_number=[REDACTED]
iban=[REDACTED]
log_level=info
```

The example uses synthetic data only.

## Runtime model

Browser integrations can let the generated WebAssembly glue load
`cribra_bg.wasm` through its default URL/fetch path.

Server-side runtimes instead pass the Wasm bytes explicitly:

```text
production Wasm artifact
        │
        ├── browser
        │     └── URL / fetch initialization
        │
        └── JavaScript runtime
              └── explicit Wasm bytes
                    ├── Node.js
                    ├── Bun
                    └── Deno
```

Both paths use the same Cribra Wasm artifact and the same Rust semantic
authority.