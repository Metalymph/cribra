#!/usr/bin/env python3
"""Build reproducible, profile-specific Cribra Kotlin/JVM distribution bundles.

Each bundle is self-contained for the host it was built on:

    lib/cribra-kotlin-<profile>-<version>.jar
    lib/jna-<version>.jar
    native/<platform native library>
    metadata/build-info.json
    metadata/SHA256SUMS
    README.md

The profile JARs come from ``just uniffi-kotlin-jars``-equivalent Gradle tasks
and stay thin: the native library is never embedded in a JAR. Every profile is
compiled in its own isolated ``CARGO_TARGET_DIR`` so a previous profile's
cdylib can never be packaged by accident. ZIP entries are sorted, timestamped
with the ZIP epoch and compressed with a fixed level, so a clean rebuild
produces the same bytes on the same host.
"""

from __future__ import annotations

from email.parser import BytesParser
import hashlib
import json
import os
import platform
import re
import shutil
import struct
import subprocess
import sys
import tomllib
from typing import NoReturn
import zipfile
from pathlib import Path

PROFILES = ("core", "flow", "fuse")
PROFILE_CLASSES = {"core": "CoreProfile", "flow": "FlowProfile", "fuse": "FuseProfile"}
PROFILE_TITLES = {"core": "Core", "flow": "Flow/Share", "fuse": "Fuse"}

CARGO = ("cargo", "+1.97.0")
RUST_TOOLCHAIN = "1.97.0"
JDK_BASELINE = 17

# ZIP epoch: the earliest timestamp the format can represent. Fixed so the
# archive never depends on when or where it was assembled.
ZIP_EPOCH = (1980, 1, 1, 0, 0, 0)
ZIP_COMPRESSION_LEVEL = 9

# Mach-O load commands used by the macOS LINKEDIT alignment pass.
MACHO_MAGIC_64_LE = 0xFEEDFACF
MACHO_PAGE_SIZE = 0x4000  # arm64 macOS segment alignment
LC_SYMTAB = 0x2
LC_DYSYMTAB = 0xB
LC_SEGMENT_64 = 0x19
LC_CODE_SIGNATURE = 0x1D
LC_DYLD_INFO_ONLY = 0x80000022
LC_LINKEDIT_OFFSET_COMMANDS = frozenset(
    {
        0x1D,  # LC_CODE_SIGNATURE
        0x1E,  # LC_SEGMENT_SPLIT_INFO
        0x26,  # LC_FUNCTION_STARTS
        0x29,  # LC_DATA_IN_CODE
        0x2B,  # LC_DYLIB_CODE_SIGN_DRS
        0x2E,  # LC_LINKER_OPTIMIZATION_HINT
        0x80000033,  # LC_DYLD_EXPORTS_TRIE
        0x80000034,  # LC_DYLD_CHAINED_FIXUPS
    }
)
DYSYMTAB_OFFSET_INDICES = (6, 8, 10, 12, 14, 16)


def fail(message: str) -> NoReturn:
    raise SystemExit(f"error: {message}")


def run(args, root: Path, env: dict[str, str] | None = None) -> None:
    command = [str(arg) for arg in args]
    print("+", " ".join(command), flush=True)
    # Gradle's Windows wrapper is a batch launcher, not a native executable.
    batch = sys.platform == "win32" and Path(command[0]).suffix.lower() in (".bat", ".cmd")
    subprocess.run(command, cwd=root, env=env, check=True, shell=batch)


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def host_spec() -> dict[str, str]:
    """Return the current host, failing closed on unsupported platforms."""
    machine = platform.machine().lower()

    if sys.platform == "darwin":
        if machine not in ("arm64", "aarch64"):
            fail(f"unsupported macOS architecture: {platform.machine()} (expected arm64)")
        return {
            "os": "macos",
            "arch": "aarch64",
            "platform": "macos-arm64",
            "rustTarget": "aarch64-apple-darwin",
            "nativeLibrary": "libcribra_uniffi.dylib",
        }

    if sys.platform.startswith("linux"):
        if machine not in ("x86_64", "amd64"):
            fail(f"unsupported Linux architecture: {platform.machine()} (expected x86_64)")
        return {
            "os": "linux",
            "arch": "x86_64",
            "platform": "linux-x86_64",
            "rustTarget": "x86_64-unknown-linux-gnu",
            "nativeLibrary": "libcribra_uniffi.so",
        }

    if sys.platform == "win32":
        if machine not in ("amd64", "x86_64"):
            fail(f"unsupported Windows architecture: {platform.machine()} (expected x86_64)")
        return {
            "os": "windows",
            "arch": "x86_64",
            "platform": "windows-x86_64",
            "rustTarget": "x86_64-pc-windows-msvc",
            "nativeLibrary": "cribra_uniffi.dll",
        }

    fail(f"unsupported host platform: {sys.platform}")


