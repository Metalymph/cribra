#!/usr/bin/env python3
"""Qualify the Cribra Kotlin/JVM distribution bundles in isolation.

For each host bundle this driver:

1. Reads and verifies the ZIP's ``metadata/build-info.json``, exact layout and
   ``metadata/SHA256SUMS`` before extracting anything.
2. Extracts the bundle into a fresh temporary directory outside the repository.
3. Re-verifies the extracted layout, checksums, profile JAR, JNA and native
   library.
4. Compiles a minimal consumer from ``verify/consumer`` against only the
   extracted profile JAR and JNA.
5. Runs the consumer in a fresh JVM with JNA restricted to the extracted
   ``native/`` directory, then repeats with the native library removed and with
   an invalid replacement.
6. Runs a mixed-profile classpath case that must be rejected by the profile
   guard.

The runtime classpath contains only files under the temporary qualification
root: the extracted profile JAR, extracted JNA, compiled consumer classes and a
Kotlin standard library staged from the locally installed compiler. No Gradle
output, generated binding, ``target/debug`` artifact or repository source set is
on the runtime classpath.
"""

from __future__ import annotations

import argparse
from email.parser import BytesParser
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import platform
import re
import shutil
import subprocess
import sys
import tempfile
from typing import NoReturn
import zipfile

PROFILES = ("core", "flow", "fuse")
PROFILE_TITLES = {
    "core": "Core",
    "flow": "Flow",
    "fuse": "Fuse",
}
PROFILE_CLASSES = {"core": "CoreProfile", "flow": "FlowProfile", "fuse": "FuseProfile"}
CONSUMER_SOURCES = {
    "core": "CoreConsumer.kt",
    "flow": "FlowConsumer.kt",
    "fuse": "FuseConsumer.kt",
}
CONSUMER_MAINS = {
    "core": "cribra.kotlin.verify.consumer.CoreConsumerKt",
    "flow": "cribra.kotlin.verify.consumer.FlowConsumerKt",
    "fuse": "cribra.kotlin.verify.consumer.FuseConsumerKt",
}
MIXED_PROFILE = {"core": "flow", "flow": "fuse", "fuse": "core"}

REPO_ROOT = Path(__file__).resolve().parents[2]
CONSUMER_DIR = Path(__file__).resolve().parent / "verify" / "consumer"
DEFAULT_DIST = REPO_ROOT / "target" / "uniffi-kotlin-bundles" / "dist"
DEFAULT_REPORT = REPO_ROOT / "target" / "uniffi-kotlin-bundle-consumer" / "qualification.json"


def fail(message: str) -> NoReturn:
    raise SystemExit(f"error: {message}")


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def parse_manifest(data: bytes):
    return BytesParser().parsebytes(data)


def parse_sha256sums(text: str) -> dict[str, str]:
    result: dict[str, str] = {}
    for number, line in enumerate(text.splitlines(), 1):
        if not line.strip():
            continue
        if "  " not in line:
            fail(f"metadata/SHA256SUMS line {number} is not '<sha256>  <path>'")
        digest, relative = line.split("  ", 1)
        if not re.fullmatch(r"[0-9a-f]{64}", digest):
            fail(f"metadata/SHA256SUMS line {number} has an invalid SHA-256")
        path = PurePosixPath(relative)
        if path.is_absolute() or ".." in path.parts or "\\" in relative:
            fail(f"metadata/SHA256SUMS line {number} has an unsafe path: {relative}")
        if relative in result:
            fail(f"metadata/SHA256SUMS repeats {relative}")
        result[relative] = digest
    return result


def host_spec() -> dict[str, str]:
    """Return the current host, matching the bundle builder's host names."""
    machine = platform.machine().lower()

    if sys.platform == "darwin":
        if machine in ("arm64", "aarch64"):
            arch = "aarch64"
            platform_name = "macos-arm64"
        elif machine in ("x86_64", "amd64"):
            arch = "x86_64"
            platform_name = "macos-x86_64"
        else:
            fail(f"unsupported macOS architecture: {platform.machine()}")
        return {
            "os": "macos",
            "arch": arch,
            "platform": platform_name,
            "nativeLibrary": "libcribra_uniffi.dylib",
        }

    if sys.platform.startswith("linux"):
        if machine not in ("x86_64", "amd64"):
            fail(f"unsupported Linux architecture: {platform.machine()}")
        return {
            "os": "linux",
            "arch": "x86_64",
            "platform": "linux-x86_64",
            "nativeLibrary": "libcribra_uniffi.so",
        }

    if sys.platform == "win32":
        if machine not in ("amd64", "x86_64"):
            fail(f"unsupported Windows architecture: {platform.machine()}")
        return {
            "os": "windows",
            "arch": "x86_64",
            "platform": "windows-x86_64",
            "nativeLibrary": "cribra_uniffi.dll",
        }

    fail(f"unsupported host platform: {sys.platform}")


