# Cribra contributor/development commands.
#
# `just` is the preferred local task runner. The Makefile mirrors the most
# common commands for contributors who do not use just.

set shell := ["sh", "-cu"]

default:
    @just --list

# Show available recipes.
help:
    @just --list

# Start the independent Astro website development server.
website-dev:
    npm --prefix website run dev

# Build the static website and its Pagefind search index.
website-build:
    npm --prefix website run build

# Check Astro and TypeScript in the website only.
website-check:
    npm --prefix website run check

# Format the workspace.
fmt:
    cargo fmt --all

# Check formatting without modifying files.
fmt-check:
    cargo fmt --all -- --check

# Check the default core build.
check:
    cargo check

# Check every optional core feature.
check-all:
    cargo check --all-features

# Build/generate the Core-only UniFFI Python consumer and run its focused tests.
uniffi-python-smoke python="python3.13":
    "{{python}}" bindings/python/smoke.py

# Build/generate isolated Flow bindings and qualify Core, Flow and Share in Python.
uniffi-python-flow-smoke python="python3.13":
    "{{python}}" bindings/python/smoke.py flow

# Qualify Core, Flow/Share and Core-owned parallel scanning with isolated Fuse bindings.
uniffi-python-fuse-smoke python="python3.13":
    "{{python}}" bindings/python/smoke.py fuse

# Build the local Fuse wheel with a Python environment containing build-requirements.txt.
uniffi-python-wheel python="python3.13":
    "{{python}}" bindings/python/build_wheel.py

# Qualify the prebuilt Fuse wheel across the local pyenv interpreter matrix.
uniffi-python-wheel-matrix wheel python="python3.13":
    "{{python}}" bindings/python/wheel_smoke.py "{{wheel}}"

# Qualify the prebuilt Fuse wheel on one pyenv version or interpreter path.
uniffi-python-wheel-smoke wheel interpreter="3.13.7" python="python3.13":
    "{{python}}" bindings/python/wheel_smoke.py "{{wheel}}" --python "{{interpreter}}"

# Build and auditwheel-qualify the manylinux_2_28_x86_64 Fuse wheel (needs Docker).
uniffi-python-manylinux-wheel:
    sh bindings/python/build_manylinux_wheel.sh

# Qualify the repaired manylinux wheel on the container's CPython 3.11-3.14 (needs Docker).
uniffi-python-manylinux-wheel-matrix wheel="":
    sh bindings/python/qualify_manylinux_wheel.sh "{{wheel}}"

# Build the Core native library for the host, generate the Kotlin/JVM binding under
# target/ and run the JVM consumer smoke against the real cdylib (needs JDK 17).
uniffi-kotlin-smoke:
    CARGO_TARGET_DIR=target cargo +1.97.0 build -p cribra-uniffi --no-default-features --features core --lib
    case "$(uname -s)" in Darwin) library="libcribra_uniffi.dylib" ;; Linux) library="libcribra_uniffi.so" ;; MINGW*|MSYS*|CYGWIN*) library="cribra_uniffi.dll" ;; *) echo "uniffi-kotlin-smoke: unsupported host $(uname -s)" >&2; exit 2 ;; esac; \
    CARGO_TARGET_DIR=target cargo +1.97.0 run -p cribra-uniffi --no-default-features --features bindgen,core --bin cribra-uniffi-bindgen -- generate --library "target/debug/$library" --language kotlin --out-dir target/uniffi-kotlin-smoke/generated
    CARGO_TARGET_DIR=target cargo +1.97.0 run -p cribra-uniffi --no-default-features --features core --example scanner_oracle > target/uniffi-kotlin-smoke/scanner-oracle.json
    bindings/kotlin/gradlew -p bindings/kotlin --console=plain run runCoreConformance

# Build the Flow/Share native library, generate the Kotlin/JVM binding and the
# direct-Core oracle outputs under target/, then run the Flow/Share JVM smoke
# against the real cdylib (needs JDK 17).
uniffi-kotlin-flow-smoke:
    case "$(uname -s)" in Darwin) library="libcribra_uniffi.dylib" ;; Linux) library="libcribra_uniffi.so" ;; MINGW*|MSYS*|CYGWIN*) library="cribra_uniffi.dll" ;; *) echo "uniffi-kotlin-flow-smoke: unsupported host $(uname -s)" >&2; exit 2 ;; esac; \
    mkdir -p target/uniffi-kotlin-flow-smoke && \
    CARGO_TARGET_DIR=target cargo +1.97.0 build -p cribra-uniffi --no-default-features --features flow --lib && \
    CARGO_TARGET_DIR=target cargo +1.97.0 run -p cribra-uniffi --no-default-features --features bindgen,flow --bin cribra-uniffi-bindgen -- generate --library "target/debug/$library" --language kotlin --out-dir target/uniffi-kotlin-flow-smoke/generated && \
    CARGO_TARGET_DIR=target cargo +1.97.0 run -p cribra-uniffi --no-default-features --features flow --example flow_oracle > target/uniffi-kotlin-flow-smoke/flow-oracle.json && \
    CARGO_TARGET_DIR=target cargo +1.97.0 run -p cribra-uniffi --no-default-features --features flow --example share_oracle > target/uniffi-kotlin-flow-smoke/share-oracle.json
    CARGO_TARGET_DIR=target cargo +1.97.0 run -p cribra-uniffi --no-default-features --features flow --example scanner_oracle > target/uniffi-kotlin-flow-smoke/scanner-oracle.json
    bindings/kotlin/gradlew -p bindings/kotlin --console=plain runFlow runFlowConformance

