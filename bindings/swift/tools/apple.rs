//! Dependency-free Apple packaging/consumer driver. Run through the just recipes.
use std::{
    collections::{BTreeMap, BTreeSet},
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
const DIST_REPOSITORY: &str = "Metalymph/cribra-swift";
const DIST_PACKAGE: &str = "cribra-swift";
const DIST_NAME: &str = "Cribra";
const DIST_BINARY: &str = "CribraFFI";
const SWIFT_TOOLS: &str = "6.4";
const MACOS_MIN: &str = "13.0";
const IOS_MIN: &str = "16.0";
const DIST_PROFILE: &str = "fuse";

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
fn inspect_deployment(path: &Path, platform: &str) -> Result<()> {
    let expected = match platform {
        "macos" => MACOS_MIN,
        "ios" | "simulator" => IOS_MIN,
        _ => unreachable!(),
    };
    let output = capture(Command::new("xcrun").args(["otool", "-l"]).arg(path))?;
    let mut versions = Vec::new();
    let mut parsed = Vec::new();
    for line in output.lines() {
        if let Some(value) = line.trim().strip_prefix("minos ") {
            let value = value.trim();
            versions.push(value.to_owned());
            parsed.push(
                value
                    .parse::<f64>()
                    .map_err(|_| fail(format!("unreadable deployment minimum {value:?}")))?,
            );
        }
    }
    let maximum = parsed.iter().copied().fold(0.0, f64::max);
    if parsed.is_empty() || (maximum - expected.parse::<f64>()?).abs() > f64::EPSILON {
        return Err(fail(format!(
            "unexpected {platform} deployment minimum in {}: {versions:?}; expected {expected}",
            path.display()
        )));
    }
    println!(
        "PASS: {platform} deployment minimum {expected}: {}",
        path.display()
    );
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
        inspect_deployment(&framework.join(&id).join(&library), tag)?;
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
fn plutil_field(path: &Path, key: &str) -> Result<Option<String>> {
    let output = Command::new("plutil")
        .args(["-extract", key, "raw", "-o", "-"])
        .arg(path)
        .output()?;
    if output.status.success() {
        Ok(Some(String::from_utf8(output.stdout)?.trim().to_owned()))
    } else {
        Ok(None)
    }
}
fn build_info(path: &Path) -> Result<BTreeMap<String, String>> {
    let mut info = BTreeMap::new();
    for line in fs::read_to_string(path)?.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let (key, value) = line
            .split_once('=')
            .ok_or_else(|| fail(format!("malformed line in {}: {line:?}", path.display())))?;
        if info
            .insert(key.trim().to_owned(), value.trim().to_owned())
            .is_some()
        {
            return Err(fail(format!(
                "duplicate {} in {}",
                key.trim(),
                path.display()
            )));
        }
    }
    Ok(info)
}
fn expect_build_info(info: &BTreeMap<String, String>, key: &str, expected: &str) -> Result<()> {
    match info.get(key) {
        Some(value) if value == expected => Ok(()),
        Some(value) => Err(fail(format!(
            "generated BuildInfo.txt has {key}={value}, expected {expected}; rerun `just swift-apple fuse`"
        ))),
        None => Err(fail(format!(
            "generated BuildInfo.txt is missing {key}; rerun `just swift-apple fuse`"
        ))),
    }
}
fn swift_version() -> Result<String> {
    let output = capture(Command::new("swiftc").arg("--version"))?;
    output
        .split("Swift version ")
        .nth(1)
        .and_then(|value| value.split_whitespace().next())
        .map(str::to_owned)
        .ok_or_else(|| fail("cannot determine the Swift compiler version"))
}
fn git(root: &Path, args: &[&str]) -> Result<String> {
    capture(Command::new("git").current_dir(root).args(args))
}
fn json_escape(value: &str) -> String {
    let mut escaped = String::new();
    for character in value.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            character if character < ' ' => {
                escaped.push_str(&format!("\\u{:04x}", character as u32))
            }
            character => escaped.push(character),
        }
    }
    escaped
}
fn json_string(value: &str) -> String {
    format!("\"{}\"", json_escape(value))
}
fn required_archive_entries() -> Vec<String> {
    let root = format!("{DIST_BINARY}.xcframework");
    let mut entries = vec![format!("{root}/Info.plist")];
    for slice in ["macos-arm64", "ios-arm64", "ios-arm64-simulator"] {
        entries.push(format!("{root}/{slice}/libcribra_uniffi.a"));
        entries.push(format!("{root}/{slice}/Headers/{DIST_BINARY}.h"));
        entries.push(format!("{root}/{slice}/Headers/module.modulemap"));
    }
    entries
}
fn validate_archive_entries(entries: &[String]) -> Result<()> {
    if entries.is_empty() {
        return Err(fail("XCFramework archive is empty"));
    }
    let root = format!("{DIST_BINARY}.xcframework");
    let mut roots = BTreeSet::new();
    for entry in entries {
        let normalized = entry.trim_end_matches('/');
        if normalized.is_empty()
            || entry.starts_with('/')
            || entry.contains('\\')
            || normalized.contains("__MACOSX")
            || normalized.contains(".DS_Store")
        {
            return Err(fail(format!(
                "unexpected XCFramework archive entry {entry:?}"
            )));
        }
        let parts: Vec<_> = normalized.split('/').collect();
        if parts
            .iter()
            .any(|part| part.is_empty() || *part == "." || *part == "..")
        {
            return Err(fail(format!("unsafe XCFramework archive entry {entry:?}")));
        }
        roots.insert(parts[0].to_owned());
    }
    if roots.len() != 1 || !roots.contains(&root) {
        return Err(fail(format!(
            "archive must contain only {root} at its root, found {roots:?}"
        )));
    }
    for required in required_archive_entries() {
        if !entries
            .iter()
            .any(|entry| entry.trim_end_matches('/') == required)
        {
            return Err(fail(format!(
                "archive is missing required XCFramework entry {required}"
            )));
        }
    }
    println!("PASS: archive contains {root} at its root with all required slices and headers");
    Ok(())
}
fn validate_staging_layout(staging: &Path, asset: &str) -> Result<()> {
    let mut expected = BTreeSet::new();
    for name in ["Package.swift", "README.md", "LICENSE", "distribution.json"] {
        expected.insert(name.to_owned());
    }
    expected.insert(asset.to_owned());
    expected.insert(format!("{asset}.checksum"));
    expected.insert(format!("{asset}.sha256"));
    expected.insert("Sources".to_owned());
    let mut actual = BTreeSet::new();
    for entry in fs::read_dir(staging)? {
        actual.insert(entry?.file_name().to_string_lossy().into_owned());
    }
    if expected != actual {
        let unexpected = actual.difference(&expected).cloned().collect::<Vec<_>>();
        let missing = expected.difference(&actual).cloned().collect::<Vec<_>>();
        return Err(fail(format!(
            "staged distribution has unexpected entries {unexpected:?} or missing entries {missing:?}"
        )));
    }
    println!("PASS: staged distribution contains only the expected package files");
    Ok(())
}
fn extract_zip(archive: &Path, destination: &Path) -> Result<()> {
    fs::create_dir_all(destination)?;
    run(Command::new("unzip")
        .args(["-q"])
        .arg(archive)
        .arg("-d")
        .arg(destination))
}
fn tree_hashes(root: &Path) -> Result<BTreeMap<String, String>> {
    let mut paths = Vec::new();
    files(root, &mut paths)?;
    let mut hashes = BTreeMap::new();
    for path in paths {
        let relative = path
            .strip_prefix(root)?
            .components()
            .map(|component| component.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");
        hashes.insert(relative, sha(&path)?);
    }
    Ok(hashes)
}
fn compare_trees(expected: &Path, actual: &Path) -> Result<()> {
    let expected_hashes = tree_hashes(expected)?;
    let actual_hashes = tree_hashes(actual)?;
    if expected_hashes == actual_hashes {
        println!(
            "PASS: extracted tree matches {}: {}",
            expected.display(),
            actual.display()
        );
        return Ok(());
    }
    let mut differences = Vec::new();
    for (path, hash) in &expected_hashes {
        match actual_hashes.get(path) {
            None => differences.push(format!("missing {path}")),
            Some(other) if other != hash => differences.push(format!("different {path}")),
            Some(_) => {}
        }
    }
    for path in actual_hashes.keys() {
        if !expected_hashes.contains_key(path) {
            differences.push(format!("unexpected {path}"));
        }
    }
    Err(fail(format!(
        "staged tree does not match {}: {}",
        expected.display(),
        differences.join(", ")
    )))
}
fn distribution_manifest(version: &str, url: &str, checksum: &str) -> String {
    format!(
        r#"// swift-tools-version: {SWIFT_TOOLS}
import PackageDescription

// UNPUBLISHED STAGING MANIFEST generated by `just swift-apple-dist fuse`.
// Swift distribution version {version}. The binaryTarget URL is a planned future
// GitHub Release asset and does not exist yet. Do not publish or resolve this
// package from that URL until the asset is uploaded and the remote-consumer
// acceptance gate in bindings/swift/README.md passes.
let package = Package(
    name: "{DIST_NAME}",
    platforms: [.macOS(.v13), .iOS(.v16)],
    products: [.library(name: "{DIST_NAME}", targets: ["{DIST_NAME}"])],
    targets: [
        .binaryTarget(
            name: "{DIST_BINARY}",
            url: "{url}",
            checksum: "{checksum}"
        ),
        .target(name: "{DIST_NAME}", dependencies: ["{DIST_BINARY}"], path: "Sources/{DIST_NAME}"),
    ]
)
"#
    )
}
fn validate_manifest_source(manifest: &str, url: &str, checksum: &str) -> Result<()> {
    if !manifest.starts_with(&format!("// swift-tools-version: {SWIFT_TOOLS}\n")) {
        return Err(fail(format!(
            "distribution manifest must declare swift-tools-version {SWIFT_TOOLS}"
        )));
    }
    if manifest.matches(".library(name:").count() != 1 {
        return Err(fail(
            "distribution manifest must expose exactly one public library product",
        ));
    }
    if !manifest.contains(&format!("name: \"{DIST_NAME}\""))
        || !manifest.contains(&format!("targets: [\"{DIST_NAME}\"]"))
    {
        return Err(fail(format!(
            "distribution manifest must expose product and target {DIST_NAME}"
        )));
    }
    if !manifest.contains(".binaryTarget(")
        || !manifest.contains(&format!("url: \"{url}\""))
        || !manifest.contains(&format!("checksum: \"{checksum}\""))
    {
        return Err(fail(
            "distribution manifest must use the remote binary target URL and checksum",
        ));
    }
    if manifest.contains(&format!("path: \"{DIST_BINARY}.xcframework\""))
        || manifest.contains("path: \"Generated")
    {
        return Err(fail(
            "distribution manifest must not use a local binary target path",
        ));
    }
    if manifest.contains(".package(") || manifest.contains(".plugin(") {
        return Err(fail(
            "distribution manifest must not add package dependencies or build plugins",
        ));
    }
    if !manifest.contains("UNPUBLISHED STAGING MANIFEST")
        || !manifest.contains(".macOS(.v13)")
        || !manifest.contains(".iOS(.v16)")
    {
        return Err(fail(
            "distribution manifest must be marked unpublished and preserve macOS 13/iOS 16",
        ));
    }
    Ok(())
}
fn validate_staged_manifest(staging: &Path, dump: &Path, url: &str, checksum: &str) -> Result<()> {
    let manifest_path = staging.join("Package.swift");
    validate_manifest_source(&fs::read_to_string(&manifest_path)?, url, checksum)?;
    let output = Command::new("swift")
        .args(["package", "dump-package", "--package-path"])
        .arg(staging)
        .arg("--scratch-path")
        .arg(dump.with_extension("scratch"))
        .output()?;
    if !output.status.success() {
        return Err(fail(format!(
            "swift package dump-package rejected the staged manifest:\n{}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    fs::write(dump, &output.stdout)?;
    let field = |key: &str| plutil_field(dump, key);
    if field("name")?.as_deref() != Some(DIST_NAME) {
        return Err(fail("staged manifest package name must be Cribra"));
    }
    let tools = field("toolsVersion._version")?
        .ok_or_else(|| fail("staged manifest has no swift-tools-version"))?;
    if tools != SWIFT_TOOLS && tools != format!("{SWIFT_TOOLS}.0") {
        return Err(fail(format!(
            "staged manifest tools version is {tools}, expected {SWIFT_TOOLS}"
        )));
    }
    if field("dependencies")?.as_deref() != Some("0") {
        return Err(fail(
            "staged manifest must not declare package dependencies",
        ));
    }
    let products: usize = field("products")?
        .ok_or_else(|| fail("staged manifest has no products"))?
        .parse()?;
    if products != 1
        || field("products.0.name")?.as_deref() != Some(DIST_NAME)
        || field("products.0.type")?.as_deref() != Some("library")
        || field("products.0.targets")?.as_deref() != Some("1")
        || field("products.0.targets.0")?.as_deref() != Some(DIST_NAME)
    {
        return Err(fail(
            "staged manifest must expose exactly one public library product `Cribra`",
        ));
    }
    let targets: usize = field("targets")?
        .ok_or_else(|| fail("staged manifest has no targets"))?
        .parse()?;
    if targets != 2 {
        return Err(fail("staged manifest must contain exactly two targets"));
    }
    let mut binary = None;
    let mut source = None;
    for index in 0..targets {
        match field(&format!("targets.{index}.type"))?.as_deref() {
            Some("binary") => binary = Some(index),
            Some("regular") => source = Some(index),
            Some(other) => {
                return Err(fail(format!(
                    "unexpected target type {other} in staged manifest"
                )));
            }
            None => return Err(fail("staged manifest target has no type")),
        }
    }
    let binary = binary.ok_or_else(|| fail("staged manifest has no binary target"))?;
    let source = source.ok_or_else(|| fail("staged manifest has no source target"))?;
    if field(&format!("targets.{binary}.name"))?.as_deref() != Some(DIST_BINARY)
        || field(&format!("targets.{binary}.url"))?.as_deref() != Some(url)
        || field(&format!("targets.{binary}.checksum"))?.as_deref() != Some(checksum)
    {
        return Err(fail(
            "staged manifest binary target must be the remote CribraFFI archive with the staged checksum",
        ));
    }
    if field(&format!("targets.{source}.name"))?.as_deref() != Some(DIST_NAME)
        || field(&format!("targets.{source}.path"))?.as_deref()
            != Some(&format!("Sources/{DIST_NAME}"))
        || field(&format!("targets.{source}.dependencies.0.byName.0"))?.as_deref()
            != Some(DIST_BINARY)
    {
        return Err(fail(
            "staged manifest source target must depend on CribraFFI from Sources/Cribra",
        ));
    }
    let platforms: usize = field("platforms")?
        .ok_or_else(|| fail("staged manifest has no platforms"))?
        .parse()?;
    if platforms != 2 {
        return Err(fail(
            "staged manifest must declare exactly macOS 13 and iOS 16",
        ));
    }
    let mut minimums = BTreeMap::new();
    for index in 0..platforms {
        let name = field(&format!("platforms.{index}.platformName"))?
            .ok_or_else(|| fail("staged manifest platform has no name"))?;
        let version = field(&format!("platforms.{index}.version"))?
            .ok_or_else(|| fail("staged manifest platform has no version"))?;
        minimums.insert(name, version);
    }
    if minimums.get("macos").map(String::as_str) != Some(MACOS_MIN)
        || minimums.get("ios").map(String::as_str) != Some(IOS_MIN)
    {
        return Err(fail(
            "staged manifest must preserve macOS 13 and iOS 16 deployment minimums",
        ));
    }
    println!(
        "PASS: staged manifest exposes one Cribra product and a remote CribraFFI binary target"
    );
    Ok(())
}
fn local_qualification_manifest() -> String {
    format!(
        r#"// swift-tools-version: {SWIFT_TOOLS}
import PackageDescription

// Temporary local qualification manifest. This is NOT the staged distribution
// manifest and is never copied into the distribution output.
let package = Package(
    name: "{DIST_NAME}",
    platforms: [.macOS(.v13), .iOS(.v16)],
    products: [.library(name: "{DIST_NAME}", targets: ["{DIST_NAME}"])],
    targets: [
        .binaryTarget(name: "{DIST_BINARY}", path: "{DIST_BINARY}.xcframework"),
        .target(name: "{DIST_NAME}", dependencies: ["{DIST_BINARY}"], path: "Sources/{DIST_NAME}"),
    ]
)
"#
    )
}
fn qualify_distribution(root: &Path, staging: &Path, archive: &Path) -> Result<PathBuf> {
    let external = env::temp_dir().join(format!(
        "cribra-swift-fuse-{}-{}",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs()
    ));
    let parent = external
        .parent()
        .ok_or_else(|| fail("qualification needs a parent directory"))?
        .canonicalize()?;
    let name = external
        .file_name()
        .ok_or_else(|| fail("qualification needs a directory name"))?;
    let external = parent.join(name);
    if external.starts_with(root) {
        return Err(fail(
            "distribution qualification must run outside the repository",
        ));
    }
    fs::create_dir(&external)?;
    let distribution = external.join("CribraDistribution");
    fs::create_dir(&distribution)?;
    fs::create_dir_all(distribution.join("Sources"))?;
    copy_tree(
        &staging.join(format!("Sources/{DIST_NAME}")),
        &distribution.join(format!("Sources/{DIST_NAME}")),
    )?;
    let extracted = external.join("archive");
    extract_zip(archive, &extracted)?;
    copy_tree(
        &extracted.join(format!("{DIST_BINARY}.xcframework")),
        &distribution.join(format!("{DIST_BINARY}.xcframework")),
    )?;
    fs::write(
        distribution.join("Package.swift"),
        local_qualification_manifest(),
    )?;
    let consumer = external.join("Consumer");
    copy_tree(&root.join("bindings/swift/Consumer"), &consumer)?;
    let template = fs::read_to_string(consumer.join("Package.swift"))?;
    if !template.contains("// PROFILE_DEFINES") {
        return Err(fail(
            "consumer template is missing the profile defines marker",
        ));
    }
    fs::write(
        consumer.join("Package.swift"),
        template.replace(
            "// PROFILE_DEFINES",
            ".define(\"CRIBRA_FLOW\"), .define(\"CRIBRA_FUSE\"),",
        ),
    )?;
    let state = external.join("state");
    fs::create_dir_all(&state)?;
    let mac_scratch = external.join("macos-build");
    run(swiftpm(root, "package", &consumer, &mac_scratch, &state).arg("resolve"))?;
    run(swiftpm(root, "build", &consumer, &mac_scratch, &state).args(["--product", "Consumer"]))?;
    let bin =
        capture(swiftpm(root, "build", &consumer, &mac_scratch, &state).arg("--show-bin-path"))?;
    run(Command::new(Path::new(&bin).join("Consumer")).args([DIST_PROFILE]))?;
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
        "PASS: staged distribution qualified with the Fuse consumer on macOS and arm64-apple-ios16.0-simulator: {}",
        external.display()
    );
    Ok(external)
}
struct Distribution {
    version: String,
    core_version: String,
    adapter_version: String,
    swift_version: String,
    git_revision: String,
    git_dirty: bool,
    timestamp: String,
    asset: String,
    url: String,
    checksum: String,
    archive_sha256: String,
    archive_size: u64,
    source_inputs_sha256: String,
    artifacts_sha256: String,
    generated_sources: Vec<(String, String)>,
}
fn distribution_metadata(distribution: &Distribution) -> String {
    let mut output = String::new();
    output.push_str("{\n");
    output.push_str("  \"schemaVersion\": 1,\n");
    output.push_str("  \"distribution\": {\n");
    output.push_str(&format!(
        "    \"profile\": {},\n",
        json_string(DIST_PROFILE)
    ));
    output.push_str(&format!(
        "    \"repository\": {},\n",
        json_string(DIST_REPOSITORY)
    ));
    output.push_str(&format!(
        "    \"package\": {},\n",
        json_string(DIST_PACKAGE)
    ));
    output.push_str(&format!("    \"name\": {},\n", json_string(DIST_NAME)));
    output.push_str(&format!("    \"product\": {},\n", json_string(DIST_NAME)));
    output.push_str(&format!(
        "    \"sourceTarget\": {},\n",
        json_string(DIST_NAME)
    ));
    output.push_str(&format!(
        "    \"binaryTarget\": {},\n",
        json_string(DIST_BINARY)
    ));
    output.push_str(&format!(
        "    \"version\": {},\n",
        json_string(&distribution.version)
    ));
    output.push_str("    \"published\": false\n");
    output.push_str("  },\n");
    output.push_str("  \"versions\": {\n");
    output.push_str(&format!(
        "    \"swiftDistribution\": {},\n",
        json_string(&distribution.version)
    ));
    output.push_str(&format!(
        "    \"rustCore\": {},\n",
        json_string(&distribution.core_version)
    ));
    output.push_str(&format!(
        "    \"uniffiAdapter\": {},\n",
        json_string(&distribution.adapter_version)
    ));
    output.push_str(&format!(
        "    \"uniffiGenerator\": {},\n",
        json_string(UNIFFI)
    ));
    output.push_str(&format!(
        "    \"rustToolchain\": {},\n",
        json_string(TOOLCHAIN)
    ));
    output.push_str(&format!(
        "    \"swiftToolchain\": {},\n",
        json_string(&distribution.swift_version)
    ));
    output.push_str(&format!(
        "    \"swiftToolsVersion\": {},\n",
        json_string(SWIFT_TOOLS)
    ));
    output.push_str(&format!(
        "    \"macosMinimum\": {},\n",
        json_string(MACOS_MIN)
    ));
    output.push_str(&format!("    \"iosMinimum\": {}\n", json_string(IOS_MIN)));
    output.push_str("  },\n");
    output.push_str("  \"source\": {\n");
    output.push_str(&format!(
        "    \"gitRevision\": {},\n",
        json_string(&distribution.git_revision)
    ));
    output.push_str(&format!("    \"gitDirty\": {},\n", distribution.git_dirty));
    output.push_str("    \"sourceInputs\": \"SourceInputs.sha256\",\n");
    output.push_str(&format!(
        "    \"sourceInputsSha256\": {},\n",
        json_string(&distribution.source_inputs_sha256)
    ));
    output.push_str("    \"artifacts\": \"Artifacts.sha256\",\n");
    output.push_str(&format!(
        "    \"artifactsSha256\": {},\n",
        json_string(&distribution.artifacts_sha256)
    ));
    output.push_str(&format!(
        "    \"generatedSourcesOrigin\": {},\n",
        json_string(&format!(
            "bindings/swift/Profiles/Fuse/Generated/Sources/{DIST_NAME}"
        ))
    ));
    output.push_str("    \"generatedSources\": [\n");
    for (index, (path, hash)) in distribution.generated_sources.iter().enumerate() {
        let comma = if index + 1 == distribution.generated_sources.len() {
            ""
        } else {
            ","
        };
        output.push_str(&format!(
            "      {{ \"path\": {}, \"sha256\": {} }}{}\n",
            json_string(path),
            json_string(hash),
            comma
        ));
    }
    output.push_str("    ]\n");
    output.push_str("  },\n");
    output.push_str("  \"archive\": {\n");
    output.push_str(&format!(
        "    \"filename\": {},\n",
        json_string(&distribution.asset)
    ));
    output.push_str(&format!(
        "    \"sha256\": {},\n",
        json_string(&distribution.archive_sha256)
    ));
    output.push_str(&format!(
        "    \"swiftPMChecksum\": {},\n",
        json_string(&distribution.checksum)
    ));
    output.push_str(&format!(
        "    \"sizeBytes\": {},\n",
        distribution.archive_size
    ));
    output.push_str(&format!(
        "    \"url\": {}\n",
        json_string(&distribution.url)
    ));
    output.push_str("  },\n");
    output.push_str("  \"platforms\": [\n");
    output.push_str(&format!(
        "    {{ \"platform\": \"macOS\", \"slice\": \"macos-arm64\", \"architecture\": \"arm64\", \"minimum\": {} }},\n",
        json_string(MACOS_MIN)
    ));
    output.push_str(&format!(
        "    {{ \"platform\": \"iOS\", \"slice\": \"ios-arm64\", \"architecture\": \"arm64\", \"minimum\": {} }},\n",
        json_string(IOS_MIN)
    ));
    output.push_str(&format!(
        "    {{ \"platform\": \"iOS Simulator\", \"slice\": \"ios-arm64-simulator\", \"architecture\": \"arm64\", \"minimum\": {} }}\n",
        json_string(IOS_MIN)
    ));
    output.push_str("  ],\n");
    output.push_str("  \"build\": {\n");
    output.push_str(&format!(
        "    \"timestamp\": {},\n",
        json_string(&distribution.timestamp)
    ));
    output.push_str("    \"command\": \"just swift-apple-dist fuse\",\n");
    output.push_str("    \"qualification\": {\n");
    output.push_str("      \"consumer\": \"bindings/swift/Consumer\",\n");
    output.push_str("      \"macos\": \"built and executed\",\n");
    output.push_str(
        "      \"iosSimulator\": \"compiled and linked for arm64-apple-ios16.0-simulator\"\n",
    );
    output.push_str("    }\n");
    output.push_str("  }\n");
    output.push_str("}\n");
    output
}
fn validate_distribution_metadata(path: &Path, distribution: &Distribution) -> Result<()> {
    let expected = [
        ("distribution.profile", DIST_PROFILE.to_owned()),
        ("distribution.published", "false".to_owned()),
        ("distribution.version", distribution.version.clone()),
        ("archive.filename", distribution.asset.clone()),
        ("archive.swiftPMChecksum", distribution.checksum.clone()),
        ("archive.sha256", distribution.archive_sha256.clone()),
        ("versions.swiftDistribution", distribution.version.clone()),
        ("versions.rustCore", distribution.core_version.clone()),
        (
            "versions.uniffiAdapter",
            distribution.adapter_version.clone(),
        ),
        ("versions.uniffiGenerator", UNIFFI.to_owned()),
        ("versions.rustToolchain", TOOLCHAIN.to_owned()),
        (
            "versions.swiftToolchain",
            distribution.swift_version.clone(),
        ),
    ];
    for (key, value) in expected {
        if plutil_field(path, key)?.as_deref() != Some(value.as_str()) {
            return Err(fail(format!(
                "distribution metadata {key} must equal {value:?}"
            )));
        }
    }
    println!(
        "PASS: distribution.json records the Fuse profile, versions, checksum and source provenance"
    );
    Ok(())
}
fn distribution_readme(distribution: &Distribution) -> String {
    format!(
        r#"# Cribra for Swift {version} (Fuse) — UNPUBLISHED STAGING

This directory is generated by `just swift-apple-dist fuse`. It is a local
staging package, **not a published release**. The binary URL in `Package.swift`
is a planned future GitHub Release asset and does not resolve today.

## Contents

- `Package.swift`: unpublished manifest exposing one public product `{name}`.
- `{asset}`: complete Fuse `{binary}.xcframework` with macOS, iOS and
  iOS Simulator ARM64 slices, headers and module maps.
- `{asset}.checksum`: the exact SwiftPM archive checksum.
- `{asset}.sha256`: the archive SHA-256 integrity digest.
- `Sources/{name}/{{Cribra.swift,BuildInfo.swift}}`: generated from the same
  Fuse build as the archive.
- `distribution.json`: machine-readable versions, source provenance and platform
  metadata.
- `LICENSE`: the repository license.

## Versions and platforms

Rust Core `{core}`, UniFFI adapter `{adapter}`, UniFFI generator `{uniffi}`,
Rust `{rust}`, Swift `{swift}`. Deployment minimums are macOS {macos}
and iOS {ios}. Only ARM64 macOS, iOS device and iOS Simulator slices are included.

## Checksum

```sh
swift package compute-checksum {asset}
```

This must print `{checksum}`. The checksum is also recorded in `Package.swift`
and `distribution.json`.

## Qualification

Packaging inspected the archive, extracted it, and compared every file with the
generated XCFramework. It validated the three ARM64 slices, headers, module maps,
macOS {macos}/iOS {ios} deployment minimums and the SwiftPM manifest, then ran
the existing external consumer on macOS and compiled it for
`arm64-apple-ios16.0-simulator`. The macOS run exercises scanning, batch scanning,
streaming, incremental scanning, transformations, Share Bundles and parallel
scanning. The iOS Simulator result is compile/link evidence, not simulator launch
or device execution.

## Publication status

Do not publish or resolve this package from the URL until the release asset
exists and the remote-consumer acceptance gate in `bindings/swift/README.md`
passes. This staging output is ignored by Git and is not committed.
"#,
        version = distribution.version,
        name = DIST_NAME,
        binary = DIST_BINARY,
        asset = distribution.asset,
        core = distribution.core_version,
        adapter = distribution.adapter_version,
        uniffi = UNIFFI,
        rust = TOOLCHAIN,
        swift = distribution.swift_version,
        macos = MACOS_MIN,
        ios = IOS_MIN,
        checksum = distribution.checksum,
    )
}
fn dist(root: &Path, profile: &str) -> Result<()> {
    if profile != DIST_PROFILE {
        return Err(fail(format!(
            "distribution packaging supports only the Fuse profile; requested {profile}"
        )));
    }
    if env::consts::OS != "macos" || env::consts::ARCH != "aarch64" {
        return Err(fail("native macOS ARM64 is required"));
    }
    let directory = package(root, profile)?;
    let generated = directory.join("Generated");
    if !generated.is_dir() {
        return Err(fail(format!(
            "missing {}; run `just swift-apple fuse` first",
            generated.display()
        )));
    }
    let base = root.join("target/swift-apple");
    let _lock = lock(&base)?;
    let adapter_manifest = root.join("crates/cribra-uniffi/Cargo.toml");
    if version(&adapter_manifest, "rust-version")? != "1.97"
        || !fs::read_to_string(&adapter_manifest)?.contains("uniffi = { version = \"=0.32.2\"")
    {
        return Err(fail(
            "adapter MSRV/UniFFI pins do not match this packaging driver",
        ));
    }
    let rust = capture(Command::new("rustc").args(["+1.97.0", "--version"]))?;
    if !rust.starts_with("rustc 1.97.0 ") {
        return Err(fail("Rust 1.97.0 is required"));
    }
    let swift = swift_version()?;
    if swift != SWIFT_TOOLS {
        return Err(fail(format!(
            "Swift {SWIFT_TOOLS} is required to package and qualify this distribution, found {swift}"
        )));
    }
    let core_version = version(&root.join("Cargo.toml"), "version")?;
    let adapter_version = version(&adapter_manifest, "version")?;
    let info = build_info(&generated.join("BuildInfo.txt"))?;
    expect_build_info(&info, "profile", DIST_PROFILE)?;
    expect_build_info(&info, "core", &core_version)?;
    expect_build_info(&info, "adapter", &adapter_version)?;
    expect_build_info(&info, "uniffi", UNIFFI)?;
    expect_build_info(&info, "rust", TOOLCHAIN)?;
    expect_build_info(&info, "macos_min", MACOS_MIN)?;
    expect_build_info(&info, "ios_min", IOS_MIN)?;
    let source_inputs_recorded = fs::read_to_string(generated.join("SourceInputs.sha256"))?;
    let source_inputs_current = source_inputs(root)?;
    if source_inputs_recorded.trim() != source_inputs_current {
        return Err(fail(
            "generated artifacts were built from different Rust/UniFFI inputs; rerun `just swift-apple fuse`",
        ));
    }
    run(Command::new("shasum").current_dir(&generated).args([
        "-a",
        "256",
        "-c",
        "Artifacts.sha256",
    ]))?;
    let framework = generated.join(format!("{DIST_BINARY}.xcframework"));
    if !framework.is_dir() {
        return Err(fail(format!(
            "missing {}; rerun `just swift-apple fuse`",
            framework.display()
        )));
    }
    inspect_framework(&framework)?;
    let generated_sources = generated.join(format!("Sources/{DIST_NAME}"));
    for required in ["Cribra.swift", "BuildInfo.swift"] {
        if !generated_sources.join(required).is_file() {
            return Err(fail(format!(
                "missing generated source {required}; rerun `just swift-apple fuse`"
            )));
        }
    }
    let build_info_swift = fs::read_to_string(generated_sources.join("BuildInfo.swift"))?;
    for expected in [
        format!("profile = \"{DIST_PROFILE}\""),
        format!("coreVersion = \"{core_version}\""),
        format!("adapterVersion = \"{adapter_version}\""),
        format!("uniffiVersion = \"{UNIFFI}\""),
        format!("rustVersion = \"{TOOLCHAIN}\""),
    ] {
        if !build_info_swift.contains(&expected) {
            return Err(fail(format!(
                "generated BuildInfo.swift is missing {expected}; rerun `just swift-apple fuse`"
            )));
        }
    }
    let dist_root = root.join("target/swift-distribution");
    fs::create_dir_all(&dist_root)?;
    let release = format!("{DIST_PROFILE}-{core_version}");
    let staging = dist_root.join(format!("{release}.staging-{}", std::process::id()));
    fs::create_dir(&staging)?;
    let asset = format!("{DIST_BINARY}-{DIST_PROFILE}-{core_version}.xcframework.zip");
    let archive = staging.join(&asset);
    run(Command::new("zip")
        .current_dir(&generated)
        .args(["-q", "-r", "-X"])
        .arg(&archive)
        .arg(format!("{DIST_BINARY}.xcframework")))?;
    let checksum = capture(
        Command::new("swift")
            .args(["package", "compute-checksum"])
            .arg(&archive),
    )?;
    let recomputed = capture(
        Command::new("swift")
            .args(["package", "compute-checksum"])
            .arg(&archive),
    )?;
    if checksum != recomputed {
        return Err(fail(
            "SwiftPM archive checksum changed between computations",
        ));
    }
    if checksum.len() != 64
        || !checksum
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        return Err(fail(format!(
            "invalid SwiftPM archive checksum {checksum:?}"
        )));
    }
    let archive_sha256 = sha(&archive)?;
    let checksum_file = staging.join(format!("{asset}.checksum"));
    fs::write(&checksum_file, format!("{checksum}\n"))?;
    let sha_file = staging.join(format!("{asset}.sha256"));
    fs::write(&sha_file, format!("{archive_sha256}  {asset}\n"))?;
    run(Command::new("shasum")
        .current_dir(&staging)
        .args(["-a", "256", "-c"])
        .arg(format!("{asset}.sha256")))?;
    let url =
        format!("https://github.com/{DIST_REPOSITORY}/releases/download/{core_version}/{asset}");
    fs::create_dir_all(staging.join("Sources"))?;
    copy_tree(
        &generated_sources,
        &staging.join(format!("Sources/{DIST_NAME}")),
    )?;
    compare_trees(
        &generated_sources,
        &staging.join(format!("Sources/{DIST_NAME}")),
    )?;
    let manifest = distribution_manifest(&core_version, &url, &checksum);
    validate_manifest_source(&manifest, &url, &checksum)?;
    fs::write(staging.join("Package.swift"), &manifest)?;
    let git_revision = git(root, &["rev-parse", "HEAD"])?;
    let git_dirty = !git(root, &["status", "--porcelain"])?.is_empty();
    let timestamp = capture(Command::new("date").args(["-u", "+%Y-%m-%dT%H:%M:%SZ"]))?;
    let archive_size = fs::metadata(&archive)?.len();
    let generated_source_hashes = tree_hashes(&generated_sources)?
        .into_iter()
        .map(|(path, hash)| (format!("Sources/{DIST_NAME}/{path}"), hash))
        .collect();
    let distribution = Distribution {
        version: core_version.clone(),
        core_version,
        adapter_version,
        swift_version: swift,
        git_revision,
        git_dirty,
        timestamp,
        asset,
        url,
        checksum,
        archive_sha256,
        archive_size,
        source_inputs_sha256: sha(&generated.join("SourceInputs.sha256"))?,
        artifacts_sha256: sha(&generated.join("Artifacts.sha256"))?,
        generated_sources: generated_source_hashes,
    };
    fs::write(
        staging.join("README.md"),
        distribution_readme(&distribution),
    )?;
    fs::copy(root.join("LICENSE"), staging.join("LICENSE"))?;
    let work = base.join("work/dist");
    fs::create_dir_all(&work)?;
    let dump = work.join(format!("dump-package-{}.json", std::process::id()));
    validate_staged_manifest(&staging, &dump, &distribution.url, &distribution.checksum)?;
    let entries = capture(
        Command::new("unzip")
            .args(["-Z1"])
            .arg(&staging.join(&distribution.asset)),
    )?
    .lines()
    .map(str::to_owned)
    .collect::<Vec<_>>();
    validate_archive_entries(&entries)?;
    let extracted = work.join(format!("extracted-{}", std::process::id()));
    if extracted.exists() {
        fs::remove_dir_all(&extracted)?;
    }
    extract_zip(&staging.join(&distribution.asset), &extracted)?;
    let extracted_framework = extracted.join(format!("{DIST_BINARY}.xcframework"));
    inspect_framework(&extracted_framework)?;
    compare_trees(&framework, &extracted_framework)?;
    if fs::read_to_string(&checksum_file)?.trim() != distribution.checksum {
        return Err(fail(
            "standalone checksum file does not match the manifest checksum",
        ));
    }
    let _qualified = qualify_distribution(root, &staging, &staging.join(&distribution.asset))?;
    if fs::read_to_string(staging.join("Package.swift"))? != manifest {
        return Err(fail(
            "local qualification modified the staged remote manifest",
        ));
    }
    if source_inputs_current != source_inputs(root)? {
        return Err(fail(
            "Rust/UniFFI inputs changed during packaging; staged distribution was not promoted",
        ));
    }
    fs::write(
        staging.join("distribution.json"),
        distribution_metadata(&distribution),
    )?;
    validate_distribution_metadata(&staging.join("distribution.json"), &distribution)?;
    validate_staging_layout(&staging, &distribution.asset)?;
    let _ = fs::remove_dir_all(&extracted);
    let _ = fs::remove_dir_all(dump.with_extension("scratch"));
    let _ = fs::remove_file(&dump);
    let published = dist_root.join(&release);
    let previous = dist_root.join(format!("{release}.previous-{}", std::process::id()));
    if published.exists() {
        let recognized = fs::read_to_string(published.join("distribution.json"))
            .map(|metadata| {
                metadata.contains(&format!("\"profile\": \"{DIST_PROFILE}\""))
                    && metadata.contains(&format!("\"version\": \"{}\"", distribution.version))
            })
            .unwrap_or(false);
        if !recognized {
            return Err(fail(format!(
                "refusing to replace unrecognized distribution directory {}",
                published.display()
            )));
        }
        fs::rename(&published, &previous)?;
    }
    if let Err(error) = fs::rename(&staging, &published) {
        if previous.exists() {
            fs::rename(&previous, &published)?;
        }
        return Err(error.into());
    }
    if previous.exists() {
        fs::remove_dir_all(previous)?;
    }
    println!(
        "PASS: unpublished Fuse distribution staged at {} (asset {})",
        published.display(),
        distribution.asset
    );
    Ok(())
}
fn main() {
    let result = (|| -> Result<()> {
        let args: Vec<_> = env::args_os().collect();
        if args.len() < 4 {
            return Err(fail(
                "usage: apple <repository> <build|consumer|inspect|dist> <core|flow|fuse> [external-directory]",
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
            "dist" => dist(&root, profile),
            _ => Err(fail("unknown action")),
        }
    })();
    if let Err(error) = result {
        eprintln!("ERROR: {error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checksum() -> String {
        "0123456789abcdef".repeat(4)
    }

    fn url() -> String {
        format!(
            "https://github.com/{DIST_REPOSITORY}/releases/download/0.7.2/{DIST_BINARY}-{DIST_PROFILE}-0.7.2.xcframework.zip"
        )
    }

    #[test]
    fn archive_entries_require_one_xcframework_root() {
        let root = format!("{DIST_BINARY}.xcframework");
        let mut entries = required_archive_entries();
        entries.push(format!("{root}/"));
        entries.push(format!("{root}/macos-arm64/"));
        validate_archive_entries(&entries).unwrap();

        let mut generated = entries.clone();
        generated.push("Generated/Sources/Cribra/Cribra.swift".to_owned());
        assert!(validate_archive_entries(&generated).is_err());

        let mut traversal = entries.clone();
        traversal.push(format!("{root}/../Core.swift"));
        assert!(validate_archive_entries(&traversal).is_err());

        let mut resource_fork = entries;
        resource_fork.push(format!("{root}/__MACOSX/CribraFFI.h"));
        assert!(validate_archive_entries(&resource_fork).is_err());
    }

    #[test]
    fn archive_entries_require_every_slice_and_header() {
        let root = format!("{DIST_BINARY}.xcframework");
        let mut entries = required_archive_entries();
        entries.push(format!("{root}/"));
        entries.retain(|entry| !entry.ends_with("ios-arm64/Headers/module.modulemap"));
        assert!(validate_archive_entries(&entries).is_err());
    }

    #[test]
    fn manifest_is_remote_and_unpublished() {
        let manifest = distribution_manifest("0.7.2", &url(), &checksum());
        validate_manifest_source(&manifest, &url(), &checksum()).unwrap();

        let local = manifest.replace(
            &format!("url: \"{}\"", url()),
            &format!("path: \"{DIST_BINARY}.xcframework\""),
        );
        assert!(validate_manifest_source(&local, &url(), &checksum()).is_err());
    }

    #[test]
    fn manifest_rejects_extra_products_and_dependencies() {
        let manifest = distribution_manifest("0.7.2", &url(), &checksum());
        let extra = manifest.replace(
            "products: [.library(name: \"Cribra\", targets: [\"Cribra\"])],",
            "products: [.library(name: \"Cribra\", targets: [\"Cribra\"]), .library(name: \"Extra\", targets: [\"Cribra\"])],",
        );
        assert!(validate_manifest_source(&extra, &url(), &checksum()).is_err());
        let dependency = manifest.replace(
            "let package = Package(",
            "let package = Package(\n    dependencies: [.package(url: \"https://example.invalid\", from: \"1.0.0\")],",
        );
        assert!(validate_manifest_source(&dependency, &url(), &checksum()).is_err());
    }

    #[test]
    fn json_strings_escape_control_characters() {
        assert_eq!(json_string("a\"b\\c\n"), "\"a\\\"b\\\\c\\n\"");
    }
}