def read_build_info(zip_path: Path) -> dict:
    try:
        with zipfile.ZipFile(zip_path) as archive:
            bad = archive.testzip()
            if bad is not None:
                fail(f"{zip_path.name}: corrupt ZIP entry {bad}")
            try:
                data = archive.read("metadata/build-info.json")
            except KeyError:
                fail(f"{zip_path.name}: missing metadata/build-info.json")
    except zipfile.BadZipFile as exc:
        fail(f"{zip_path.name}: invalid ZIP: {exc}")

    try:
        info = json.loads(data)
    except json.JSONDecodeError as exc:
        fail(f"{zip_path.name}: invalid metadata/build-info.json: {exc}")
    if not isinstance(info, dict):
        fail(f"{zip_path.name}: metadata/build-info.json is not an object")
    return info


def validate_build_info(info: dict, profile: str, host: dict[str, str]) -> set[str]:
    expected = {
        "profile": profile,
        "bundleFormat": 1,
        "classpathRule": "exactly-one-profile",
        "os": host["os"],
        "arch": host["arch"],
        "platform": host["platform"],
        "nativeLibrary": f"native/{host['nativeLibrary']}",
    }
    for key, value in expected.items():
        if info.get(key) != value:
            fail(f"{profile}: build-info {key}={info.get(key)!r}, expected {value!r}")

    for key in (
        "coreVersion",
        "adapterVersion",
        "uniffiVersion",
        "jnaVersion",
        "kotlinVersion",
        "rustToolchain",
        "rustTarget",
    ):
        if not isinstance(info.get(key), str) or not info[key]:
            fail(f"{profile}: build-info is missing {key}")

    if info.get("jdkBaseline") != 17:
        fail(f"{profile}: build-info jdkBaseline must be 17")

    artifacts = info.get("artifacts")
    if not isinstance(artifacts, dict):
        fail(f"{profile}: build-info artifacts must be an object")

    profile_jar = f"lib/cribra-kotlin-{profile}-{info['coreVersion']}.jar"
    jna_jar = f"lib/jna-{info['jnaVersion']}.jar"
    native_library = f"native/{host['nativeLibrary']}"
    if set(artifacts) != {profile_jar, jna_jar, native_library}:
        fail(f"{profile}: unexpected build-info artifacts: {sorted(artifacts)}")

    for relative, metadata in artifacts.items():
        if not isinstance(metadata, dict):
            fail(f"{profile}: artifact {relative} metadata must be an object")
        digest = metadata.get("sha256")
        size = metadata.get("bytes")
        if not isinstance(digest, str) or not re.fullmatch(r"[0-9a-f]{64}", digest):
            fail(f"{profile}: artifact {relative} has an invalid SHA-256")
        if not isinstance(size, int) or size <= 0:
            fail(f"{profile}: artifact {relative} has an invalid size")

    return {
        "README.md",
        "metadata/build-info.json",
        "metadata/SHA256SUMS",
        profile_jar,
        jna_jar,
        native_library,
    }