# Build the Fuse native library, generate the Kotlin/JVM binding and the
# direct-Core oracle output under target/, then run the Fuse parallel-scanning
# smoke against the real cdylib (needs JDK 17).
uniffi-kotlin-fuse-smoke:
    case "$(uname -s)" in Darwin) library="libcribra_uniffi.dylib" ;; Linux) library="libcribra_uniffi.so" ;; MINGW*|MSYS*|CYGWIN*) library="cribra_uniffi.dll" ;; *) echo "uniffi-kotlin-fuse-smoke: unsupported host $(uname -s)" >&2; exit 2 ;; esac; \
    mkdir -p target/uniffi-kotlin-fuse-smoke && \
    CARGO_TARGET_DIR=target cargo +1.97.0 build -p cribra-uniffi --no-default-features --features fuse --lib && \
    CARGO_TARGET_DIR=target cargo +1.97.0 run -p cribra-uniffi --no-default-features --features bindgen,fuse --bin cribra-uniffi-bindgen -- generate --library "target/debug/$library" --language kotlin --out-dir target/uniffi-kotlin-fuse-smoke/generated && \
    CARGO_TARGET_DIR=target cargo +1.97.0 run -p cribra-uniffi --no-default-features --features fuse --example parallel_oracle > target/uniffi-kotlin-fuse-smoke/parallel-oracle.json
    CARGO_TARGET_DIR=target cargo +1.97.0 run -p cribra-uniffi --no-default-features --features fuse --example scanner_oracle > target/uniffi-kotlin-fuse-smoke/scanner-oracle.json
    bindings/kotlin/gradlew -p bindings/kotlin --console=plain runFuse runFuseConformance

# Regenerate the Core, Flow and Fuse Kotlin bindings and verify that Core-owned
# parallel scanning is exported only by the Fuse profile.
uniffi-kotlin-parallel-gating:
    case "$(uname -s)" in Darwin) library="libcribra_uniffi.dylib" ;; Linux) library="libcribra_uniffi.so" ;; MINGW*|MSYS*|CYGWIN*) library="cribra_uniffi.dll" ;; *) echo "uniffi-kotlin-parallel-gating: unsupported host $(uname -s)" >&2; exit 2 ;; esac; \
    mkdir -p target/uniffi-kotlin-smoke/generated target/uniffi-kotlin-flow-smoke/generated target/uniffi-kotlin-fuse-smoke/generated && \
    CARGO_TARGET_DIR=target cargo +1.97.0 build -p cribra-uniffi --no-default-features --features core --lib && \
    CARGO_TARGET_DIR=target cargo +1.97.0 run -p cribra-uniffi --no-default-features --features bindgen,core --bin cribra-uniffi-bindgen -- generate --library "target/debug/$library" --language kotlin --out-dir target/uniffi-kotlin-smoke/generated && \
    CARGO_TARGET_DIR=target cargo +1.97.0 build -p cribra-uniffi --no-default-features --features flow --lib && \
    CARGO_TARGET_DIR=target cargo +1.97.0 run -p cribra-uniffi --no-default-features --features bindgen,flow --bin cribra-uniffi-bindgen -- generate --library "target/debug/$library" --language kotlin --out-dir target/uniffi-kotlin-flow-smoke/generated && \
    CARGO_TARGET_DIR=target cargo +1.97.0 build -p cribra-uniffi --no-default-features --features fuse --lib && \
    CARGO_TARGET_DIR=target cargo +1.97.0 run -p cribra-uniffi --no-default-features --features bindgen,fuse --bin cribra-uniffi-bindgen -- generate --library "target/debug/$library" --language kotlin --out-dir target/uniffi-kotlin-fuse-smoke/generated
    test -s target/uniffi-kotlin-smoke/generated/uniffi/cribra_uniffi/cribra_uniffi.kt && ! grep -Eq 'parallelScan|parallel_scan' target/uniffi-kotlin-smoke/generated/uniffi/cribra_uniffi/cribra_uniffi.kt
    test -s target/uniffi-kotlin-flow-smoke/generated/uniffi/cribra_uniffi/cribra_uniffi.kt && ! grep -Eq 'parallelScan|parallel_scan' target/uniffi-kotlin-flow-smoke/generated/uniffi/cribra_uniffi/cribra_uniffi.kt
    test -s target/uniffi-kotlin-fuse-smoke/generated/uniffi/cribra_uniffi/cribra_uniffi.kt && grep -Eq 'parallelScan|parallel_scan' target/uniffi-kotlin-fuse-smoke/generated/uniffi/cribra_uniffi/cribra_uniffi.kt