def read_versions(root: Path) -> dict[str, str]:
    """Read versions from the single source of truth for each component."""
    core_data = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))
    adapter_data = tomllib.loads(
        (root / "crates/cribra-uniffi/Cargo.toml").read_text(encoding="utf-8")
    )

    uniffi_spec = adapter_data["dependencies"]["uniffi"]
    uniffi_version = uniffi_spec["version"] if isinstance(uniffi_spec, dict) else uniffi_spec

    gradle = (root / "bindings/kotlin/build.gradle.kts").read_text(encoding="utf-8")
    kotlin_match = re.search(r'kotlin\("jvm"\)\s+version\s+"([^"]+)"', gradle)
    jna_match = re.search(r'val jna = "net\.java\.dev\.jna:jna:([^"]+)"', gradle)
    if kotlin_match is None:
        fail("could not read the Kotlin plugin version from build.gradle.kts")
    if jna_match is None:
        fail("could not read the JNA version from build.gradle.kts")

    return {
        "core": core_data["package"]["version"],
        "adapter": adapter_data["package"]["version"],
        "uniffi": uniffi_version.lstrip("="),
        "kotlin": kotlin_match.group(1),
        "jna": jna_match.group(1),
    }


def expected_files(profile: str, versions: dict[str, str], host: dict[str, str]) -> set[str]:
    return {
        "README.md",
        f"lib/cribra-kotlin-{profile}-{versions['core']}.jar",
        f"lib/jna-{versions['jna']}.jar",
        f"native/{host['nativeLibrary']}",
        "metadata/build-info.json",
        "metadata/SHA256SUMS",
    }


def relative_files(root: Path) -> list[str]:
    return sorted(path.relative_to(root).as_posix() for path in root.rglob("*") if path.is_file())


def macho_commands(data: bytes) -> list[tuple[int, int, int]]:
    if struct.unpack_from("<I", data, 0)[0] != MACHO_MAGIC_64_LE:
        fail("expected a 64-bit little-endian Mach-O dylib")
    ncmds = struct.unpack_from("<I", data, 16)[0]
    commands = []
    offset = 32
    for _ in range(ncmds):
        cmd, size = struct.unpack_from("<II", data, offset)
        if size < 8:
            fail(f"malformed Mach-O load command at {offset:#x}")
        commands.append((offset, cmd, size))
        offset += size
    return commands


