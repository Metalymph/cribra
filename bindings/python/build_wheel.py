"""Build a local macOS ARM64 Fuse wheel using standard PEP 517 tooling."""

import json
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import sys
import tempfile
import tomllib


# Explicit, host-independent deployment target. Override with
# CRIBRA_MACOS_DEPLOYMENT_TARGET as MAJOR.MINOR (for example 13.0). Only the
# compiled binary's minimum OS is verified here; runtime compatibility with the
# configured target is not asserted.
DEFAULT_DEPLOYMENT_TARGET = "13.0"


def deployment_target():
    value = os.environ.get("CRIBRA_MACOS_DEPLOYMENT_TARGET", DEFAULT_DEPLOYMENT_TARGET).strip()
    match = re.fullmatch(r"(\d+)\.(\d+)(?:\.\d+)?", value)
    if not match:
        raise SystemExit(
            "CRIBRA_MACOS_DEPLOYMENT_TARGET must be MAJOR.MINOR, e.g. 13.0; "
            f"got {value!r}.")
    return int(match.group(1)), int(match.group(2))


def build_version(library):
    """Return each LC_BUILD_VERSION load command as a platform/minos mapping."""
    output = subprocess.check_output(["/usr/bin/xcrun", "vtool", "-show-build", str(library)], text=True)
    print(output, end="")
    blocks = []
    current = None
    for line in output.splitlines():
        stripped = line.strip()
        if stripped.startswith("cmd "):
            current = {} if stripped == "cmd LC_BUILD_VERSION" else None
            if current is not None:
                blocks.append(current)
        elif current is not None and stripped.startswith("platform "):
            current["platform"] = stripped.split(None, 1)[1]
        elif current is not None and stripped.startswith("minos "):
            current["minos"] = stripped.split(None, 1)[1]
    return blocks


def main():
    if sys.version_info < (3, 13) or sys.platform != "darwin" or platform.machine() != "arm64":
        raise SystemExit("This local wheel builder supports Python 3.13+ on macOS ARM64 only.")
    target = deployment_target()
    target_text = f"{target[0]}.{target[1]}"
    root = Path(__file__).resolve().parents[2]
    source = root / "bindings/python"
    output = root / "target/uniffi-python-wheel/fuse"
    output.mkdir(parents=True, exist_ok=True)
    metadata = tomllib.loads((source / "pyproject.toml").read_text())
    core_version = tomllib.loads((root / "Cargo.toml").read_text())["package"]["version"]
    if metadata["project"]["version"] != core_version + "+fuse":
        raise SystemExit("Update Python metadata to match the Core version plus +fuse.")
    # Target the configured deployment explicitly instead of inheriting this host's
    # macOS version or the deployment target used to build the Python interpreter.
    env = dict(os.environ, CARGO_TARGET_DIR=str(output / "cargo"),
               MACOSX_DEPLOYMENT_TARGET=target_text, CARGO_PROFILE_RELEASE_STRIP="none",
               PYTHONDONTWRITEBYTECODE="1")

    def run(args):
        subprocess.run(args, cwd=root, env=env, check=True)

    cargo = ["cargo", "+1.97.0"]
    common = ["--locked", "-p", "cribra-uniffi", "--no-default-features"]
    # Cargo does not fingerprint MACOSX_DEPLOYMENT_TARGET, so a previously built
    # cdylib would otherwise be reused despite a changed target. Drop the cached
    # cdylib outputs so the link always runs with the configured deployment
    # target; the verification below still fails closed if the binary disagrees.
    for stale in (output / "cargo/aarch64-apple-darwin/release/libcribra_uniffi.dylib",
                  output / "cargo/aarch64-apple-darwin/release/deps/libcribra_uniffi.dylib"):
        stale.unlink(missing_ok=True)
    run(cargo + ["build"] + common + ["--release", "--features", "fuse",
                                    "--target", "aarch64-apple-darwin", "--lib"])
    library = output / "cargo/aarch64-apple-darwin/release/libcribra_uniffi.dylib"
    blocks = build_version(library)
    if len(blocks) != 1 or blocks[0].get("platform") != "MACOS":
        raise SystemExit(f"Expected one MACOS LC_BUILD_VERSION in {library}, got {blocks}.")
    minos = blocks[0].get("minos", "")
    match = re.fullmatch(r"(\d+)\.(\d+)(?:\.\d+)?", minos)
    if not match or (int(match.group(1)), int(match.group(2))) != target:
        raise SystemExit(
            f"Built {library} with minos {minos!r}; expected {target_text}. "
            "Refusing to tag the wheel from an unverified binary.")
    # Derive the wheel platform tag from the verified binary target, never the host.
    wheel_platform = "macosx_" + minos.replace(".", "_") + "_arm64"
    env["CRIBRA_WHEEL_PLATFORM"] = wheel_platform
    generated = output / "generated"
    run(cargo + ["run"] + common + ["--features", "fuse,bindgen", "--bin", "cribra-uniffi-bindgen",
                                   "--", "generate", "--library", str(library),
                                   "--language", "python", "--out-dir", str(generated)])
    # All setuptools staging, egg-info and build outputs stay under ignored target/.
    # The generated module is used byte-for-byte as the package initializer, so
    # its existing __file__-relative library loader needs no patch or facade.
    with tempfile.TemporaryDirectory(prefix="stage-", dir=output) as temporary:
        stage = Path(temporary)
        for name in ("pyproject.toml", "setup.py", "README.md"):
            shutil.copy2(source / name, stage / name)
        shutil.copy2(root / "LICENSE", stage / "LICENSE")
        package = stage / "cribra_uniffi"
        package.mkdir()
        shutil.copy2(generated / "cribra_uniffi.py", package / "__init__.py")
        shutil.copy2(library, package / library.name)
        run([sys.executable, "-B", "-m", "build", "--wheel", "--no-isolation",
             "--outdir", str(output / "dist"), str(stage)])
    wheel = output / "dist" / (
        f"cribra-{metadata['project']['version']}-py3-none-{wheel_platform}.whl")
    info = {"wheel": str(wheel), "wheelBytes": wheel.stat().st_size,
            "nativeLibrary": str(library), "nativeBytes": library.stat().st_size,
            "coreVersion": core_version, "profile": "fuse", "platform": wheel_platform,
            "deploymentTarget": target_text, "minos": minos}
    (output / "build-info.json").write_text(json.dumps(info, indent=2) + "\n")
    print(json.dumps(info, indent=2))


if __name__ == "__main__":
    main()