# Generate Core, Flow and Fuse Kotlin bindings from scratch, package each as an
# independent profile JAR under bindings/kotlin/build/libs, then verify contents,
# manifests, Java 17 classfiles, API gating and the single-profile classpath rule
# (needs JDK 17).
uniffi-kotlin-jars:
    case "$(uname -s)" in Darwin) library="libcribra_uniffi.dylib" ;; Linux) library="libcribra_uniffi.so" ;; MINGW*|MSYS*|CYGWIN*) library="cribra_uniffi.dll" ;; *) echo "uniffi-kotlin-jars: unsupported host $(uname -s)" >&2; exit 2 ;; esac; \
    rm -rf target/uniffi-kotlin-smoke/generated target/uniffi-kotlin-flow-smoke/generated target/uniffi-kotlin-fuse-smoke/generated && \
    mkdir -p target/uniffi-kotlin-smoke target/uniffi-kotlin-flow-smoke target/uniffi-kotlin-fuse-smoke target/uniffi-kotlin-jars && \
    CARGO_TARGET_DIR=target cargo +1.97.0 build -p cribra-uniffi --no-default-features --features core --lib && \
    CARGO_TARGET_DIR=target cargo +1.97.0 run -p cribra-uniffi --no-default-features --features bindgen,core --bin cribra-uniffi-bindgen -- generate --library "target/debug/$library" --language kotlin --out-dir target/uniffi-kotlin-smoke/generated && \
    CARGO_TARGET_DIR=target cargo +1.97.0 build -p cribra-uniffi --no-default-features --features flow --lib && \
    CARGO_TARGET_DIR=target cargo +1.97.0 run -p cribra-uniffi --no-default-features --features bindgen,flow --bin cribra-uniffi-bindgen -- generate --library "target/debug/$library" --language kotlin --out-dir target/uniffi-kotlin-flow-smoke/generated && \
    CARGO_TARGET_DIR=target cargo +1.97.0 build -p cribra-uniffi --no-default-features --features fuse --lib && \
    CARGO_TARGET_DIR=target cargo +1.97.0 run -p cribra-uniffi --no-default-features --features bindgen,fuse --bin cribra-uniffi-bindgen -- generate --library "target/debug/$library" --language kotlin --out-dir target/uniffi-kotlin-fuse-smoke/generated
    bindings/kotlin/gradlew -p bindings/kotlin --console=plain clean
    bindings/kotlin/gradlew -p bindings/kotlin --console=plain profileJars verifyProfileJars
    javap -classpath bindings/kotlin/build/libs/cribra-kotlin-core-0.7.2.jar uniffi.cribra_uniffi.Scanner > target/uniffi-kotlin-jars/core-scanner.txt && ! grep -q parallelScan target/uniffi-kotlin-jars/core-scanner.txt
    javap -classpath bindings/kotlin/build/libs/cribra-kotlin-flow-0.7.2.jar uniffi.cribra_uniffi.Scanner > target/uniffi-kotlin-jars/flow-scanner.txt && ! grep -q parallelScan target/uniffi-kotlin-jars/flow-scanner.txt
    javap -classpath bindings/kotlin/build/libs/cribra-kotlin-fuse-0.7.2.jar uniffi.cribra_uniffi.Scanner > target/uniffi-kotlin-jars/fuse-scanner.txt && grep -q parallelScan target/uniffi-kotlin-jars/fuse-scanner.txt
    java --class-path bindings/kotlin/build/libs/cribra-kotlin-core-0.7.2.jar bindings/kotlin/verify/ClasspathCheck.java core single
    java --class-path bindings/kotlin/build/libs/cribra-kotlin-flow-0.7.2.jar bindings/kotlin/verify/ClasspathCheck.java flow single
    java --class-path bindings/kotlin/build/libs/cribra-kotlin-fuse-0.7.2.jar bindings/kotlin/verify/ClasspathCheck.java fuse single
    java --class-path bindings/kotlin/build/libs/cribra-kotlin-core-0.7.2.jar:bindings/kotlin/build/libs/cribra-kotlin-flow-0.7.2.jar bindings/kotlin/verify/ClasspathCheck.java core mixed
    java --class-path bindings/kotlin/build/libs/cribra-kotlin-flow-0.7.2.jar:bindings/kotlin/build/libs/cribra-kotlin-fuse-0.7.2.jar bindings/kotlin/verify/ClasspathCheck.java flow mixed
    java --class-path bindings/kotlin/build/libs/cribra-kotlin-core-0.7.2.jar:bindings/kotlin/build/libs/cribra-kotlin-fuse-0.7.2.jar bindings/kotlin/verify/ClasspathCheck.java fuse mixed
    @echo "Profile JARs:"
    @ls -l bindings/kotlin/build/libs/cribra-kotlin-*-0.7.2.jar

# Build reproducible, host-specific Core, Flow/Share and Fuse Kotlin/JVM
# distribution bundles (profile JAR + JNA + native library + metadata + README)
# under target/uniffi-kotlin-bundles (needs JDK 17 and Python 3.11+).
uniffi-kotlin-bundles python="python3":
    "{{python}}" bindings/kotlin/build_bundle.py

# Qualify the existing ZIP bundles with isolated JVM consumers; never rebuild them.
uniffi-kotlin-bundle-consumer python="python3" dist="target/uniffi-kotlin-bundles/dist":
    "{{python}}" bindings/kotlin/qualify_bundle_consumer.py --dist "{{dist}}"