def verify_zip(zip_path: Path, info: dict, expected: set[str]) -> None:
    try:
        with zipfile.ZipFile(zip_path) as archive:
            names = archive.namelist()
            if len(names) != len(set(names)):
                fail(f"{zip_path.name}: duplicate ZIP entries")
            if set(names) != expected:
                missing = sorted(expected - set(names))
                extra = sorted(set(names) - expected)
                fail(f"{zip_path.name}: layout mismatch; missing={missing}, extra={extra}")

            for entry in archive.infolist():
                if entry.is_dir() or entry.filename.endswith("/"):
                    fail(f"{zip_path.name}: unexpected directory entry {entry.filename}")
                path = PurePosixPath(entry.filename)
                if path.is_absolute() or ".." in path.parts or "\\" in entry.filename:
                    fail(f"{zip_path.name}: unsafe ZIP entry {entry.filename}")

            sums = parse_sha256sums(archive.read("metadata/SHA256SUMS").decode("utf-8"))
            if set(sums) != expected - {"metadata/SHA256SUMS"}:
                fail(f"{zip_path.name}: SHA256SUMS path set does not match the layout")

            for relative, digest in sums.items():
                actual = sha256_bytes(archive.read(relative))
                if actual != digest:
                    fail(f"{zip_path.name}: SHA-256 mismatch for {relative}")

            for relative, metadata in info["artifacts"].items():
                data = archive.read(relative)
                if len(data) != metadata["bytes"]:
                    fail(f"{zip_path.name}: size mismatch for {relative}")
                if sha256_bytes(data) != metadata["sha256"]:
                    fail(f"{zip_path.name}: build-info SHA-256 mismatch for {relative}")

            repo_marker = str(REPO_ROOT).encode("utf-8")
            for relative in ("README.md", "metadata/build-info.json", "metadata/SHA256SUMS"):
                if repo_marker in archive.read(relative):
                    fail(f"{zip_path.name}: repository path leaked into {relative}")
            if repo_marker in archive.read(info["nativeLibrary"]):
                fail(f"{zip_path.name}: repository path leaked into the native library")
    except zipfile.BadZipFile as exc:
        fail(f"{zip_path.name}: invalid ZIP: {exc}")


def extract_zip(zip_path: Path, destination: Path) -> None:
    destination.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(zip_path) as archive:
        for entry in archive.infolist():
            pure = PurePosixPath(entry.filename)
            if entry.is_dir() or entry.filename.endswith("/"):
                fail(f"{zip_path.name}: unexpected directory entry {entry.filename}")
            if pure.is_absolute() or ".." in pure.parts or "\\" in entry.filename:
                fail(f"{zip_path.name}: unsafe ZIP entry {entry.filename}")
            target = destination.joinpath(*pure.parts)
            target.parent.mkdir(parents=True, exist_ok=True)
            with archive.open(entry) as source, target.open("wb") as sink:
                shutil.copyfileobj(source, sink)


def verify_profile_jar(path: Path, profile: str, info: dict) -> None:
    with zipfile.ZipFile(path) as archive:
        names = archive.namelist()
        manifest = parse_manifest(archive.read("META-INF/MANIFEST.MF"))
        checks = {
            "Cribra-Profile": profile,
            "Cribra-Core-Version": info["coreVersion"],
            "Cribra-Adapter-Version": info["adapterVersion"],
            "Cribra-Uniffi-Version": info["uniffiVersion"],
            "Cribra-Native-Library": "external",
            "Cribra-Classpath-Rule": "exactly-one-profile",
            "Implementation-Version": info["coreVersion"],
            "Automatic-Module-Name": f"cribra.kotlin.{profile}",
        }
        for header, expected in checks.items():
            actual = manifest.get(header)
            if actual != expected:
                fail(f"{profile}: profile JAR {header}={actual!r}, expected {expected!r}")

        markers = sorted(
            name
            for name in names
            if name.startswith("cribra/kotlin/profile/") and name.endswith(".class")
        )
        expected_marker = f"cribra/kotlin/profile/{PROFILE_CLASSES[profile]}.class"
        if markers != [expected_marker]:
            fail(f"{profile}: profile markers are {markers}, expected [{expected_marker}]")

        uniffi_classes = [
            name for name in names if name.startswith("uniffi/cribra_uniffi/") and name.endswith(".class")
        ]
        if not uniffi_classes:
            fail(f"{profile}: profile JAR has no uniffi.cribra_uniffi classes")

        first_class = archive.read(uniffi_classes[0])
        if len(first_class) < 8 or int.from_bytes(first_class[6:8], "big") != 61:
            fail(f"{profile}: profile JAR is not compiled for JDK 17 (classfile major 61)")

        forbidden = (
            "com/google/gson/",
            "cribra/kotlin/smoke/",
            "cribra/kotlin/flow/",
            "cribra/kotlin/fuse/",
        )
        for name in names:
            if name.startswith(forbidden):
                fail(f"{profile}: profile JAR contains forbidden entry {name}")
            if name.endswith((".dylib", ".so", ".dll")):
                fail(f"{profile}: profile JAR embeds a native library: {name}")


