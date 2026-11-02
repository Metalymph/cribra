use crate::{
    BackendKind, CapabilityManifest, ProfileKind, capability_manifest, wasm_adapter_version,
};

#[test]
fn adapter_version_comes_from_the_adapter_package() {
    let version = wasm_adapter_version();

    assert!(!version.is_empty());
    assert_eq!(version, env!("CARGO_PKG_VERSION"));
}

#[test]
fn manifest_identifies_the_wasm_backend() {
    assert_eq!(capability_manifest().backend(), BackendKind::Wasm);
    assert_eq!(BackendKind::Wasm as u32, 0);
}

#[test]
fn manifest_profile_matches_the_compiled_feature() {
    let manifest = capability_manifest();

    #[cfg(feature = "transform")]
    assert_eq!(manifest.profile(), ProfileKind::Flow);
    #[cfg(not(feature = "transform"))]
    assert_eq!(manifest.profile(), ProfileKind::Core);

    assert_eq!(ProfileKind::Core as u32, 0);
    assert_eq!(ProfileKind::Flow as u32, 1);
}

#[test]
fn transformation_flags_follow_the_transform_feature() {
    let transform = cfg!(feature = "transform");
    let manifest = capability_manifest();

    assert_eq!(manifest.redaction(), transform);
    assert_eq!(manifest.templating(), transform);
    assert_eq!(manifest.pseudonymization(), transform);
    assert_eq!(manifest.synthesis(), transform);
    assert_eq!(manifest.share_bundle(), transform);
}

#[test]
fn always_exported_operations_are_reported() {
    let manifest = capability_manifest();

    assert!(manifest.single_source_scan());
    assert!(manifest.batch_scan());
    assert!(manifest.streaming());
    assert!(manifest.incremental());
    assert!(manifest.rule_metadata());
    assert!(manifest.explanations());
}

#[test]
fn parallel_execution_is_not_exported() {
    assert!(!capability_manifest().parallel());
}

#[test]
fn manifest_is_copyable_metadata() {
    let first = CapabilityManifest::current();
    let second = first;

    assert_eq!(first, second);
}