# Build the native library and generate Swift bindings under target/.
uniffi-swift-bindings features="core":
    test "$(uname -s)" = Darwin && test "$(uname -m)" = arm64
    CARGO_TARGET_DIR=target cargo +1.97.0 build -p cribra-uniffi --no-default-features --features "{{features}}" --lib
    CARGO_TARGET_DIR=target cargo +1.97.0 run -p cribra-uniffi --no-default-features --features "bindgen,{{features}}" --bin cribra-uniffi-bindgen -- generate --library target/debug/libcribra_uniffi.dylib --language swift --out-dir target/uniffi-swift-smoke/generated

# Compile a Swift 6 module and executable, then run against the real cdylib.
uniffi-swift-smoke features="core": (uniffi-swift-bindings features)
    swiftc --version | grep 'Swift version 6.4'
    CARGO_TARGET_DIR=target cargo +1.97.0 run -p cribra-uniffi --no-default-features --features "{{features}}" --example scanner_oracle > target/uniffi-swift-smoke/core-oracle.json
    swiftc -swift-version 6 -module-cache-path target/uniffi-swift-smoke/module-cache -emit-library -emit-module -module-name cribra_uniffi target/uniffi-swift-smoke/generated/cribra_uniffi.swift -I target/uniffi-swift-smoke/generated -Xcc -fmodule-map-file=target/uniffi-swift-smoke/generated/cribra_uniffiFFI.modulemap -L target/debug -lcribra_uniffi -Xlinker -rpath -Xlinker @loader_path/../debug -emit-module-path target/uniffi-swift-smoke/cribra_uniffi.swiftmodule -o target/uniffi-swift-smoke/libcribra_uniffi_swift.dylib
    swiftc -swift-version 6 -module-cache-path target/uniffi-swift-smoke/module-cache -parse-as-library -I target/uniffi-swift-smoke -I target/uniffi-swift-smoke/generated -Xcc -fmodule-map-file=target/uniffi-swift-smoke/generated/cribra_uniffiFFI.modulemap crates/cribra-uniffi/tests/swift/Smoke.swift -L target/uniffi-swift-smoke -lcribra_uniffi_swift -Xlinker -rpath -Xlinker @loader_path -o target/uniffi-swift-smoke/smoke
    target/uniffi-swift-smoke/smoke "$(cargo +1.97.0 pkgid -p cribra | sed 's/.*#//')" crates/cribra-uniffi/tests/fixtures/scanner.json target/uniffi-swift-smoke/core-oracle.json crates/cribra-uniffi/tests/fixtures/configurations.json crates/cribra-uniffi/tests/fixtures/incremental.json

# Generate the feature-gated Flow interface separately from the core-only smoke.
uniffi-swift-flow-bindings features="flow":
    test "$(uname -s)" = Darwin && test "$(uname -m)" = arm64
    CARGO_TARGET_DIR=target cargo +1.97.0 build -p cribra-uniffi --no-default-features --features "{{features}}" --lib
    CARGO_TARGET_DIR=target cargo +1.97.0 run -p cribra-uniffi --no-default-features --features "bindgen,{{features}}" --bin cribra-uniffi-bindgen -- generate --library target/debug/libcribra_uniffi.dylib --language swift --out-dir target/uniffi-flow-smoke/generated

# Compile and run real Swift Flow transformations against a direct Core oracle.
uniffi-swift-flow-smoke features="flow": (uniffi-swift-flow-bindings features)
    swiftc --version | grep 'Swift version 6.4'
    CARGO_TARGET_DIR=target cargo +1.97.0 run -p cribra-uniffi --no-default-features --features "{{features}}" --example flow_oracle > target/uniffi-flow-smoke/core-oracle.json
    swiftc -swift-version 6 -module-cache-path target/uniffi-flow-smoke/module-cache -emit-library -emit-module -module-name cribra_uniffi target/uniffi-flow-smoke/generated/cribra_uniffi.swift -I target/uniffi-flow-smoke/generated -Xcc -fmodule-map-file=target/uniffi-flow-smoke/generated/cribra_uniffiFFI.modulemap -L target/debug -lcribra_uniffi -Xlinker -rpath -Xlinker @loader_path/../debug -emit-module-path target/uniffi-flow-smoke/cribra_uniffi.swiftmodule -o target/uniffi-flow-smoke/libcribra_uniffi_swift.dylib
    swiftc -swift-version 6 -module-cache-path target/uniffi-flow-smoke/module-cache -parse-as-library -I target/uniffi-flow-smoke -I target/uniffi-flow-smoke/generated -Xcc -fmodule-map-file=target/uniffi-flow-smoke/generated/cribra_uniffiFFI.modulemap crates/cribra-uniffi/tests/swift/FlowSmoke.swift -L target/uniffi-flow-smoke -lcribra_uniffi_swift -Xlinker -rpath -Xlinker @loader_path -o target/uniffi-flow-smoke/smoke
    target/uniffi-flow-smoke/smoke crates/cribra-uniffi/tests/fixtures/flow.json target/uniffi-flow-smoke/core-oracle.json crates/cribra-uniffi/tests/fixtures/keyed-flow.json crates/cribra-uniffi/tests/fixtures/keyed-options.json

