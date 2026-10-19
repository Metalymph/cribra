"""Build a local macOS ARM64 or Linux x86_64 Fuse wheel using PEP 517 tooling."""

from email.parser import BytesParser
import json
import os
from pathlib import Path
import platform
import re
import shutil
import struct
import subprocess
import sys
import tempfile
import tomllib
import zipfile


# Explicit, host-independent macOS deployment target. Override with
# CRIBRA_MACOS_DEPLOYMENT_TARGET as MAJOR.MINOR (for example 13.0). Only the
# compiled binary's minimum OS is verified here; runtime compatibility with the
# configured target is not asserted.
DEFAULT_DEPLOYMENT_TARGET = "13.0"
# Local Linux builds are tagged for the build host only. A manylinux/musllinux tag
# is intentionally not assigned until that compatibility contract is verified.
LINUX_WHEEL_PLATFORM = "linux_x86_64"
# Opt-in manylinux qualification (Linux only). When CRIBRA_MANYLINUX_PLATFORM names
# an exact manylinux x86_64 platform tag, the builder keeps the provisional wheel
# under its native `linux_x86_64` tag in its own directory and only surfaces a
# relabelled wheel once `auditwheel repair --plat` accepts the requested policy.
# The macOS and native Linux paths are unchanged when the variable is unset.
MANYLINUX_PLATFORM = "CRIBRA_MANYLINUX_PLATFORM"


def deployment_target():
    value = os.environ.get("CRIBRA_MACOS_DEPLOYMENT_TARGET", DEFAULT_DEPLOYMENT_TARGET).strip()
    match = re.fullmatch(r"(\d+)\.(\d+)(?:\.\d+)?", value)
    if not match:
        raise SystemExit(
            "CRIBRA_MACOS_DEPLOYMENT_TARGET must be MAJOR.MINOR, e.g. 13.0; "
            f"got {value!r}.")
    return int(match.group(1)), int(match.group(2))


def host_spec():
    """Describe the supported native host as (os, target triple, library name)."""
    if sys.platform == "darwin":
        if platform.machine() != "arm64":
            raise SystemExit("This local wheel builder supports macOS ARM64 only.")
        return "macos", "aarch64-apple-darwin", "libcribra_uniffi.dylib"
    if sys.platform.startswith("linux"):
        if platform.machine() not in ("x86_64", "amd64"):
            raise SystemExit("This local wheel builder supports Linux x86_64 only.")
        return "linux", "x86_64-unknown-linux-gnu", "libcribra_uniffi.so"
    raise SystemExit("This local wheel builder supports macOS ARM64 and Linux x86_64 only.")


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


def verify_macos(library, target):
    """Fail closed unless the compiled dylib declares the configured macOS target."""
    blocks = build_version(library)
    if len(blocks) != 1 or blocks[0].get("platform") != "MACOS":
        raise SystemExit(f"Expected one MACOS LC_BUILD_VERSION in {library}, got {blocks}.")
    minos = blocks[0].get("minos", "")
    match = re.fullmatch(r"(\d+)\.(\d+)(?:\.\d+)?", minos)
    if not match or (int(match.group(1)), int(match.group(2))) != target:
        raise SystemExit(
            f"Built {library} with minos {minos!r}; expected "
            f"{target[0]}.{target[1]}. Refusing to tag the wheel from an unverified binary.")
    return minos


def verify_linux(library):
    """Fail closed unless the compiled library is a 64-bit little-endian x86_64 ELF object."""
    header = library.read_bytes()[:20]
    if len(header) < 20 or header[:4] != b"\x7fELF" or header[4] != 2 or header[5] != 1:
        raise SystemExit(f"{library} is not a 64-bit little-endian ELF shared library.")
    e_type, e_machine = struct.unpack_from("<HH", header, 16)
    if e_type != 3 or e_machine != 0x3E:  # ET_DYN (shared object), EM_X86_64
        raise SystemExit(
            f"{library} is not an x86_64 ELF shared object "
            f"(type={e_type}, machine=0x{e_machine:x}).")
    print(f"Verified {library}: ELF64 little-endian ET_DYN, machine EM_X86_64.")


