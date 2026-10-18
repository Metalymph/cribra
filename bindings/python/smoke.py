"""Build the Core-only UniFFI library/bindings, then run the local consumer tests."""

import os
from pathlib import Path
import shutil
import subprocess
import sys


def main():
    if sys.version_info < (3, 13):
        raise SystemExit("Use Python 3.13+ (qualified with Python 3.13.7).")

    root = Path(__file__).resolve().parents[2]
    output = root / "target" / "uniffi-python-consumer"
    generated = output / "generated"
    generated.mkdir(parents=True, exist_ok=True)
    # Keep native artifacts separate from the existing Swift/profile workflows.
    env = dict(os.environ, CARGO_TARGET_DIR=str(output / "cargo"), PYTHONDONTWRITEBYTECODE="1")
    cargo = ["cargo", "+1.97.0"]
    common = ["--locked", "-p", "cribra-uniffi", "--no-default-features"]

    def run(args, **kwargs):
        subprocess.run(args, cwd=root, env=env, check=True, **kwargs)

    run(cargo + ["build"] + common + ["--features", "core", "--lib"])
    if sys.platform == "darwin":
        library = "libcribra_uniffi.dylib"
    elif sys.platform == "win32":
        library = "cribra_uniffi.dll"
    else:
        library = "libcribra_uniffi.so"
    shutil.copy2(output / "cargo" / "debug" / library, generated / library)
    run(cargo + ["run"] + common + [
        "--features", "core,bindgen", "--bin", "cribra-uniffi-bindgen", "--",
        "generate", "--library", str(generated / library), "--language", "python",
        "--out-dir", str(generated),
    ])
    oracle = output / "core-oracle.json"
    with oracle.open("w", encoding="utf-8") as destination:
        run(cargo + ["run"] + common + [
            "--features", "core", "--example", "scanner_oracle",
        ], stdout=destination)
    env["PYTHONPATH"] = str(generated)
    run([
        sys.executable, "-B", str(root / "bindings/python/test_consumer.py"),
        str(generated), str(root / "crates/cribra-uniffi/tests/fixtures/scanner.json"),
        str(oracle),
    ])


if __name__ == "__main__":
    main()