def verify_jna(path: Path, info: dict) -> None:
    with zipfile.ZipFile(path) as archive:
        names = set(archive.namelist())
        for required in ("com/sun/jna/Native.class", "com/sun/jna/NativeLibrary.class"):
            if required not in names:
                fail(f"{path.name}: missing {required}")
        manifest = parse_manifest(archive.read("META-INF/MANIFEST.MF"))
        version = manifest.get("Implementation-Version", "")
        if not version.startswith(info["jnaVersion"]):
            fail(f"{path.name}: JNA version {version!r} does not match {info['jnaVersion']}")


def verify_native(path: Path, host: dict[str, str]) -> None:
    data = path.read_bytes()
    if not data:
        fail(f"{path.name}: native library is empty")
    if host["os"] == "macos":
        valid = data[:4] in (b"\xcf\xfa\xed\xfe", b"\xce\xfa\xed\xfe", b"\xca\xfe\xba\xbe")
    elif host["os"] == "linux":
        valid = data[:4] == b"\x7fELF"
    else:
        valid = data[:2] == b"MZ"
    if not valid:
        fail(f"{path.name}: unexpected native library format for {host['platform']}")


def verify_extracted(
    extract_dir: Path,
    zip_info: dict,
    expected: set[str],
    profile: str,
    host: dict[str, str],
) -> tuple[Path, Path, Path]:
    files = sorted(
        path.relative_to(extract_dir).as_posix()
        for path in extract_dir.rglob("*")
        if path.is_file()
    )
    if set(files) != expected or len(files) != len(expected):
        missing = sorted(expected - set(files))
        extra = sorted(set(files) - expected)
        fail(f"{profile}: extracted layout mismatch; missing={missing}, extra={extra}")
    for path in extract_dir.rglob("*"):
        if path.is_symlink():
            fail(f"{profile}: extracted bundle contains a symlink: {path}")

    on_disk_info = json.loads((extract_dir / "metadata/build-info.json").read_text(encoding="utf-8"))
    if on_disk_info != zip_info:
        fail(f"{profile}: extracted build-info.json differs from the ZIP")

    sums = parse_sha256sums((extract_dir / "metadata/SHA256SUMS").read_text(encoding="utf-8"))
    if set(sums) != expected - {"metadata/SHA256SUMS"}:
        fail(f"{profile}: extracted SHA256SUMS path set does not match the layout")
    for relative, digest in sums.items():
        path = extract_dir / relative
        if sha256_file(path) != digest:
            fail(f"{profile}: extracted SHA-256 mismatch for {relative}")
        metadata = zip_info["artifacts"].get(relative)
        if metadata is not None and path.stat().st_size != metadata["bytes"]:
            fail(f"{profile}: extracted size mismatch for {relative}")

    profile_jar = extract_dir / f"lib/cribra-kotlin-{profile}-{zip_info['coreVersion']}.jar"
    jna_jar = extract_dir / f"lib/jna-{zip_info['jnaVersion']}.jar"
    native = extract_dir / zip_info["nativeLibrary"]
    verify_profile_jar(profile_jar, profile, zip_info)
    verify_jna(jna_jar, zip_info)
    verify_native(native, host)
    return profile_jar, jna_jar, native


def find_executable(name: str, env_name: str) -> Path:
    raw = os.environ.get(env_name) or shutil.which(name)
    if not raw:
        fail(f"{name} not found; set {env_name} or add {name} to PATH")
    path = Path(raw).resolve()
    if not path.is_file():
        fail(f"{env_name} points to a non-file: {path}")
    return path


def run_capture(args: list[Path | str], cwd: Path, env: dict[str, str]) -> subprocess.CompletedProcess:
    command = [str(arg) for arg in args]
    # kotlinc.bat needs Windows shell handling; Java and POSIX launchers do not.
    batch = sys.platform == "win32" and Path(command[0]).suffix.lower() in (".bat", ".cmd")
    return subprocess.run(
        command,
        cwd=cwd,
        env=env,
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        timeout=600,
        shell=batch,
    )


def process_evidence(result: subprocess.CompletedProcess) -> dict:
    return {
        "exitCode": result.returncode,
        "command": result.args,
        "stdout": result.stdout,
        "stderr": result.stderr,
    }


def print_result(result: subprocess.CompletedProcess) -> None:
    if result.stdout:
        print(result.stdout, end="")
    if result.stderr:
        print(result.stderr, end="", file=sys.stderr)