def verify_wheel_native(wheel, library_name):
    """Fail closed unless the wheel packages exactly the built native library."""
    with zipfile.ZipFile(wheel) as archive:
        packaged = sorted(name for name in archive.namelist()
                          if name.startswith("cribra_uniffi/libcribra_uniffi."))
    expected = f"cribra_uniffi/{library_name}"
    if packaged != [expected]:
        raise SystemExit(
            f"Wheel {wheel.name} packages {packaged}; expected exactly [{expected}]. "
            "Check [tool.setuptools.package-data] in bindings/python/pyproject.toml.")
    print(f"Verified {wheel.name}: packages exactly {expected}.")


def manylinux_platform():
    """Return the requested manylinux x86_64 platform tag, or None when unset."""
    value = os.environ.get(MANYLINUX_PLATFORM, "").strip()
    if not value:
        return None
    if not re.fullmatch(r"manylinux_\d+_\d+_x86_64", value):
        raise SystemExit(
            f"{MANYLINUX_PLATFORM} must name a manylinux x86_64 platform tag, "
            f"for example manylinux_2_28_x86_64; got {value!r}.")
    return value


def verify_manylinux_wheel(wheel, library_name, platform):
    """Fail closed unless auditwheel produced an ABI-independent platform wheel."""
    verify_wheel_native(wheel, library_name)
    with zipfile.ZipFile(wheel) as archive:
        metadata = next(name for name in archive.namelist()
                        if name.endswith(".dist-info/WHEEL"))
        tags = BytesParser().parsebytes(archive.read(metadata))
    expected = f"py3-none-{platform}"
    if tags["Tag"] != expected or tags["Root-Is-Purelib"] != "false":
        raise SystemExit(
            f"Wheel {wheel.name} declares Tag {tags['Tag']!r} and "
            f"Root-Is-Purelib {tags['Root-Is-Purelib']!r}; expected {expected!r} "
            "and false. The ABI-independent tag must be preserved by auditwheel.")
    print(f"Verified {wheel.name}: ABI-independent tag {expected}.")


def qualify_manylinux(wheel, library_name, platform, output, dist):
    """Relabel a wheel only via auditwheel, and only to a confirmed policy."""
    show = subprocess.run([sys.executable, "-m", "auditwheel", "show", str(wheel)],
                          text=True, capture_output=True)
    report = show.stdout + show.stderr
    (output / "auditwheel-show.txt").write_text(report)
    print(report, end="")
    if show.returncode != 0:
        raise SystemExit(f"auditwheel show failed for {wheel.name}.")
    # A repeated build must not inherit a stale qualified wheel (or the
    # "exactly one" check below would refuse a previously good output).
    if dist.exists():
        shutil.rmtree(dist)
    dist.mkdir(parents=True)
    repair = subprocess.run(
        [sys.executable, "-m", "auditwheel", "repair", "--plat", platform,
         "--wheel-dir", str(dist), str(wheel)],
        text=True, capture_output=True)
    print(repair.stdout + repair.stderr, end="")
    if repair.returncode != 0:
        raise SystemExit(
            f"auditwheel repair --plat {platform} refused {wheel.name}; "
            "refusing to claim manylinux compatibility.")
    qualified = sorted(dist.glob(f"*-{platform}.whl"))
    if len(qualified) != 1:
        raise SystemExit(
            f"Expected exactly one {platform} wheel in {dist}, "
            f"found {[path.name for path in qualified]}.")
    verify_manylinux_wheel(qualified[0], library_name, platform)
    return qualified[0]


