---
title: "Kotlin/JVM integration"
description: "Build and consume a single profile JAR with its matching native library."
order: 13
---

# Kotlin/JVM integration

> **Source and distribution boundary:** This guide describes the repository
> contracts recorded in the W2.2 audit. Source implementation, local tests,
> CI qualification, and publication are distinct claims. Consult the linked
> adapter README and compatibility matrix before shipping binaries.

Cribra performs deterministic scanning in Rust Core. A **finding** is an
authoritative detection; a **sensitive candidate** is review-only and must not
be silently transformed. Spans are zero-based, half-open **UTF-8 byte ranges**.
Streaming admits only compatible rulesets; an ordinary default scanner is not
automatically streamable. Resource budgets are logical limits, not process-wide
heap guarantees.

## Distribution and scope

Kotlin/JVM uses generated UniFFI bindings and JNA, not a second detection
engine. The repository builds host-specific Core, Flow and Fuse ZIP bundles;
**Maven Central publication is not established**. Android and Kotlin
Multiplatform are **not qualified** by this JVM integration.

Each profile has a thin JAR plus matching native library. Use exactly one
profile on the classpath: generated packages and symbols overlap.

## Build and verify

The documented baseline is Rust 1.97, Kotlin 2.4.21, JDK 17, Gradle wrapper
9.3.1, and JNA 5.17.0. The first Gradle run may need network access.

```sh
just uniffi-kotlin-smoke
just uniffi-kotlin-flow-smoke
just uniffi-kotlin-fuse-smoke
just uniffi-kotlin-jars
just uniffi-kotlin-bundles
```

Profile JARs are under [`bindings/kotlin/build/libs/`](https://github.com/Metalymph/cribra/tree/main/bindings/kotlin/build/libs/); bundles are generated
under ignored `target/uniffi-kotlin-bundles/dist/`. A JAR alone does not include
the host native library. The consumer must supply Kotlin runtime and JNA and
follow the documented native loader contract (`jna.library.path` where needed).

## Qualification boundaries

Recorded CI ran isolated bundle consumers on macOS ARM64, Linux x86_64 and
Windows x86_64. This does not qualify Android, KMP, arbitrary JDK versions,
other architectures, or external Gradle application integration.

The same Core-owned streaming, revision, UTF-8 edit boundaries and
finding/candidate distinctions apply to Kotlin consumers.

## Source references

- [`bindings/kotlin/README.md`](https://github.com/Metalymph/cribra/blob/main/bindings/kotlin/README.md)
- [`bindings/kotlin/verify/consumer/`](https://github.com/Metalymph/cribra/tree/main/bindings/kotlin/verify/consumer/)
- [`docs/COMPATIBILITY.md`](https://github.com/Metalymph/cribra/blob/main/docs/COMPATIBILITY.md)