# Verify in-memory Share Bundles using real Swift and a direct Core oracle.
uniffi-swift-share-smoke features="flow": (uniffi-swift-flow-smoke features)
    CARGO_TARGET_DIR=target cargo +1.97.0 run -p cribra-uniffi --no-default-features --features "{{features}}" --example share_oracle > target/uniffi-flow-smoke/share-oracle.json
    swiftc -swift-version 6 -module-cache-path target/uniffi-flow-smoke/module-cache -parse-as-library -I target/uniffi-flow-smoke -I target/uniffi-flow-smoke/generated -Xcc -fmodule-map-file=target/uniffi-flow-smoke/generated/cribra_uniffiFFI.modulemap crates/cribra-uniffi/tests/swift/ShareSmoke.swift -L target/uniffi-flow-smoke -lcribra_uniffi_swift -Xlinker -rpath -Xlinker @loader_path -o target/uniffi-flow-smoke/share-smoke
    target/uniffi-flow-smoke/share-smoke crates/cribra-uniffi/tests/fixtures/share.json crates/cribra-uniffi/tests/fixtures/flow.json target/uniffi-flow-smoke/share-oracle.json

# Compile and execute the fuse-only parallel scanner against a direct Core oracle.
uniffi-swift-parallel-smoke: (uniffi-swift-flow-bindings "fuse")
    swiftc --version | grep 'Swift version 6.4'
    CARGO_TARGET_DIR=target cargo +1.97.0 run -p cribra-uniffi --no-default-features --features fuse --example parallel_oracle > target/uniffi-flow-smoke/parallel-oracle.json
    swiftc -swift-version 6 -module-cache-path target/uniffi-flow-smoke/module-cache -emit-library -emit-module -module-name cribra_uniffi target/uniffi-flow-smoke/generated/cribra_uniffi.swift -I target/uniffi-flow-smoke/generated -Xcc -fmodule-map-file=target/uniffi-flow-smoke/generated/cribra_uniffiFFI.modulemap -L target/debug -lcribra_uniffi -Xlinker -rpath -Xlinker @loader_path/../debug -emit-module-path target/uniffi-flow-smoke/cribra_uniffi.swiftmodule -o target/uniffi-flow-smoke/libcribra_uniffi_swift.dylib
    swiftc -swift-version 6 -module-cache-path target/uniffi-flow-smoke/module-cache -parse-as-library -I target/uniffi-flow-smoke -I target/uniffi-flow-smoke/generated -Xcc -fmodule-map-file=target/uniffi-flow-smoke/generated/cribra_uniffiFFI.modulemap crates/cribra-uniffi/tests/swift/ParallelSmoke.swift -L target/uniffi-flow-smoke -lcribra_uniffi_swift -Xlinker -rpath -Xlinker @loader_path -o target/uniffi-flow-smoke/parallel-smoke
    target/uniffi-flow-smoke/parallel-smoke crates/cribra-uniffi/tests/fixtures/parallel.json target/uniffi-flow-smoke/parallel-oracle.json

# Regenerate core/flow bindings and verify that parallel scanning stays fuse-only.
uniffi-swift-parallel-gating:
    just uniffi-swift-bindings
    test -s target/uniffi-swift-smoke/generated/cribra_uniffi.swift && ! rg -n 'parallelScan|parallel_scan' target/uniffi-swift-smoke/generated/cribra_uniffi.swift
    just uniffi-swift-flow-bindings flow
    test -s target/uniffi-flow-smoke/generated/cribra_uniffi.swift && ! rg -n 'parallelScan|parallel_scan' target/uniffi-flow-smoke/generated/cribra_uniffi.swift

# Check the complete generated Swift API/DTO/error contract for all profiles.
uniffi-swift-api-contract:
    mkdir -p target/uniffi-swift-smoke
    swiftc -swift-version 6 -parse-as-library -module-cache-path target/uniffi-swift-smoke/module-cache crates/cribra-uniffi/tests/swift/APIContract.swift -o target/uniffi-swift-smoke/api-contract
    just uniffi-swift-bindings core
    target/uniffi-swift-smoke/api-contract core crates/cribra-uniffi/tests/fixtures/api-contract.json target/uniffi-swift-smoke/generated/cribra_uniffi.swift
    just uniffi-swift-flow-bindings flow
    target/uniffi-swift-smoke/api-contract flow crates/cribra-uniffi/tests/fixtures/api-contract.json target/uniffi-flow-smoke/generated/cribra_uniffi.swift
    just uniffi-swift-flow-bindings fuse
    target/uniffi-swift-smoke/api-contract fuse crates/cribra-uniffi/tests/fixtures/api-contract.json target/uniffi-flow-smoke/generated/cribra_uniffi.swift
    CARGO_TARGET_DIR=target cargo +1.97.0 build -p cribra-uniffi --no-default-features --lib
    CARGO_TARGET_DIR=target cargo +1.97.0 run -p cribra-uniffi --no-default-features --features bindgen --bin cribra-uniffi-bindgen -- generate --library target/debug/libcribra_uniffi.dylib --language swift --out-dir target/uniffi-profile-none/generated
    target/uniffi-swift-smoke/api-contract none crates/cribra-uniffi/tests/fixtures/api-contract.json target/uniffi-profile-none/generated/cribra_uniffi.swift

