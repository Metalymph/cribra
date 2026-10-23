# Cribra C example

This example consumes Cribra through its public native C ABI.

It demonstrates the complete integration path:

```text
builder
  → current built-ins
  → opt-in financial built-ins
  → scanner
  → borrowed active rule metadata
  → scan
  → findings
  → redaction
  → explicit cleanup
```

The example uses synthetic PAN and IBAN values only.

## Build Cribra's native adapter

From the repository root:

```sh
cargo build -p cribra-capi
```

## macOS

```sh
mkdir -p target/examples
clang -std=c11 -Wall -Wextra -Werror -Iinclude examples/c/scan.c -Ltarget/debug -lcribra_capi -Wl,-rpath,@loader_path/../debug -o target/examples/cribra-c-example
target/examples/cribra-c-example
```

## Linux

```sh
mkdir -p target/examples
cc -std=c11 -Wall -Wextra -Werror -Iinclude examples/c/scan.c -Ltarget/debug -lcribra_capi -Wl,-rpath,'$ORIGIN/../debug' -o target/examples/cribra-c-example
target/examples/cribra-c-example
```

## Ownership

Cribra owns the scanner, report, transformed output, and explicit error objects
returned through the ABI. Each owned handle has a corresponding destruction
function.

`cribra_builder_build` consumes a non-null builder on every build attempt.
Do not reuse or free the builder after calling it.

Finding views borrow data from their report. Output views borrow data from their
transformed output. Borrowed views must not outlive their owner.

The original source remains caller-owned.

`cribra_scanner_rule_count`/`cribra_scanner_rule_at` expose Core's active rule order
and five public metadata fields. Rule IDs borrow the scanner until scanner free;
copy their bytes for longer storage. The example also reads the new structured
construction category through `cribra_error_build_info` on build failure.
That view's optional rule ID borrows the error, independently of the builder,
until error free. Coarse statuses, fixed diagnostic messages and cleanup are
unchanged. No matcher/regex/source payload is exposed through metadata.

See `include/cribra.h` for the normative ABI contract.