def run_checked(args: list[Path | str], cwd: Path, env: dict[str, str], label: str) -> subprocess.CompletedProcess:
    result = run_capture(args, cwd, env)
    print_result(result)
    if result.returncode != 0:
        fail(f"{label} failed with exit code {result.returncode}")
    return result


def clean_env() -> dict[str, str]:
    env = dict(os.environ)
    for name in (
        "CLASSPATH",
        "JAVA_TOOL_OPTIONS",
        "_JAVA_OPTIONS",
        "JDK_JAVA_OPTIONS",
        "KOTLIN_OPTS",
        "JAVA_OPTS",
    ):
        env.pop(name, None)
    return env


def kotlinc_version(kotlinc: Path, env: dict[str, str]) -> str:
    result = run_capture([kotlinc, "-version"], REPO_ROOT, env)
    output = result.stdout + result.stderr
    match = re.search(r"kotlinc-jvm\s+([0-9]+\.[0-9]+\.[0-9]+)", output)
    if result.returncode != 0 or match is None:
        fail(f"could not determine kotlinc version:\n{output}")
    return match.group(1)


def java_major(java: Path, env: dict[str, str]) -> int:
    result = run_capture([java, "-version"], REPO_ROOT, env)
    output = result.stdout + result.stderr
    match = re.search(r'version "(\d+)(?:\.(\d+))?', output)
    if result.returncode != 0 or match is None:
        fail(f"could not determine Java version:\n{output}")
    major = int(match.group(1))
    if major == 1 and match.group(2):
        major = int(match.group(2))
    return major


def find_kotlin_stdlib(kotlinc: Path, expected_version: str) -> Path:
    candidates: list[Path] = []
    kotlin_home = os.environ.get("KOTLIN_HOME")
    if kotlin_home:
        candidates.extend(
            [
                Path(kotlin_home) / "lib" / "kotlin-stdlib.jar",
                Path(kotlin_home) / "libexec" / "lib" / "kotlin-stdlib.jar",
            ]
        )
    resolved = kotlinc.resolve()
    candidates.extend(
        [
            resolved.parent.parent / "libexec" / "lib" / "kotlin-stdlib.jar",
            resolved.parent.parent / "lib" / "kotlin-stdlib.jar",
            resolved.parent / "lib" / "kotlin-stdlib.jar",
        ]
    )

    for candidate in candidates:
        if not candidate.is_file():
            continue
        with zipfile.ZipFile(candidate) as archive:
            manifest = parse_manifest(archive.read("META-INF/MANIFEST.MF"))
        if manifest.get("Implementation-Version") == expected_version:
            return candidate.resolve()

    fail(
        f"kotlin-stdlib {expected_version} not found next to {kotlinc}; "
        "install the matching Kotlin compiler"
    )


def discover_bundles(dist: Path, host: dict[str, str]) -> dict[str, tuple[Path, dict]]:
    bundles: dict[str, tuple[Path, dict]] = {}
    for profile in PROFILES:
        matches: list[tuple[Path, dict]] = []
        for candidate in sorted(dist.glob(f"cribra-kotlin-{profile}-*.zip")):
            info = read_build_info(candidate)
            if (
                info.get("profile") == profile
                and info.get("os") == host["os"]
                and info.get("arch") == host["arch"]
            ):
                matches.append((candidate, info))
        if not matches:
            fail(
                f"no {host['platform']} bundle for profile {profile} under {dist}; "
                "run `just uniffi-kotlin-bundles` first"
            )
        if len(matches) > 1:
            fail(f"multiple {host['platform']} bundles for profile {profile}: {[str(p) for p, _ in matches]}")
        bundles[profile] = matches[0]
    return bundles


def isolated_classpath(
    entries: list[Path],
    temp_root: Path,
) -> list[Path]:
    root = temp_root.resolve()
    for entry in entries:
        resolved = entry.resolve()
        if resolved != root and root not in resolved.parents:
            fail(f"runtime classpath entry escapes the isolated root: {resolved}")
    return entries


