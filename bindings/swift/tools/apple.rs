//! Dependency-free Apple packaging/consumer driver. Run through the just recipes.
use std::{
    env,
    error::Error,
    ffi::OsStr,
    fs, io,
    path::{Path, PathBuf},
    process::{Command, Output},
    time::{SystemTime, UNIX_EPOCH},
};
type Result<T> = std::result::Result<T, Box<dyn Error>>;
const TOOLCHAIN: &str = "1.97.0";
const UNIFFI: &str = "0.32.2";
const TARGETS: [&str; 3] = [
    "aarch64-apple-darwin",
    "aarch64-apple-ios",
    "aarch64-apple-ios-sim",
];

fn fail(message: impl Into<String>) -> Box<dyn Error> {
    io::Error::other(message.into()).into()
}
fn run(command: &mut Command) -> Result<()> {
    eprintln!("+ {command:?}");
    let status = command.status()?;
    if !status.success() {
        return Err(fail(format!("command failed ({status}): {command:?}")));
    }
    Ok(())
}
fn capture(command: &mut Command) -> Result<String> {
    let Output {
        status,
        stdout,
        stderr,
    } = command.output()?;
    if !status.success() {
        return Err(fail(format!(
            "command failed ({status}): {command:?}\n{}",
            String::from_utf8_lossy(&stderr)
        )));
    }
    Ok(String::from_utf8(stdout)?.trim().to_owned())
}
fn cargo(root: &Path, target_dir: &Path) -> Command {
    let mut c = Command::new("cargo");
    c.current_dir(root)
        .arg(format!("+{TOOLCHAIN}"))
        .env("CARGO_TARGET_DIR", target_dir)
        // Xcode 27 rejects stripped host proc-macro dylibs (mis-aligned LINKEDIT).
        // Keep release optimization and explicitly disable stripping for this build.
        .env("CARGO_PROFILE_RELEASE_STRIP", "none");
    c
}
fn package(root: &Path, profile: &str) -> Result<PathBuf> {
    let base = root.join("bindings/swift");
    Ok(match profile {
        "core" => base,
        "flow" => base.join("Profiles/Flow"),
        "fuse" => base.join("Profiles/Fuse"),
        _ => return Err(fail("profile must be core, flow or fuse")),
    })
}
fn sdk(name: &str) -> Result<String> {
    capture(Command::new("xcrun").args(["--sdk", name, "--show-sdk-path"]))
}
fn version(manifest: &Path, key: &str) -> Result<String> {
    let source = fs::read_to_string(manifest)?;
    source
        .lines()
        .find_map(|line| {
            line.strip_prefix(&format!("{key} = \""))
                .and_then(|s| s.strip_suffix('"'))
                .map(str::to_owned)
        })
        .ok_or_else(|| fail(format!("missing {key} in {}", manifest.display())))
}
fn sha(path: &Path) -> Result<String> {
    let output = capture(Command::new("shasum").args(["-a", "256"]).arg(path))?;
    Ok(output
        .split_whitespace()
        .next()
        .ok_or_else(|| fail("missing SHA-256"))?
        .to_owned())
}
fn files(path: &Path, result: &mut Vec<PathBuf>) -> Result<()> {
    for item in fs::read_dir(path)? {
        let item = item?;
        if item.file_type()?.is_dir() {
            files(&item.path(), result)?;
        } else {
            result.push(item.path());
        }
    }
    Ok(())
}
fn source_inputs(root: &Path) -> Result<String> {
    let mut paths = vec![
        root.join("Cargo.toml"),
        root.join("Cargo.lock"),
        root.join("crates/cribra-uniffi/Cargo.toml"),
        root.join("crates/cribra-uniffi/build.rs"),
        root.join("bindings/swift/uniffi.toml"),
    ];
    files(&root.join("src"), &mut paths)?;
    files(&root.join("crates/cribra-uniffi/src"), &mut paths)?;
    paths.sort();
    let mut command = Command::new("shasum");
    command
        .current_dir(root)
        .args(["-a", "256"])
        .args(paths.iter().map(|p| p.strip_prefix(root).unwrap()));
    capture(&mut command)
}
struct BuildLock(PathBuf);
impl Drop for BuildLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
fn lock(base: &Path) -> Result<BuildLock> {
    fs::create_dir_all(base)?;
    let path = base.join("build.lock");
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|e| {
            fail(format!(
                "cannot acquire {} ({e}); another build may be active",
                path.display()
            ))
        })?;
    fs::write(&path, format!("pid={}\n", std::process::id()))?;
    Ok(BuildLock(path))
}
fn generate(root: &Path, library: &Path, out: &Path) -> Result<()> {
    run(
        Command::new(root.join("target/debug/cribra-uniffi-bindgen"))
            .current_dir(root)
            .args(["generate", "--library"])
            .arg(library)
            .args(["--language", "swift", "--config"])
            .arg(root.join("bindings/swift/uniffi.toml"))
            .arg("--out-dir")
            .arg(out),
    )
}
fn inspect_archive(path: &Path, platform: &str) -> Result<()> {
    if !fs::read(path)?.starts_with(b"!<arch>\n") {
        return Err(fail("native slice is not a static ar archive"));
    }
    let architecture = capture(Command::new("xcrun").args(["lipo", "-archs"]).arg(path))?;
    if architecture != "arm64" {
        return Err(fail(format!("unexpected architecture: {architecture}")));
    }
    let output = capture(Command::new("xcrun").args(["otool", "-l"]).arg(path))?;
    let platforms: Vec<_> = output
        .lines()
        .filter_map(|l| l.trim().strip_prefix("platform "))
        .collect();
    let number = match platform {
        "macos" => "1",
        "ios" => "2",
        "simulator" => "7",
        _ => unreachable!(),
    };
    if platforms.is_empty()
        || platforms.iter().any(|p| {
            *p != number && *p != platform && !(platform == "simulator" && *p == "IOSSIMULATOR")
        })
    {
        return Err(fail(format!(
            "incorrect Mach-O platform tags in {}: {:?}",
            path.display(),
            platforms
                .into_iter()
                .collect::<std::collections::BTreeSet<_>>()
        )));
    }
    println!("PASS: static arm64 slice {platform}: {}", path.display());
    Ok(())
}
fn plist(path: &Path, key: &str) -> Result<String> {
    capture(
        Command::new("plutil")
            .args(["-extract", key, "raw", "-o", "-"])
            .arg(path),
    )
}
fn inspect_framework(framework: &Path) -> Result<()> {
    let info = framework.join("Info.plist");
    let expected = [
        ("macos-arm64", "macos", "macos"),
        ("ios-arm64", "ios", "ios"),
        ("ios-arm64-simulator", "ios", "simulator"),
    ];
    let mut seen = std::collections::BTreeSet::new();
    for i in 0..3 {
        let prefix = format!("AvailableLibraries.{i}");
        let id = plist(&info, &format!("{prefix}.LibraryIdentifier"))?;
        let (_, platform, tag) = expected
            .iter()
            .find(|(identifier, _, _)| *identifier == id)
            .ok_or_else(|| fail(format!("unexpected XCFramework slice {id}")))?;
        if !seen.insert(id.clone()) {
            return Err(fail("duplicate XCFramework slice"));
        }
        if plist(&info, &format!("{prefix}.SupportedPlatform"))? != *platform {
            return Err(fail("XCFramework platform mismatch"));
        }
        if plist(&info, &format!("{prefix}.SupportedArchitectures.0"))? != "arm64" {
            return Err(fail("XCFramework architecture mismatch"));
        }
        let variants = Command::new("plutil")
            .args([
                "-extract",
                &format!("{prefix}.SupportedPlatformVariant"),
                "raw",
                "-o",
                "-",
            ])
            .arg(&info)
            .output()?;
        if *tag == "simulator" {
            if !variants.status.success()
                || String::from_utf8(variants.stdout)?.trim() != "simulator"
            {
                return Err(fail("missing simulator variant"));
            }
        } else if variants.status.success() {
            return Err(fail("unexpected platform variant"));
        }
        let library = plist(&info, &format!("{prefix}.LibraryPath"))?;
        if library != "libcribra_uniffi.a" {
            return Err(fail(
                "XCFramework must contain only the Rust static library",
            ));
        }
        inspect_archive(&framework.join(&id).join(&library), tag)?;
        let headers = plist(&info, &format!("{prefix}.HeadersPath"))?;
        for name in ["CribraFFI.h", "module.modulemap"] {
            if !framework.join(&id).join(&headers).join(name).is_file() {
                return Err(fail(format!("missing {name}")));
            }
        }
    }
    // Reject an accidental fourth slice rather than silently ignoring it.
    if Command::new("plutil")
        .args(["-extract", "AvailableLibraries.3", "json", "-o", "-"])
        .arg(&info)
        .output()?
        .status
        .success()
    {
        return Err(fail("unexpected extra XCFramework slice"));
    }
    println!(
        "PASS: XCFramework has macOS/device/simulator variants, arm64 static archives and headers"
    );
    Ok(())
}
fn check_contract(root: &Path, base: &Path, profile: &str, source: &Path) -> Result<()> {
    let executable = base.join("api-contract");
    run(Command::new("swiftc")
        .args([
            "-swift-version",
            "6",
            "-parse-as-library",
            "-module-cache-path",
        ])
        .arg(base.join("module-cache"))
        .arg(root.join("crates/cribra-uniffi/tests/swift/APIContract.swift"))
        .arg("-o")
        .arg(&executable))?;
    run(Command::new(executable)
        .arg(profile)
        .arg(root.join("crates/cribra-uniffi/tests/fixtures/api-contract.json"))
        .arg(source))
}
fn build(root: &Path, profile: &str) -> Result<()> {
    if env::consts::OS != "macos" || env::consts::ARCH != "aarch64" {
        return Err(fail("native macOS ARM64 is required"));
    }
    let destination = package(root, profile)?;
    let base = root.join("target/swift-apple");
    let _lock = lock(&base)?;
    let before = source_inputs(root)?;
    let adapter_manifest = root.join("crates/cribra-uniffi/Cargo.toml");
    if version(&adapter_manifest, "rust-version")? != "1.97"
        || !fs::read_to_string(&adapter_manifest)?.contains("uniffi = { version = \"=0.32.2\"")
    {
        return Err(fail(
            "adapter MSRV/UniFFI pins do not match this build driver",
        ));
    }
    let compiler = capture(Command::new("rustc").args(["+1.97.0", "--version"]))?;
    if !compiler.starts_with("rustc 1.97.0 ") {
        return Err(fail("Rust 1.97.0 is required"));
    }
    if !capture(Command::new("swiftc").arg("--version"))?.contains("Swift version 6.4") {
        return Err(fail("Swift 6.4 is required"));
    }
    let installed = capture(Command::new("rustup").args([
        "target",
        "list",
        "--installed",
        "--toolchain",
        TOOLCHAIN,
    ]))?;
    for target in TARGETS {
        if !installed.lines().any(|t| t == target) {
            return Err(fail(format!(
                "missing Rust target {target}; run rustup target add --toolchain {TOOLCHAIN} {target}"
            )));
        }
    }
    let staged = destination.join(format!("Generated.staging-{}", std::process::id()));
    fs::create_dir(&staged)?;
    let work = base.join("work").join(profile);
    fs::create_dir_all(&work)?;
    let cache = base.join("cargo").join(profile);
    let sdks = [sdk("macosx")?, sdk("iphoneos")?, sdk("iphonesimulator")?];
    let mut slices = Vec::new();
    for (index, target) in TARGETS.iter().enumerate() {
        // --crate-type overrides cdylib/rlib for this artifact build: never use host dylibs on iOS.
        run(cargo(root, &cache)
            .args([
                "rustc",
                "--locked",
                "-p",
                "cribra-uniffi",
                "--no-default-features",
                "--features",
                profile,
                "--release",
                "--target",
                target,
                "--lib",
                "--crate-type",
                "staticlib",
            ])
            .env("SDKROOT", &sdks[index])
            .env("MACOSX_DEPLOYMENT_TARGET", "13.0")
            .env("IPHONEOS_DEPLOYMENT_TARGET", "16.0"))?;
        let dir = work.join(target);
        fs::create_dir_all(&dir)?;
        let library = dir.join("libcribra_uniffi.a");
        fs::copy(
            cache.join(target).join("release/libcribra_uniffi.a"),
            &library,
        )?;
        slices.push(library);
    }
    run(cargo(root, &root.join("target")).args([
        "build",
        "--locked",
        "-p",
        "cribra-uniffi",
        "--no-default-features",
        "--features",
        "bindgen",
        "--bin",
        "cribra-uniffi-bindgen",
    ]))?;
    let mut generated = Vec::new();
    for (index, library) in slices.iter().enumerate() {
        let out = work.join(format!("bindings-{}", TARGETS[index]));
        generate(root, library, &out)?;
        generated.push(out);
    }
    for name in ["Cribra.swift", "CribraFFI.h", "CribraFFI.modulemap"] {
        let reference = fs::read(generated[0].join(name))?;
        for other in &generated[1..] {
            if fs::read(other.join(name))? != reference {
                return Err(fail(format!("incompatible {name} between native targets")));
            }
        }
    }
    check_contract(root, &base, profile, &generated[0].join("Cribra.swift"))?;
    let headers = work.join("Headers");
    fs::create_dir_all(&headers)?;
    fs::copy(
        generated[0].join("CribraFFI.h"),
        headers.join("CribraFFI.h"),
    )?;
    fs::copy(
        generated[0].join("CribraFFI.modulemap"),
        headers.join("module.modulemap"),
    )?;
    let framework = staged.join("CribraFFI.xcframework");
    let mut xc = Command::new("xcodebuild");
    xc.arg("-create-xcframework");
    for library in &slices {
        xc.arg("-library")
            .arg(library)
            .arg("-headers")
            .arg(&headers);
    }
    run(xc.arg("-output").arg(&framework))?;
    inspect_framework(&framework)?;
    let sources = staged.join("Sources/Cribra");
    fs::create_dir_all(&sources)?;
    fs::copy(
        generated[0].join("Cribra.swift"),
        sources.join("Cribra.swift"),
    )?;
    let core_version = version(&root.join("Cargo.toml"), "version")?;
    let adapter_version = version(&root.join("crates/cribra-uniffi/Cargo.toml"), "version")?;
    fs::write(
        sources.join("BuildInfo.swift"),
        format!(
            "// Generated with this profile's native slices. Do not mix profiles.\npublic enum CribraBuild {{\n    public static let profile = \"{profile}\"\n    public static let coreVersion = \"{core_version}\"\n    public static let adapterVersion = \"{adapter_version}\"\n    public static let uniffiVersion = \"{UNIFFI}\"\n    public static let rustVersion = \"{TOOLCHAIN}\"\n}}\n"
        ),
    )?;
    fs::write(staged.join("SourceInputs.sha256"), &before)?;
    fs::write(
        staged.join("BuildInfo.txt"),
        format!(
            "profile={profile}\ncore={core_version}\nadapter={adapter_version}\nuniffi={UNIFFI}\nrust={TOOLCHAIN}\nmacos_min=13.0\nios_min=16.0\n"
        ),
    )?;
    let mut hashes = String::new();
    let mut artifacts = Vec::new();
    files(&staged, &mut artifacts)?;
    artifacts.sort();
    for path in artifacts {
        hashes.push_str(&format!(
            "{}  {}\n",
            sha(&path)?,
            path.strip_prefix(&staged)?.display()
        ));
    }
    fs::write(staged.join("Artifacts.sha256"), hashes)?;
    if before != source_inputs(root)? {
        return Err(fail(
            "Rust inputs changed during packaging; staged artifacts were not published",
        ));
    }
    let published = destination.join("Generated");
    let previous = destination.join(format!("Generated.previous-{}", std::process::id()));
    if published.exists() {
        if !fs::read_to_string(published.join("BuildInfo.txt"))?
            .starts_with(&format!("profile={profile}\n"))
        {
            return Err(fail(
                "refusing to replace an unrecognized Generated directory",
            ));
        }
        fs::rename(&published, &previous)?;
    }
    if let Err(error) = fs::rename(&staged, &published) {
        if previous.exists() {
            fs::rename(&previous, &published)?;
        }
        return Err(error.into());
    }
    if previous.exists() {
        fs::remove_dir_all(previous)?;
    }
    println!(
        "PASS: complete {profile} Swift Package: {}",
        destination.display()
    );
    Ok(())
}
fn copy_tree(from: &Path, to: &Path) -> Result<()> {
    fs::create_dir(to)?;
    for item in fs::read_dir(from)? {
        let item = item?;
        if item.file_type()?.is_dir() {
            copy_tree(&item.path(), &to.join(item.file_name()))?;
        } else {
            fs::copy(item.path(), to.join(item.file_name()))?;
        }
    }
    Ok(())
}
fn swiftpm(root: &Path, verb: &str, consumer: &Path, scratch: &Path, state: &Path) -> Command {
    let mut c = Command::new("swift");
    c.current_dir(root)
        .arg(verb)
        .arg("--package-path")
        .arg(consumer)
        .arg("--scratch-path")
        .arg(scratch)
        .arg("--cache-path")
        .arg(state.join("cache"))
        .arg("--config-path")
        .arg(state.join("config"))
        .arg("--security-path")
        .arg(state.join("security"))
        .arg("--disable-sandbox")
        .env("CLANG_MODULE_CACHE_PATH", state.join("modules"))
        .env("SWIFTPM_MODULECACHE_OVERRIDE", state.join("modules"));
    c
}
fn consumer(root: &Path, profile: &str, requested: Option<&OsStr>) -> Result<()> {
    let distribution = package(root, profile)?;
    let generated = distribution.join("Generated");
    if !fs::read_to_string(generated.join("BuildInfo.txt"))?
        .starts_with(&format!("profile={profile}\n"))
    {
        return Err(fail("wrong or missing generated profile"));
    }
    run(Command::new("shasum").current_dir(&generated).args([
        "-a",
        "256",
        "-c",
        "Artifacts.sha256",
    ]))?;
    let external = if let Some(path) = requested.filter(|s| !s.is_empty()) {
        PathBuf::from(path)
    } else {
        env::temp_dir().join(format!(
            "cribra-apple-{profile}-{}-{}",
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs()
        ))
    };
    if !external.is_absolute() {
        return Err(fail("consumer directory must be absolute"));
    }
    let parent = external
        .parent()
        .ok_or_else(|| fail("consumer needs a parent directory"))?
        .canonicalize()?;
    let name = external
        .file_name()
        .ok_or_else(|| fail("consumer needs a directory name"))?;
    let external = parent.join(name);
    if external.starts_with(root) {
        return Err(fail("consumer must be outside the repository"));
    }
    fs::create_dir(&external)?;
    let copied = external.join("CribraDistribution");
    fs::create_dir(&copied)?;
    fs::copy(
        distribution.join("Package.swift"),
        copied.join("Package.swift"),
    )?;
    copy_tree(&generated, &copied.join("Generated"))?;
    // Recheck the portable copy: concurrent replacement must not produce a mixed set.
    run(Command::new("shasum")
        .current_dir(copied.join("Generated"))
        .args(["-a", "256", "-c", "Artifacts.sha256"]))?;
    let consumer = external.join("Consumer");
    copy_tree(&root.join("bindings/swift/Consumer"), &consumer)?;
    let template = fs::read_to_string(consumer.join("Package.swift"))?;
    let defines = match profile {
        "core" => "",
        "flow" => ".define(\"CRIBRA_FLOW\"),",
        "fuse" => ".define(\"CRIBRA_FLOW\"), .define(\"CRIBRA_FUSE\"),",
        _ => unreachable!(),
    };
    fs::write(
        consumer.join("Package.swift"),
        template.replace("// PROFILE_DEFINES", defines),
    )?;
    let state = external.join("state");
    fs::create_dir_all(&state)?;
    let mac_scratch = external.join("macos-build");
    run(swiftpm(root, "package", &consumer, &mac_scratch, &state).arg("resolve"))?;
    run(swiftpm(root, "build", &consumer, &mac_scratch, &state).args(["--product", "Consumer"]))?;
    let bin =
        capture(swiftpm(root, "build", &consumer, &mac_scratch, &state).arg("--show-bin-path"))?;
    run(Command::new(Path::new(&bin).join("Consumer")).args([profile]))?;
    let sim_scratch = external.join("simulator-build");
    run(
        swiftpm(root, "build", &consumer, &sim_scratch, &state).args([
            "--triple",
            "arm64-apple-ios16.0-simulator",
            "--sdk",
            &sdk("iphonesimulator")?,
            "--product",
            "Consumer",
        ]),
    )?;
    println!(
        "PASS: external {profile} consumer resolved, ran on macOS and compiled for iOS Simulator: {}",
        external.display()
    );
    Ok(())
}
fn main() {
    let result = (|| -> Result<()> {
        let args: Vec<_> = env::args_os().collect();
        if args.len() < 4 {
            return Err(fail(
                "usage: apple <repository> <build|consumer|inspect> <core|flow|fuse> [external-directory]",
            ));
        }
        let root = PathBuf::from(&args[1]).canonicalize()?;
        let action = args[2].to_str().ok_or_else(|| fail("invalid action"))?;
        let profile = args[3].to_str().ok_or_else(|| fail("invalid profile"))?;
        match action {
            "build" => build(&root, profile),
            "consumer" => consumer(&root, profile, args.get(4).map(OsStr::new)),
            "inspect" => {
                inspect_framework(&package(&root, profile)?.join("Generated/CribraFFI.xcframework"))
            }
            _ => Err(fail("unknown action")),
        }
    })();
    if let Err(error) = result {
        eprintln!("ERROR: {error}");
        std::process::exit(1);
    }
}