def align_macho_linkedit(library: Path) -> None:
    """Pad a Mach-O dylib so dyld accepts its LINKEDIT structures.

    The system linker can place the symbol string table four bytes early when
    the indirect symbol table has an odd number of entries. The current macOS
    dyld rejects that library even though it is validly signed, and the same
    happens to the code signature once the string table moves. Pad both
    structures, shift every affected load-command offset, and re-sign ad-hoc.
    The pass is a no-op when the linker already aligned both structures, and it
    is deterministic for a given input.
    """
    data = bytearray(library.read_bytes())
    commands = macho_commands(data)

    symtab = next((entry for entry in commands if entry[1] == LC_SYMTAB), None)
    signature = next((entry for entry in commands if entry[1] == LC_CODE_SIGNATURE), None)
    if symtab is None or signature is None:
        fail(f"{library.name}: missing LC_SYMTAB or LC_CODE_SIGNATURE")

    stroff = struct.unpack_from("<I", data, symtab[0] + 16)[0]
    sigoff = struct.unpack_from("<I", data, signature[0] + 8)[0]

    pad_strings = (-stroff) % 8
    pad_signature = (-(sigoff + pad_strings)) % 16
    if pad_strings == 0 and pad_signature == 0:
        return

    def remap(value: int) -> int:
        if value < stroff:
            return value
        shifted = value + pad_strings
        if value >= sigoff:
            shifted += pad_signature
        return shifted

    if pad_signature:
        data[sigoff:sigoff] = b"\x00" * pad_signature
    if pad_strings:
        data[stroff:stroff] = b"\x00" * pad_strings

    for offset, cmd, _ in commands:
        if cmd == LC_SYMTAB:
            positions = [offset + 16]
        elif cmd == LC_DYSYMTAB:
            positions = [offset + 8 + index * 4 for index in DYSYMTAB_OFFSET_INDICES]
        elif cmd == LC_DYLD_INFO_ONLY:
            positions = [offset + 8 + index * 4 for index in range(0, 10, 2)]
        elif cmd in LC_LINKEDIT_OFFSET_COMMANDS:
            positions = [offset + 8]
        elif cmd == LC_SEGMENT_64:
            name = bytes(data[offset + 8:offset + 24]).rstrip(b"\x00").decode("ascii")
            fileoff, filesize = struct.unpack_from("<QQ", data, offset + 40)
            if name == "__LINKEDIT":
                vmsize = struct.unpack_from("<Q", data, offset + 32)[0]
                new_filesize = filesize + pad_strings + pad_signature
                new_vmsize = max(
                    vmsize,
                    (new_filesize + MACHO_PAGE_SIZE - 1) // MACHO_PAGE_SIZE * MACHO_PAGE_SIZE,
                )
                struct.pack_into("<Q", data, offset + 32, new_vmsize)
                struct.pack_into("<Q", data, offset + 48, new_filesize)
            elif fileoff >= stroff:
                fail(f"{library.name}: unexpected segment {name} after the string table")
            continue
        else:
            continue

        for position in positions:
            value = struct.unpack_from("<I", data, position)[0]
            struct.pack_into("<I", data, position, remap(value))

    library.write_bytes(data)
    run(["codesign", "--force", "--sign", "-", library], library.parent)


def verify_macho_alignment(library: Path) -> None:
    data = library.read_bytes()
    commands = macho_commands(data)
    symtab = next(entry for entry in commands if entry[1] == LC_SYMTAB)
    signature = next(entry for entry in commands if entry[1] == LC_CODE_SIGNATURE)
    stroff = struct.unpack_from("<I", data, symtab[0] + 16)[0]
    sigoff = struct.unpack_from("<I", data, signature[0] + 8)[0]
    if stroff % 8:
        fail(f"{library.name}: string table is not 8-byte aligned")
    if sigoff % 16:
        fail(f"{library.name}: code signature is not 16-byte aligned")
    run(["codesign", "--verify", "--strict", library], library.parent)
    # Load in a fresh process: all three profiles share the same install name,
    # and dyld may otherwise reuse an already-loaded image from this process.
    run(
        [sys.executable, "-c", "import ctypes, sys; ctypes.CDLL(sys.argv[1])", library],
        library.parent,
    )


# ---- Native builds ----------------------------------------------------------


