"""Build profile-matched UniFFI bindings and run local Python consumer tests."""

import argparse
import os
from pathlib import Path
import shutil
import subprocess
import sys


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("profile", choices=("core", "flow", "fuse"), nargs="?", default="core")
    profile = parser.parse_args().profile
    if sys.version_info < (3, 13):
        raise SystemExit("Use Python 3.13+ (qualified with Python 3.13.7).")

    root = Path(__file__).resolve().parents[2]
    output = root / "target" / "uniffi-python-consumer" / profile
    generated = output / "generated"
    generated.mkdir(parents=True, exist_ok=True)
    # Keep native artifacts separate from the existing Swift/profile workflows.
    env = dict(os.environ, CARGO_TARGET_DIR=str(output / "cargo"), PYTHONDONTWRITEBYTECODE="1")
    cargo = ["cargo", "+1.97.0"]
    common = ["--locked", "-p", "cribra-uniffi", "--no-default-features"]

    def run(args, **kwargs):
        subprocess.run(args, cwd=root, env=env, check=True, **kwargs)

    run(cargo + ["build"] + common + ["--features", profile, "--lib"])
    if sys.platform == "darwin":
        library = "libcribra_uniffi.dylib"
    elif sys.platform == "win32":
        library = "cribra_uniffi.dll"
    else:
        library = "libcribra_uniffi.so"
    shutil.copy2(output / "cargo" / "debug" / library, generated / library)
    run(cargo + ["run"] + common + [
        "--features", profile + ",bindgen", "--bin", "cribra-uniffi-bindgen", "--",
        "generate", "--library", str(generated / library), "--language", "python",
        "--out-dir", str(generated),
    ])
    oracles = ["scanner"] if profile == "core" else ["scanner", "flow", "share"]
    if profile == "fuse":
        oracles.append("parallel")
    for name in oracles:
        with (output / (name + "-oracle.json")).open("w", encoding="utf-8") as destination:
            run(cargo + ["run"] + common + [
                "--features", profile, "--example", name + "_oracle",
            ], stdout=destination)
    env["PYTHONPATH"] = str(generated)
    fixtures = root / "crates/cribra-uniffi/tests/fixtures"
    run([
        sys.executable, "-B", str(root / "bindings/python/test_consumer.py"),
        str(generated), str(fixtures / "scanner.json"),
        str(output / "scanner-oracle.json"), profile,
    ])
    if profile in ("flow", "fuse"):
        run([
            sys.executable, "-B", str(root / "bindings/python/test_flow_consumer.py"),
            str(generated), str(fixtures), str(output / "flow-oracle.json"),
            str(output / "share-oracle.json"), profile,
        ])
    if profile == "fuse":
        run([
            sys.executable, "-B", str(root / "bindings/python/test_parallel_consumer.py"),
            str(fixtures / "parallel.json"), str(output / "parallel-oracle.json"),
        ])


if __name__ == "__main__":
    main()