def compile_consumer(
    kotlinc: Path,
    profile: str,
    profile_jar: Path,
    jna_jar: Path,
    classes_dir: Path,
    temp_root: Path,
    env: dict[str, str],
) -> None:
    sources = [CONSUMER_DIR / "BundleNative.kt", CONSUMER_DIR / CONSUMER_SOURCES[profile]]
    for source in sources:
        if not source.is_file():
            fail(f"missing consumer source: {source}")
    classes_dir.mkdir(parents=True, exist_ok=True)
    classpath = os.pathsep.join([str(profile_jar), str(jna_jar)])
    compiler_args = ["-jvm-target", "17", "-classpath", classpath, "-d", str(classes_dir),
                     *(str(source) for source in sources)]
    # Python's Windows argv quoting does not quote a whitespace-free ';' value.
    # kotlinc.bat then splits that classpath and treats the next JAR as a source.
    # Let Kotlin parse quoted arguments itself, beyond the batch launcher.
    argument_file = classes_dir.with_suffix(".args")
    argument_file.write_text(
        "\n".join(json.dumps(arg, ensure_ascii=False) for arg in compiler_args) + "\n",
        encoding="utf-8",
    )
    run_checked(
        [kotlinc, f"@{argument_file}"],
        cwd=temp_root,
        env=env,
        label=f"compile {profile} consumer",
    )


def consumer_command(
    java: Path,
    classpath: list[Path],
    main_class: str,
    native_dir: Path,
    expected_native: Path,
    case_dir: Path,
) -> tuple[list[Path | str], Path, dict[str, str]]:
    jna_tmp = case_dir / "jna"
    java_tmp = case_dir / "java-tmp"
    run_dir = case_dir / "cwd"
    for directory in (jna_tmp, java_tmp, run_dir):
        directory.mkdir(parents=True, exist_ok=True)

    args: list[Path | str] = [
        java,
        f"-Djna.library.path={native_dir}",
        "-Djna.nosys=true",
        f"-Djna.tmpdir={jna_tmp}",
        f"-Djava.library.path={native_dir}",
        f"-Djava.io.tmpdir={java_tmp}",
        "-Dfile.encoding=UTF-8",
        "-cp",
        os.pathsep.join(str(entry) for entry in classpath),
        main_class,
        expected_native,
    ]
    return args, run_dir, clean_env()


def expect_success(result: subprocess.CompletedProcess, profile: str, expected_native: Path) -> str:
    output = result.stdout + result.stderr
    if result.returncode != 0:
        fail(f"{profile}: isolated consumer failed with exit code {result.returncode}:\n{output}")
    if f"PASS: isolated {PROFILE_TITLES[profile]} consumer" not in result.stdout:
        fail(f"{profile}: isolated consumer did not print its PASS marker:\n{output}")

    native_lines = [line for line in result.stdout.splitlines() if line.startswith("NATIVE=")]
    if len(native_lines) != 1:
        fail(f"{profile}: expected exactly one NATIVE= line, got {native_lines}")
    loaded = Path(native_lines[0].split("=", 1)[1]).resolve()
    if loaded != expected_native.resolve():
        fail(f"{profile}: consumer loaded {loaded}, expected {expected_native}")
    return str(loaded)


def expect_failure(
    result: subprocess.CompletedProcess,
    label: str,
    required: tuple[str, ...],
) -> dict:
    output = result.stdout + result.stderr
    if result.returncode == 0:
        fail(f"{label} unexpectedly succeeded:\n{output}")
    if result.returncode < 0:
        fail(f"{label} terminated by signal {-result.returncode}:\n{output}")
    for marker in required:
        if marker not in output:
            fail(f"{label} failed without expected marker {marker!r}:\n{output}")
    print(f"PASS: expected rejection: {label} (exit {result.returncode})")
    return {**process_evidence(result), "rejected": True, "requiredMarkers": list(required)}


