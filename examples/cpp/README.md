# Cribra C++ example

Cribra exposes a native C ABI rather than a separate C++ binding.

This example shows how a C++ application can consume that ABI while adding
normal C++ ownership ergonomics locally.

The example wraps the Rust-owned scanner, report, and transformed output handles
with small move-only RAII types whose destructors call the corresponding Cribra
destruction functions.

No detection or classification semantics are reimplemented in C++.

## Build Cribra's native adapter

From the repository root:

```sh
cargo build -p cribra-capi
```

## macOS

```sh
mkdir -p target/examples
clang++ -std=c++17 -Wall -Wextra -Werror -Iinclude examples/cpp/scan.cpp -Ltarget/debug -lcribra_capi -Wl,-rpath,@loader_path/../debug -o target/examples/cribra-cpp-example
target/examples/cribra-cpp-example
```

## Linux

```sh
mkdir -p target/examples
c++ -std=c++17 -Wall -Wextra -Werror -Iinclude examples/cpp/scan.cpp -Ltarget/debug -lcribra_capi -Wl,-rpath,'$ORIGIN/../debug' -o target/examples/cribra-cpp-example
target/examples/cribra-cpp-example
```

## What the example demonstrates

The scanner is built from Cribra's current built-ins plus the opt-in financial
pack. Synthetic PAN and IBAN values are classified by the Rust core and then
redacted through Cribra's generic transform API.

The C++ wrappers manage ownership only:

```text
C++ application
      │
      └── local RAII wrappers
                │
                ▼
          Cribra C ABI
                │
                ▼
          Cribra Rust core
```

The wrappers are example code, not an additional Cribra binding or semantic
authority.

See `include/cribra.h` for the normative ABI contract.