def main():
    if sys.version_info < (3, 13):
        raise SystemExit("This local wheel builder supports Python 3.13+ only.")
    operating_system, triple, library_name = host_spec()
    manylinux = manylinux_platform() if operating_system == "linux" else None
    if operating_system == "macos":
        target = deployment_target()
        target_text = f"{target[0]}.{target[1]}"
        platform_key = "macosx_" + target_text.replace(".", "_") + "_arm64"
    else:
        target = None
        target_text = None
        # A requested manylinux build keeps its own directory, so it never
        # overwrites the native `linux_x86_64` artifacts.
        platform_key = manylinux or LINUX_WHEEL_PLATFORM
    root = Path(__file__).resolve().parents[2]
    source = root / "bindings/python"
    # Each host/target writes to its own platform directory, so macOS and Linux
    # Cargo, generated files, dist and metadata never overwrite one another.
    output = root / "target/uniffi-python-wheel/fuse" / platform_key
    output.mkdir(parents=True, exist_ok=True)
    metadata = tomllib.loads((source / "pyproject.toml").read_text())
    core_version = tomllib.loads((root / "Cargo.toml").read_text())["package"]["version"]
    if metadata["project"]["version"] != core_version + "+fuse":
        raise SystemExit("Update Python metadata to match the Core version plus +fuse.")
    # Target the configured deployment explicitly instead of inheriting this host's
    # macOS version or the deployment target used to build the Python interpreter.
    env = dict(os.environ, CARGO_TARGET_DIR=str(output / "cargo"),
               CARGO_PROFILE_RELEASE_STRIP="none", PYTHONDONTWRITEBYTECODE="1")
    if target_text is not None:
        env["MACOSX_DEPLOYMENT_TARGET"] = target_text

    def run(args):
        subprocess.run(args, cwd=root, env=env, check=True)

    cargo = ["cargo", "+1.97.0"]
    common = ["--locked", "-p", "cribra-uniffi", "--no-default-features"]
    release = output / "cargo" / triple / "release"
    # Cargo does not fingerprint the deployment/toolchain environment, so a
    # previously built cdylib would otherwise be reused despite a changed target.
    # Drop the cached cdylib outputs so the link always runs with the configured
    # settings; the verification below still fails closed if the binary disagrees.
    for stale in (release / library_name, release / "deps" / library_name):
        stale.unlink(missing_ok=True)
    run(cargo + ["build"] + common + ["--release", "--features", "fuse",
                                    "--target", triple, "--lib"])
    library = release / library_name
    if operating_system == "macos":
        minos = verify_macos(library, target)
        # Derive the wheel platform tag from the verified binary target, never the host.
        wheel_platform = "macosx_" + minos.replace(".", "_") + "_arm64"
    else:
        verify_linux(library)
        minos = None
        wheel_platform = LINUX_WHEEL_PLATFORM
    env["CRIBRA_WHEEL_PLATFORM"] = wheel_platform
    generated = output / "generated"
    run(cargo + ["run"] + common + ["--features", "fuse,bindgen", "--bin", "cribra-uniffi-bindgen",
                                   "--", "generate", "--library", str(library),
                                   "--language", "python", "--out-dir", str(generated)])
    # All setuptools staging, egg-info and build outputs stay under ignored target/.
    # The generated module is used byte-for-byte as the package initializer, so
    # its existing __file__-relative library loader needs no patch or facade; on
    # Linux that loader already resolves `libcribra_uniffi.so`.
    dist = output / "dist"
    # A manylinux build keeps the provisional wheel under its native tag in a
    # separate directory; auditwheel owns the qualified `dist/` output.
    provisional_dir = output / "provisional" if manylinux is not None else dist
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
             "--outdir", str(provisional_dir), str(stage)])
    provisional = provisional_dir / (
        f"cribra-{metadata['project']['version']}-py3-none-{wheel_platform}.whl")
    if manylinux is not None:
        wheel = qualify_manylinux(provisional, library_name, manylinux, output, dist)
        wheel_platform = manylinux
    else:
        wheel = provisional
        verify_wheel_native(wheel, library_name)
    info = {"wheel": str(wheel), "wheelBytes": wheel.stat().st_size,
            "nativeLibrary": str(library), "nativeBytes": library.stat().st_size,
            "generatedModule": str(generated / "cribra_uniffi.py"),
            "coreVersion": core_version, "profile": "fuse", "platform": wheel_platform,
            "host": operating_system, "targetTriple": triple}
    if minos is not None:
        info["deploymentTarget"] = target_text
        info["minos"] = minos
    (output / "build-info.json").write_text(json.dumps(info, indent=2) + "\n")
    print(json.dumps(info, indent=2))


if __name__ == "__main__":
    main()