# Build the dependency-free Rust driver for Apple distribution workflows.
_swift-apple-tool:
    mkdir -p target/swift-apple
    tool_dir="$(mktemp -d target/swift-apple/tool.XXXXXX)" && rustc +1.97.0 --edition 2024 bindings/swift/tools/apple.rs -o "$tool_dir/apple" && mv -f "$tool_dir/apple" target/swift-apple/apple && rmdir "$tool_dir"

# Build all three static Apple slices and publish one coherent Swift Package profile.
swift-apple profile="core": _swift-apple-tool
    target/swift-apple/apple "{{justfile_directory()}}" build "{{profile}}"

# Inspect architecture/platform tags and XCFramework entries without rebuilding.
swift-apple-inspect profile="core": _swift-apple-tool
    target/swift-apple/apple "{{justfile_directory()}}" inspect "{{profile}}"

# Copy the distribution and consumer outside this checkout; run macOS and compile iOS Simulator.
swift-apple-consumer profile="core" directory="": _swift-apple-tool
    target/swift-apple/apple "{{justfile_directory()}}" consumer "{{profile}}" "{{directory}}"

# Stage and qualify the unpublished Fuse remote SwiftPM distribution; never publishes.
swift-apple-dist profile="fuse": _swift-apple-tool
    target/swift-apple/apple "{{justfile_directory()}}" dist "{{profile}}"

# Test the dependency-free Swift distribution packaging validators.
swift-apple-packaging-test:
    mkdir -p target/swift-apple
    tool_dir="$(mktemp -d target/swift-apple/test.XXXXXX)" && rustc +1.97.0 --edition 2024 --test bindings/swift/tools/apple.rs -o "$tool_dir/apple-tests" && "$tool_dir/apple-tests" && rm -rf "$tool_dir"

# Generate and validate Swift-DocC API documentation for Core, Flow or Fuse.
swift-docc profile="all":
    bash bindings/swift/scripts/generate-docc.sh "{{profile}}"

# Generate Core Swift-DocC with the versioned public hosting base path.
swift-docc-core:
    bash bindings/swift/scripts/generate-docc.sh core --hosting-base-path /api/0.7.2/swift/core

# Stage generated Core Swift-DocC assets into Astro's ignored build inputs.
stage-swift-docc-core:
    node website/scripts/stage-swift-docc.mjs --profile core --version 0.7.2

# Generate, stage, and build the website with the Core API reference at /api/0.7.2/swift/core/.
website-build-swift-core:
    just swift-docc-core
    just stage-swift-docc-core
    just website-build

# Check, lint, and build the native C adapter artifact skeleton.
capi:
    cargo check -p cribra-capi
    cargo clippy -p cribra-capi --all-targets -- -D warnings
    cargo build -p cribra-capi

# Run benchmarks for the native C adapter.
capi-bench:
	cargo bench -p cribra-capi --bench abi

# Generate the committed native C header from the Rust ABI adapter.
capi-header:
    mkdir -p include
    cbindgen --config crates/cribra-capi/cbindgen.toml --crate cribra-capi --output include/cribra.h

# Verify that regenerating the native C header is deterministic and leaves the
# committed header unchanged.
capi-header-check:
    mkdir -p target/c-smoke
    cp include/cribra.h target/c-smoke/cribra.h.expected
    cbindgen --config crates/cribra-capi/cbindgen.toml --crate cribra-capi --output target/c-smoke/cribra.h.generated
    diff -u target/c-smoke/cribra.h.expected target/c-smoke/cribra.h.generated

# Build and run the real C consumer against the native dynamic library.
#
# macOS and Linux use an executable rpath so the smoke test does not require
# DYLD_LIBRARY_PATH / LD_LIBRARY_PATH. Windows is exercised by CI.
capi-smoke: capi-header
    cargo build -p cribra-capi
    mkdir -p target/c-smoke
    if [ "$(uname -s)" = "Darwin" ]; then \
        clang -std=c11 -Wall -Wextra -Werror -Iinclude \
          crates/cribra-capi/tests/c/smoke.c \
          -Ltarget/debug -lcribra_capi \
          -Wl,-rpath,@loader_path/../debug \
          -o target/c-smoke/cribra-smoke; \
    elif [ "$(uname -s)" = "Linux" ]; then \
        cc -std=c11 -Wall -Wextra -Werror -Iinclude \
          crates/cribra-capi/tests/c/smoke.c \
          -Ltarget/debug -lcribra_capi \
          -Wl,-rpath,'$ORIGIN/../debug' \
          -o target/c-smoke/cribra-smoke; \
    else \
        echo "capi-smoke local recipe currently supports macOS and Linux; Windows is covered in CI"; \
        exit 2; \
    fi
    target/c-smoke/cribra-smoke

# Verify that the generated header and the native dynamic library expose the
# same public `cribra_*` function set on macOS/Linux.
capi-symbols: capi-header
    cargo build -p cribra-capi
    sh crates/cribra-capi/tests/c/check-capi-symbols.sh

# Build the static ABI artifact and verify that it is produced.
#
# Full static linking is platform-specific because Rust staticlibs require
# target-native system libraries at the final link step; cross-platform static
# linking is therefore validated in CI rather than guessed here.
capi-static:
    cargo build -p cribra-capi
    test -f target/debug/libcribra_capi.a

# Run the complete local native-C interoperability smoke surface.
capi-smoke-all: capi-header-check capi-smoke capi-symbols capi-static