def qualify_profile(
    *,
    profile: str,
    zip_path: Path,
    zip_info: dict,
    expected_layout: set[str],
    extract_dir: Path,
    temp_root: Path,
    host: dict[str, str],
    kotlinc: Path,
    java: Path,
    stdlib: Path,
    env: dict[str, str],
    other_jar: Path,
) -> dict:
    profile_jar, jna_jar, native = verify_extracted(
        extract_dir, zip_info, expected_layout, profile, host
    )

    classes_dir = temp_root / "classes" / profile
    compile_consumer(kotlinc, profile, profile_jar, jna_jar, classes_dir, temp_root, env)

    classpath = isolated_classpath([classes_dir, profile_jar, jna_jar, stdlib], temp_root)
    main_class = CONSUMER_MAINS[profile]

    positive_case = temp_root / "cases" / profile / "positive"
    positive_args, positive_cwd, positive_env = consumer_command(
        java, classpath, main_class, native.parent, native, positive_case
    )
    positive = run_capture(positive_args, positive_cwd, positive_env)
    loaded = expect_success(positive, profile, native)
    print_result(positive)

    negative: dict[str, dict] = {}
    native_name = host["nativeLibrary"]

    removed_dir = temp_root / "negative" / profile / "removed"
    removed_dir.mkdir(parents=True, exist_ok=True)
    if any(removed_dir.iterdir()):
        fail(f"{profile}: removed-native directory is not empty: {removed_dir}")
    removed_native = removed_dir / native_name
    removed_case = temp_root / "cases" / profile / "removed"
    removed_args, removed_cwd, removed_env = consumer_command(
        java, classpath, main_class, removed_dir, removed_native, removed_case
    )
    removed = run_capture(removed_args, removed_cwd, removed_env)
    negative["removed"] = expect_failure(
        removed, f"{profile} with the native library removed", ("UnsatisfiedLinkError", "cribra_uniffi")
    )

    invalid_dir = temp_root / "negative" / profile / "invalid"
    invalid_dir.mkdir(parents=True, exist_ok=True)
    invalid_native = invalid_dir / native_name
    invalid_native.write_bytes(b"not a native library\n")
    invalid_case = temp_root / "cases" / profile / "invalid"
    invalid_args, invalid_cwd, invalid_env = consumer_command(
        java, classpath, main_class, invalid_dir, invalid_native, invalid_case
    )
    invalid = run_capture(invalid_args, invalid_cwd, invalid_env)
    negative["invalid"] = expect_failure(
        invalid, f"{profile} with an invalid native library", ("UnsatisfiedLinkError", "cribra_uniffi")
    )

    other_profile = MIXED_PROFILE[profile]
    mixed_classpath = isolated_classpath(
        [classes_dir, profile_jar, jna_jar, stdlib, other_jar], temp_root
    )
    mixed_case = temp_root / "cases" / profile / "mixed"
    mixed_args, mixed_cwd, mixed_env = consumer_command(
        java, mixed_classpath, main_class, native.parent, native, mixed_case
    )
    mixed = run_capture(mixed_args, mixed_cwd, mixed_env)
    mixed_evidence = expect_failure(
        mixed,
        f"{profile} with the {other_profile} profile JAR on the classpath",
        ("cannot share a classpath", "exactly one Cribra"),
    )

    sources = [CONSUMER_DIR / "BundleNative.kt", CONSUMER_DIR / CONSUMER_SOURCES[profile]]
    return {
        "profile": profile,
        "bundle": str(zip_path),
        "bundleBytes": zip_path.stat().st_size,
        "bundleSha256": sha256_file(zip_path),
        "extractDir": str(extract_dir),
        "native": {
            "path": str(native),
            "sha256": sha256_file(native),
            "loaded": loaded,
        },
        "consumer": {
            "sources": [str(source) for source in sources],
            "sourceSha256": {source.name: sha256_file(source) for source in sources},
            "classes": str(classes_dir),
            "runtimeClasspath": [str(entry) for entry in classpath],
        },
        "positive": {**process_evidence(positive), "pass": True},
        "negative": negative,
        "mixedClasspath": {
            "otherProfile": other_profile,
            "otherJar": str(other_jar),
            **mixed_evidence,
        },
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--dist",
        type=Path,
        default=DEFAULT_DIST,
        help=f"directory containing the profile ZIP bundles (default: {DEFAULT_DIST})",
    )
    parser.add_argument(
        "--report",
        type=Path,
        default=DEFAULT_REPORT,
        help=f"JSON evidence file, outside --dist (default: {DEFAULT_REPORT})",
    )
    parser.add_argument(
        "--keep",
        action="store_true",
        help="keep the temporary qualification root even on success",
    )
    args = parser.parse_args()

    dist = (REPO_ROOT / args.dist).resolve()
    report = (REPO_ROOT / args.report).resolve()
    if not dist.is_dir():
        fail(f"bundle directory does not exist: {dist}")
    if report == dist or dist in report.parents:
        fail("the qualification report must be outside the distribution bundle directory")

    host = host_spec()
    temp_root = Path(tempfile.mkdtemp(prefix="cribra-kotlin-bundle-consumer-")).resolve()
    repo_root = REPO_ROOT.resolve()
    if temp_root == repo_root or repo_root in temp_root.parents:
        shutil.rmtree(temp_root)
        fail(f"temporary qualification root is inside the repository: {temp_root}")

    env = clean_env()
    success = False
    try:
        kotlinc = find_executable("kotlinc", "KOTLINC")
        java = find_executable("java", "JAVA")
        compiler_version = kotlinc_version(kotlinc, env)
        major = java_major(java, env)
        if major < 17:
            fail(f"JDK 17+ required, found Java {major}")

        bundles = discover_bundles(dist, host)
        kotlin_versions = {info["kotlinVersion"] for _, info in bundles.values()}
        if len(kotlin_versions) != 1:
            fail(f"bundles disagree on the Kotlin version: {sorted(kotlin_versions)}")
        bundle_kotlin = kotlin_versions.pop()
        if compiler_version != bundle_kotlin:
            fail(
                f"kotlinc is {compiler_version}, but the bundles declare Kotlin {bundle_kotlin}; "
                "use the matching compiler"
            )

        stdlib_source = find_kotlin_stdlib(kotlinc, bundle_kotlin)
        external_dir = temp_root / "external"
        external_dir.mkdir(parents=True, exist_ok=True)
        stdlib = external_dir / f"kotlin-stdlib-{bundle_kotlin}.jar"
        shutil.copy2(stdlib_source, stdlib)

        print(f"temporary root: {temp_root}")
        print(f"host: {host['platform']}")
        print(f"kotlinc: {kotlinc} ({compiler_version})")
        print(f"java: {java} ({major})")
        print(f"kotlin stdlib: {stdlib_source} -> {stdlib} ({sha256_file(stdlib)})")

        extracted: dict[str, dict] = {}
        for profile in PROFILES:
            zip_path, zip_info = bundles[profile]
            expected_layout = validate_build_info(zip_info, profile, host)
            verify_zip(zip_path, zip_info, expected_layout)
            extract_dir = temp_root / profile / "bundle"
            extract_zip(zip_path, extract_dir)
            verify_extracted(extract_dir, zip_info, expected_layout, profile, host)
            extracted[profile] = {
                "zip": zip_path,
                "info": zip_info,
                "expected": expected_layout,
                "dir": extract_dir,
            }
            print(f"verified {profile}: {zip_path} -> {extract_dir}")

        results = []
        for profile in PROFILES:
            bundle = extracted[profile]
            other_profile = MIXED_PROFILE[profile]
            other_jar = extracted[other_profile]["dir"] / (
                f"lib/cribra-kotlin-{other_profile}-{extracted[other_profile]['info']['coreVersion']}.jar"
            )
            result = qualify_profile(
                profile=profile,
                zip_path=bundle["zip"],
                zip_info=bundle["info"],
                expected_layout=bundle["expected"],
                extract_dir=bundle["dir"],
                temp_root=temp_root,
                host=host,
                kotlinc=kotlinc,
                java=java,
                stdlib=stdlib,
                env=env,
                other_jar=other_jar,
            )
            results.append(result)

        summary = {
            "success": True,
            "host": host,
            "tempRoot": str(temp_root),
            "kotlinCompiler": {"path": str(kotlinc), "version": compiler_version},
            "java": {"path": str(java), "major": major},
            "kotlinStdlib": {
                "source": str(stdlib_source),
                "staged": str(stdlib),
                "version": bundle_kotlin,
                "sha256": sha256_file(stdlib),
            },
            "profiles": results,
        }
        report.parent.mkdir(parents=True, exist_ok=True)
        with tempfile.NamedTemporaryFile(mode="w", encoding="utf-8", dir=report.parent,
                                         prefix=report.name + ".", delete=False) as handle:
            json.dump(summary, handle, indent=2, sort_keys=True)
            handle.write("\n")
        os.replace(handle.name, report)
        print("\nPASS: 3 isolated JVM consumers and 9 expected rejections")
        print(f"qualification evidence: {report}")
        success = True
        return 0
    finally:
        if success and not args.keep:
            shutil.rmtree(temp_root)
            print(f"\ncleaned up {temp_root}")
        else:
            print(f"\ntemporary qualification root retained at {temp_root}")


if __name__ == "__main__":
    raise SystemExit(main())
