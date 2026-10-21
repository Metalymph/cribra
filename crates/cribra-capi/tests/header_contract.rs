//! Historical public declarations pin symbols, numeric codes and field layouts.
#[test]
fn additive_header_preserves_the_existing_abi_declarations() {
    let header = include_str!("../../../include/cribra.h");
    let mut declarations = String::new();
    let mut rest = header;
    while let Some(start) = rest.find("/*") {
        declarations.push_str(&rest[..start]);
        let end = rest[start + 2..].find("*/").unwrap() + start + 4;
        rest = &rest[end..];
    }
    declarations.push_str(rest);
    let normalized: String = declarations
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    let baseline = include_str!("fixtures/abi-0.3.txt");
    for declaration in baseline.lines() {
        assert!(
            normalized.contains(declaration),
            "existing ABI declaration changed: {declaration}"
        );
    }
    assert_eq!(baseline.lines().count(), 172);
    let additions = include_str!("fixtures/metadata-contract.txt");
    for declaration in additions.lines() {
        assert!(
            normalized.contains(declaration),
            "metadata/build-error ABI changed: {declaration}"
        );
    }
    assert_eq!(additions.lines().count(), 16);
}