# Check the reusable JS/WASM adapter.
wasm-adapter-check:
    cargo check -p cribra-wasm --target wasm32-unknown-unknown
    cargo clippy -p cribra-wasm --target wasm32-unknown-unknown --all-targets -- -D warnings

# Build the reusable JS/WASM adapter.
wasm-adapter-build:
    cargo build -p cribra-wasm --target wasm32-unknown-unknown --release

# Generate browser-native ES module glue, TypeScript declarations, and WASM.
wasm-adapter-bindgen:
    mkdir -p target/wasm
    wasm-bindgen \
      --target web \
      --out-dir target/wasm \
      --out-name cribra \
      target/wasm32-unknown-unknown/release/cribra_wasm.wasm

# Validate the complete reusable JS/WASM adapter artifact.
wasm-adapter: wasm-adapter-check wasm-adapter-build wasm-adapter-bindgen
    test -s target/wasm/cribra.js
    test -s target/wasm/cribra.d.ts
    test -s target/wasm/cribra_bg.wasm
    test -s target/wasm/cribra_bg.wasm.d.ts

# Produce the reproducible Binaryen comparison matrix from one identical
# wasm-bindgen baseline. These are benchmark inputs, not release artifacts.
wasm-opt-build: wasm-adapter
    mkdir -p target/wasm-opt
    cp target/wasm/cribra_bg.wasm target/wasm-opt/cribra_bg.base.wasm
    wasm-opt -Os target/wasm/cribra_bg.wasm -o target/wasm-opt/cribra_bg.os.wasm
    wasm-opt -Oz target/wasm/cribra_bg.wasm -o target/wasm-opt/cribra_bg.oz.wasm
    wasm-opt -O3 target/wasm/cribra_bg.wasm -o target/wasm-opt/cribra_bg.o3.wasm

# Validate every Binaryen comparison artifact. Prefer a linked WABT executable,
# but support an unlinked Apple Silicon Homebrew installation when WABT and
# Binaryen expose conflicting tool names.
wasm-opt-validate: wasm-opt-build
    validator="$(command -v wasm-validate || true)"; \
    if [ -z "$validator" ] && [ -x /opt/homebrew/opt/wabt/bin/wasm-validate ]; then \
      validator=/opt/homebrew/opt/wabt/bin/wasm-validate; \
    fi; \
    if [ -z "$validator" ]; then \
      echo "wasm-validate not found; install WABT or expose wasm-validate in PATH"; \
      exit 2; \
    fi; \
    for file in \
      target/wasm-opt/cribra_bg.base.wasm \
      target/wasm-opt/cribra_bg.os.wasm \
      target/wasm-opt/cribra_bg.oz.wasm \
      target/wasm-opt/cribra_bg.o3.wasm; do \
        "$validator" "$file"; \
    done

# Print raw Binaryen comparison sizes.
wasm-opt-size: wasm-opt-build
    wc -c \
      target/wasm-opt/cribra_bg.base.wasm \
      target/wasm-opt/cribra_bg.os.wasm \
      target/wasm-opt/cribra_bg.oz.wasm \
      target/wasm-opt/cribra_bg.o3.wasm