def build_native(
    root: Path, host: dict[str, str], profile: str, profile_dir: Path
) -> Path:
    """Build one profile in its own Cargo target directory and stage the cdylib.

    The pure profile library is copied before the bindgen run, because the
    bindgen build enables the extra ``bindgen`` feature and may otherwise
    replace the artifact whose exact feature set the bundle promises.
    """
    cargo_dir = profile_dir / "cargo"
    generated = profile_dir / "generated"
    native_dir = profile_dir / "native"

    for directory in (generated, native_dir):
        if directory.exists():
            shutil.rmtree(directory)
    generated.mkdir(parents=True)
    native_dir.mkdir(parents=True)

    env = dict(os.environ, CARGO_TARGET_DIR=str(cargo_dir))

    # Cargo records the absolute output path as the Mach-O install name (and
    # would record it in an ELF SONAME). Replace it with a relocatable name so
    # the shipped library never leaks a repository or build path.
    command = list(CARGO) + [
        "rustc" if host["os"] in ("macos", "linux") else "build",
        "--locked",
        "-p",
        "cribra-uniffi",
        "--no-default-features",
        "--features",
        profile,
        "--release",
        "--lib",
    ]
    if host["os"] == "macos":
        command += [
            "--",
            "-C",
            f"link-arg=-Wl,-install_name,@rpath/{host['nativeLibrary']}",
        ]
    elif host["os"] == "linux":
        command += ["--", "-C", f"link-arg=-Wl,-soname,{host['nativeLibrary']}"]

    run(command, root, env)

    built = cargo_dir / "release" / host["nativeLibrary"]
    if not built.is_file():
        fail(f"{profile}: Cargo did not produce {built}")

    staged = native_dir / host["nativeLibrary"]
    shutil.copy2(built, staged)
    if host["os"] == "macos":
        align_macho_linkedit(staged)
        verify_macho_alignment(staged)

    run(
        CARGO
        + (
            "run",
            "--locked",
            "-p",
            "cribra-uniffi",
            "--no-default-features",
            "--features",
            f"bindgen,{profile}",
            "--bin",
            "cribra-uniffi-bindgen",
            "--",
            "generate",
            "--library",
            str(staged),
            "--language",
            "kotlin",
            "--out-dir",
            str(generated),
        ),
        root,
        env,
    )

    binding = generated / "uniffi/cribra_uniffi/cribra_uniffi.kt"
    if not binding.is_file():
        fail(f"{profile}: bindgen did not produce {binding}")

    # Existing direct-Core oracle shared with Swift/Python, never adapter DTOs.
    oracle = subprocess.run(
        CARGO + ("run", "--locked", "-p", "cribra-uniffi", "--no-default-features",
                 "--features", profile, "--example", "scanner_oracle"),
        cwd=root, env=env, stdout=subprocess.PIPE, check=True,
    )
    (profile_dir / "scanner-oracle.json").write_bytes(oracle.stdout)

    return staged


# ---- Gradle profile JARs ----------------------------------------------------


def build_profile_jars(root: Path, bundles_root: Path) -> None:
    """Package the three profile JARs from the bundle-specific bindings."""
    module = root / "bindings/kotlin"
    gradlew = module / ("gradlew.bat" if sys.platform == "win32" else "gradlew")

    run([gradlew, "-p", module, "--console=plain", "clean"], root)
    run(
        [
            gradlew,
            "-p",
            module,
            "--console=plain",
            "profileJars",
            "verifyProfileJars",
            "stageBundleDependencies",
            "runCoreConformance",
            "runFlowConformance",
            "runFuseConformance",
            f"-Pcribra.kotlin.generated={bundles_root / 'core' / 'generated'}",
            f"-Pcribra.kotlin.core.smoke-dir={bundles_root / 'core'}",
            f"-Pcribra.kotlin.flow.smoke-dir={bundles_root / 'flow'}",
            f"-Pcribra.kotlin.fuse.smoke-dir={bundles_root / 'fuse'}",
            *(f"-Pcribra.kotlin.{profile}.native={bundles_root / profile / 'native'}" for profile in PROFILES),
        ],
        root,
    )


# ---- Verification -----------------------------------------------------------


