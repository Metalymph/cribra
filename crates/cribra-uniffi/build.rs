use std::{env, fs, path::Path};

fn main() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Cargo.toml");
    println!("cargo:rerun-if-changed={}", manifest.display());
    let source = fs::read_to_string(manifest).expect("read Core manifest");
    let core: toml::Table = source.parse().expect("parse Core manifest");
    let version = core["package"]["version"]
        .as_str()
        .expect("Core package.version must be a string");
    println!("cargo:rustc-env=CRIBRA_CORE_VERSION={version}");
}