# Produce transfer-size evidence for the comparison matrix.
wasm-opt-compress: wasm-opt-build
    gzip -9 -c target/wasm-opt/cribra_bg.base.wasm > target/wasm-opt/cribra_bg.base.wasm.gz
    gzip -9 -c target/wasm-opt/cribra_bg.os.wasm > target/wasm-opt/cribra_bg.os.wasm.gz
    gzip -9 -c target/wasm-opt/cribra_bg.oz.wasm > target/wasm-opt/cribra_bg.oz.wasm.gz
    gzip -9 -c target/wasm-opt/cribra_bg.o3.wasm > target/wasm-opt/cribra_bg.o3.wasm.gz
    brotli -f -q 11 target/wasm-opt/cribra_bg.base.wasm -o target/wasm-opt/cribra_bg.base.wasm.br
    brotli -f -q 11 target/wasm-opt/cribra_bg.os.wasm -o target/wasm-opt/cribra_bg.os.wasm.br
    brotli -f -q 11 target/wasm-opt/cribra_bg.oz.wasm -o target/wasm-opt/cribra_bg.oz.wasm.br
    brotli -f -q 11 target/wasm-opt/cribra_bg.o3.wasm -o target/wasm-opt/cribra_bg.o3.wasm.br
    wc -c target/wasm-opt/*.wasm.gz
    wc -c target/wasm-opt/*.wasm.br

# Reproduce the complete Binaryen validation/size comparison surface.
wasm-opt-prepare: wasm-opt-build wasm-opt-validate wasm-opt-size wasm-opt-compress

# Validate the exact generated TypeScript/JS/WASM production artifact surface.
wasm-package-check: wasm-production
    node crates/cribra-wasm/tests/package/validate.mjs

# Build the single production WASM profile selected by the v0.4.2 browser
# benchmark gate: Binaryen -Oz.
wasm-production: wasm-adapter
    mkdir -p target/wasm-production
    cp target/wasm/cribra.js target/wasm-production/cribra.js
    cp target/wasm/cribra.d.ts target/wasm-production/cribra.d.ts
    cp target/wasm/cribra_bg.wasm.d.ts target/wasm-production/cribra_bg.wasm.d.ts
    wasm-opt -Oz target/wasm/cribra_bg.wasm -o target/wasm-production/cribra_bg.wasm
    printf '%s\n' '{"type":"module"}' > target/wasm-production/package.json
    test -s target/wasm-production/cribra.js
    test -s target/wasm-production/cribra.d.ts
    test -s target/wasm-production/cribra_bg.wasm
    test -s target/wasm-production/cribra_bg.wasm.d.ts

# Generate the Rust-native semantic oracle for the WASM parity gate.
wasm-parity-oracle:
    rm -rf target/wasm-parity
    mkdir -p target/wasm-parity
    cargo run -p cribra-wasm --example parity_oracle

# Prepare the production artifact and Rust semantic oracle.
wasm-parity-prepare: wasm-production wasm-parity-oracle
    test -s target/wasm-parity/oracle.json
    test -s target/wasm-production/cribra.js
    test -s target/wasm-production/cribra_bg.wasm

# Compare the production Binaryen -Oz WASM adapter against the Rust oracle.
wasm-parity: wasm-parity-prepare
    node crates/cribra-wasm/tests/parity/parity.mjs

# Prepare isolated base/-Os/-Oz/-O3 directories for the real-browser
# regression benchmark harness.
wasm-bench-prepare: wasm-opt-build
    rm -rf target/wasm-bench
    mkdir -p \
      target/wasm-bench/base \
      target/wasm-bench/os \
      target/wasm-bench/oz \
      target/wasm-bench/o3
    for variant in base os oz o3; do \
      cp target/wasm/cribra.js "target/wasm-bench/$variant/cribra.js"; \
    done
    cp target/wasm-opt/cribra_bg.base.wasm target/wasm-bench/base/cribra_bg.wasm
    cp target/wasm-opt/cribra_bg.os.wasm target/wasm-bench/os/cribra_bg.wasm
    cp target/wasm-opt/cribra_bg.oz.wasm target/wasm-bench/oz/cribra_bg.wasm
    cp target/wasm-opt/cribra_bg.o3.wasm target/wasm-bench/o3/cribra_bg.wasm

# Serve the browser benchmark harness with dependency-free Node.js.
wasm-bench-serve:
    node crates/cribra-wasm/benches/web/serve.mjs

# Remove Binaryen comparison, production, and browser-benchmark artifacts only.
wasm-opt-clean:
    rm -rf target/wasm-opt target/wasm-production target/wasm-bench

# Remove all generated reusable WASM artifacts while preserving unrelated Cargo
# outputs.
wasm-clean:
    rm -rf target/wasm target/wasm-opt target/wasm-production target/wasm-bench target/wasm-parity

# Validate the complete reusable WASM release surface.
wasm-release-gate:
    just wasm-adapter
    just wasm-parity
    just wasm-package-check

# Run the default test surface.
test:
    cargo test

# Run tests with Serde enabled.
test-serde:
    cargo test --features serde

# Run the complete native test surface.
test-all:
    cargo test --all-features

# Run doctests.
test-doc:
    cargo test --doc

# Run Clippy with warnings denied.
clippy:
    cargo clippy --all-targets --all-features -- -D warnings

# Build default-feature documentation.
doc:
    cargo doc --no-default-features --no-deps

# Build full-feature documentation.
doc-all:
    cargo doc --all-features --no-deps

# Check the supported browser/WASM default contract.
wasm:
    cargo check --target wasm32-unknown-unknown --no-default-features

# Check the supported browser/WASM Serde contract.
wasm-serde:
    cargo check --target wasm32-unknown-unknown --no-default-features --features serde

# Check and test against Cribra's declared MSRV.
msrv:
    cargo +1.97.0 check
    cargo +1.97.0 check --all-features
    cargo +1.97.0 test
    cargo +1.97.0 test --all-features

# Run the local RustSec audit. Requires `cargo-audit`.
audit:
    cargo audit

# Verify the crate package from a clean working tree.
package:
    cargo package

# Verify the crate package while release files are still uncommitted.
package-dirty:
    cargo package --allow-dirty

# Perform a crates.io publication dry run from a clean working tree.
publish-dry-run:
    cargo publish --dry-run

# Perform a publication dry run while release files are still uncommitted.
publish-dry-run-dirty:
    cargo publish --dry-run --allow-dirty

# Run the fast local quality gate used during normal development.
gate: fmt-check check-all capi test test-serde test-all test-doc clippy doc doc-all wasm wasm-serde
    git diff --check

# Run the release-oriented local gate. Requires `cargo-audit`.
release-gate: gate msrv audit package publish-dry-run

# Run the historical benchmark suite.
bench:
    cargo bench --bench scan --all-features

# Run the v0.2 semantic-path benchmark suite.
bench-v02:
    cargo bench --bench v02 --all-features

# Run diagnostic contextual matcher benchmarks.
bench-diagnostics:
    cargo bench --bench isolation --all-features
    cargo bench --bench contextual_tail --all-features

# Run every maintained benchmark suite.
bench-all: bench bench-v02 bench-diagnostics

# Remove Cargo build artifacts.
clean:
    cargo clean

# Measure owned edits plus exact incremental updates against edits plus full scans.
bench-incremental:
    cargo bench --bench incremental --all-features