def verify_jar(jar: Path, profile: str, versions: dict[str, str]) -> None:
    with zipfile.ZipFile(jar) as archive:
        names = archive.namelist()
        manifest = BytesParser().parsebytes(archive.read("META-INF/MANIFEST.MF"))

    entries = set(names)
    if manifest["Cribra-Profile"] != profile:
        fail(f"{jar.name}: manifest profile is {manifest['Cribra-Profile']!r}, expected {profile!r}")
    if manifest["Implementation-Version"] != versions["core"]:
        fail(f"{jar.name}: bad Implementation-Version")
    if manifest["Cribra-Core-Version"] != versions["core"]:
        fail(f"{jar.name}: bad Cribra-Core-Version")
    if manifest["Cribra-Adapter-Version"] != versions["adapter"]:
        fail(f"{jar.name}: bad Cribra-Adapter-Version")
    if manifest["Cribra-Uniffi-Version"] != versions["uniffi"]:
        fail(f"{jar.name}: bad Cribra-Uniffi-Version")
    if manifest["Cribra-Native-Library"] != "external":
        fail(f"{jar.name}: native library is not marked external")
    if manifest["Cribra-Classpath-Rule"] != "exactly-one-profile":
        fail(f"{jar.name}: bad Cribra-Classpath-Rule")

    if not any(name.startswith("uniffi/cribra_uniffi/") and name.endswith(".class") for name in entries):
        fail(f"{jar.name}: missing generated UniFFI classes")

    marker = f"cribra/kotlin/profile/{PROFILE_CLASSES[profile]}.class"
    markers = sorted(
        name
        for name in entries
        if name.startswith("cribra/kotlin/profile/") and name.endswith("Profile.class")
    )
    if markers != [marker]:
        fail(f"{jar.name}: expected exactly one profile marker {marker}, found {markers}")

    forbidden_prefixes = ("com/google/gson/", "cribra/kotlin/smoke/", "cribra/kotlin/flow/", "cribra/kotlin/fuse/")
    for prefix in forbidden_prefixes:
        if any(name.startswith(prefix) for name in entries):
            fail(f"{jar.name}: contains forbidden entry under {prefix}")

    if any(name.endswith((".dylib", ".so", ".dll")) for name in entries):
        fail(f"{jar.name}: embeds a native library")


def verify_jna(path: Path) -> None:
    with zipfile.ZipFile(path) as archive:
        names = set(archive.namelist())
    if "com/sun/jna/Native.class" not in names:
        fail(f"{path.name}: not a JNA runtime JAR")
    if any(name.startswith("com/google/gson/") for name in names):
        fail(f"{path.name}: unexpectedly contains Gson")


def verify_native(library: Path, host: dict[str, str]) -> None:
    data = library.read_bytes()
    magic = data[:4]

    if host["os"] == "macos":
        if magic not in (b"\xcf\xfa\xed\xfe", b"\xca\xfe\xba\xbe"):
            fail(f"{library.name}: not a Mach-O dylib")
    elif host["os"] == "linux":
        if magic != b"\x7fELF":
            fail(f"{library.name}: not an ELF shared object")
    elif host["os"] == "windows":
        if data[:2] != b"MZ":
            fail(f"{library.name}: not a PE DLL")


def verify_checksums(bundle_dir: Path) -> dict[str, str]:
    sums_path = bundle_dir / "metadata/SHA256SUMS"
    listed: dict[str, str] = {}
    for line in sums_path.read_text(encoding="utf-8").splitlines():
        digest, separator, relative = line.partition("  ")
        if not separator or not relative:
            fail(f"malformed SHA256SUMS line: {line!r}")
        listed[relative] = digest

    expected = set(relative_files(bundle_dir)) - {"metadata/SHA256SUMS"}
    if set(listed) != expected:
        fail(f"SHA256SUMS paths {sorted(listed)} do not match bundle files {sorted(expected)}")

    for relative, digest in listed.items():
        if sha256(bundle_dir / relative) != digest:
            fail(f"checksum mismatch for {relative}")
    return listed


def verify_no_leaks(bundle_dir: Path, root: Path) -> None:
    repository_path = str(root.resolve())
    for path in bundle_dir.rglob("*"):
        if not path.is_file():
            continue
        if repository_path.encode() in path.read_bytes():
            fail(f"{path.relative_to(bundle_dir)} embeds the repository path")


def verify_bundle(
    bundle_dir: Path,
    profile: str,
    versions: dict[str, str],
    host: dict[str, str],
    root: Path,
) -> None:
    expected = expected_files(profile, versions, host)
    actual = set(relative_files(bundle_dir))
    if actual != expected:
        fail(f"{profile}: unexpected bundle layout, missing {sorted(expected - actual)}, extra {sorted(actual - expected)}")

    jar = bundle_dir / f"lib/cribra-kotlin-{profile}-{versions['core']}.jar"
    jna = bundle_dir / f"lib/jna-{versions['jna']}.jar"
    verify_jar(jar, profile, versions)
    verify_jna(jna)

    natives = sorted((bundle_dir / "native").iterdir())
    if [path.name for path in natives] != [host["nativeLibrary"]]:
        fail(f"{profile}: expected exactly one native library {host['nativeLibrary']}, found {[p.name for p in natives]}")
    verify_native(natives[0], host)

    build_info = json.loads((bundle_dir / "metadata/build-info.json").read_text(encoding="utf-8"))
    checks = {
        "profile": profile,
        "coreVersion": versions["core"],
        "adapterVersion": versions["adapter"],
        "uniffiVersion": versions["uniffi"],
        "jnaVersion": versions["jna"],
        "kotlinVersion": versions["kotlin"],
        "jdkBaseline": JDK_BASELINE,
        "rustToolchain": RUST_TOOLCHAIN,
        "rustTarget": host["rustTarget"],
        "os": host["os"],
        "arch": host["arch"],
        "platform": host["platform"],
        "bundleFormat": 1,
        "classpathRule": "exactly-one-profile",
    }
    for key, value in checks.items():
        if build_info.get(key) != value:
            fail(f"{profile}: build-info.json {key} is {build_info.get(key)!r}, expected {value!r}")
    if build_info.get("nativeLibrary") != f"native/{host['nativeLibrary']}":
        fail(f"{profile}: build-info.json nativeLibrary is wrong")

    artifacts = build_info.get("artifacts", {})
    expected_artifacts = {
        f"lib/cribra-kotlin-{profile}-{versions['core']}.jar",
        f"lib/jna-{versions['jna']}.jar",
        f"native/{host['nativeLibrary']}",
    }
    if set(artifacts) != expected_artifacts:
        fail(f"{profile}: build-info.json artifacts {sorted(artifacts)} do not match {sorted(expected_artifacts)}")
    for relative, metadata in artifacts.items():
        path = bundle_dir / relative
        if not path.is_file():
            fail(f"{profile}: build-info.json lists missing artifact {relative}")
        if metadata.get("sha256") != sha256(path):
            fail(f"{profile}: build-info.json checksum mismatch for {relative}")
        if metadata.get("bytes") != path.stat().st_size:
            fail(f"{profile}: build-info.json size mismatch for {relative}")

    verify_checksums(bundle_dir)
    verify_no_leaks(bundle_dir, root)


def verify_zip(zip_path: Path, expected: set[str]) -> None:
    with zipfile.ZipFile(zip_path) as archive:
        infos = archive.infolist()

    names = [info.filename for info in infos]
    if names != sorted(names):
        fail(f"{zip_path.name}: entries are not sorted")
    if set(names) != expected:
        fail(f"{zip_path.name}: unexpected entries {sorted(set(names) ^ expected)}")
    for info in infos:
        if info.date_time != ZIP_EPOCH:
            fail(f"{zip_path.name}: {info.filename} has timestamp {info.date_time}")
        if info.compress_type != zipfile.ZIP_DEFLATED:
            fail(f"{zip_path.name}: {info.filename} is not deflated")
        if info.external_attr != 0o644 << 16:
            fail(f"{zip_path.name}: {info.filename} has mode {info.external_attr:#o}")


# ---- Assembly ---------------------------------------------------------------


def bundle_readme(profile: str, versions: dict[str, str], host: dict[str, str]) -> str:
    title = PROFILE_TITLES[profile]
    marker = PROFILE_CLASSES[profile]
    return f"""# Cribra Kotlin/JVM {title} bundle {versions['core']}

Reproducible, host-specific Kotlin/JVM distribution of the Cribra **{title}**
profile. This bundle is a local build artifact; it is not published.

| Component | Version |
| --- | --- |
| Cribra Core | `{versions['core']}` |
| Cribra UniFFI adapter | `{versions['adapter']}` |
| UniFFI | `{versions['uniffi']}` |
| Kotlin | `{versions['kotlin']}` |
| JNA | `{versions['jna']}` |
| JDK baseline | `{JDK_BASELINE}` (classfile 61) |
| Rust | `{RUST_TOOLCHAIN}` (`{host['rustTarget']}`) |
| Host | `{host['os']}` / `{host['arch']}` (`{host['platform']}`) |

## Layout

```
lib/cribra-kotlin-{profile}-{versions['core']}.jar
lib/jna-{versions['jna']}.jar
native/{host['nativeLibrary']}
metadata/build-info.json
metadata/SHA256SUMS
README.md
```

## Use

1. Put both `lib/*.jar` files on the application classpath.
2. Provide the Kotlin standard library (`{versions['kotlin']}`) and JDK {JDK_BASELINE}+.
3. Make `native/{host['nativeLibrary']}` loadable by JNA, for example with
   `-Djna.library.path=<bundle>/native` or by placing it on the platform
   library search path.
4. Use exactly one Cribra profile bundle per classpath. Call
   `cribra.kotlin.profile.{marker}.requireExclusiveClasspath()` once at startup.

The JARs contain no native library: `native/{host['nativeLibrary']}` stays
external by design.

## Verify

From the bundle root:

```sh
shasum -a 256 -c metadata/SHA256SUMS
```

## Limitations

- Built for the current host only (`{host['rustTarget']}`).
- The Kotlin standard library is not bundled.
- Local distribution only: no publishing, signing or external consumer.
- The three Cribra profiles share the `uniffi.cribra_uniffi` package and are
  mutually exclusive on one classpath.
"""


def write_bundle(
    bundle_dir: Path,
    profile: str,
    versions: dict[str, str],
    host: dict[str, str],
    jar: Path,
    jna: Path,
    native: Path,
) -> None:
    if bundle_dir.exists():
        shutil.rmtree(bundle_dir)
    (bundle_dir / "lib").mkdir(parents=True)
    (bundle_dir / "native").mkdir()
    (bundle_dir / "metadata").mkdir()

    shutil.copy2(jar, bundle_dir / "lib" / jar.name)
    shutil.copy2(jna, bundle_dir / "lib" / jna.name)
    shutil.copy2(native, bundle_dir / "native" / native.name)
    (bundle_dir / "README.md").write_text(
        bundle_readme(profile, versions, host), encoding="utf-8"
    )

    payloads = [
        f"lib/{jar.name}",
        f"lib/{jna.name}",
        f"native/{native.name}",
    ]
    artifacts = {
        relative: {
            "sha256": sha256(bundle_dir / relative),
            "bytes": (bundle_dir / relative).stat().st_size,
        }
        for relative in payloads
    }

    build_info = {
        "bundleFormat": 1,
        "profile": profile,
        "coreVersion": versions["core"],
        "adapterVersion": versions["adapter"],
        "uniffiVersion": versions["uniffi"],
        "kotlinVersion": versions["kotlin"],
        "jnaVersion": versions["jna"],
        "jdkBaseline": JDK_BASELINE,
        "rustToolchain": RUST_TOOLCHAIN,
        "rustTarget": host["rustTarget"],
        "os": host["os"],
        "arch": host["arch"],
        "platform": host["platform"],
        "nativeLibrary": f"native/{native.name}",
        "classpathRule": "exactly-one-profile",
        "artifacts": artifacts,
    }
    (bundle_dir / "metadata/build-info.json").write_text(
        json.dumps(build_info, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )

    lines = [
        f"{sha256(bundle_dir / relative)}  {relative}\n"
        for relative in relative_files(bundle_dir)
    ]
    (bundle_dir / "metadata/SHA256SUMS").write_text("".join(lines), encoding="utf-8")


def write_zip(bundle_dir: Path, zip_path: Path) -> None:
    if zip_path.exists():
        zip_path.unlink()
    zip_path.parent.mkdir(parents=True, exist_ok=True)

    with zipfile.ZipFile(
        zip_path,
        "w",
        compression=zipfile.ZIP_DEFLATED,
        compresslevel=ZIP_COMPRESSION_LEVEL,
    ) as archive:
        for relative in relative_files(bundle_dir):
            entry = zipfile.ZipInfo(relative, date_time=ZIP_EPOCH)
            entry.compress_type = zipfile.ZIP_DEFLATED
            entry.create_system = 3  # Unix, so the mode below is meaningful.
            entry.external_attr = 0o644 << 16
            archive.writestr(
                entry,
                (bundle_dir / relative).read_bytes(),
                compress_type=zipfile.ZIP_DEFLATED,
                compresslevel=ZIP_COMPRESSION_LEVEL,
            )


def main() -> None:
    if sys.version_info < (3, 11):
        fail("Python 3.11 or newer is required (tomllib)")

    root = Path(__file__).resolve().parents[2]
    host = host_spec()
    versions = read_versions(root)
    bundles_root = root / "target/uniffi-kotlin-bundles"
    dist = bundles_root / "dist"

    # Never leave a stale archive from an earlier version or host next to the
    # freshly verified ones.
    if dist.exists():
        shutil.rmtree(dist)
    dist.mkdir(parents=True)

    print(
        f"Building {host['platform']} bundles for Cribra {versions['core']} "
        f"(adapter {versions['adapter']}, UniFFI {versions['uniffi']}, "
        f"JNA {versions['jna']})",
        flush=True,
    )

    for profile in PROFILES:
        print(f"\n== {profile}: native library ==", flush=True)
        build_native(root, host, profile, bundles_root / profile)

    print("\n== profile JARs ==", flush=True)
    build_profile_jars(root, bundles_root)

    summaries = []
    for profile in PROFILES:
        print(f"\n== {profile}: bundle ==", flush=True)
        profile_dir = bundles_root / profile
        jar = root / "bindings/kotlin/build/libs" / f"cribra-kotlin-{profile}-{versions['core']}.jar"
        jna = root / "bindings/kotlin/build/bundle-deps" / f"jna-{versions['jna']}.jar"
        native = profile_dir / "native" / host["nativeLibrary"]
        for path in (jar, jna, native):
            if not path.is_file():
                fail(f"missing build input {path}")

        bundle_dir = profile_dir / "bundle"
        write_bundle(bundle_dir, profile, versions, host, jar, jna, native)
        verify_bundle(bundle_dir, profile, versions, host, root)

        zip_path = dist / f"cribra-kotlin-{profile}-{versions['core']}-{host['platform']}.zip"
        write_zip(bundle_dir, zip_path)
        verify_zip(zip_path, expected_files(profile, versions, host))

        summaries.append(
            {
                "profile": profile,
                "bundle": str(bundle_dir.relative_to(root)),
                "zip": str(zip_path.relative_to(root)),
                "zipBytes": zip_path.stat().st_size,
                "zipSha256": sha256(zip_path),
                "native": str(native.relative_to(root)),
                "nativeSha256": sha256(native),
            }
        )
        print(f"verified {zip_path.relative_to(root)}", flush=True)

    print("\n" + json.dumps({"host": host, "bundles": summaries}, indent=2), flush=True)


if __name__ == "__main__":
    main()
