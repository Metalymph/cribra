use silens_scan::project_name;

#[test]
fn public_api_is_available_to_consumers() {
    assert_eq!(project_name(), "Silens Scan");
}
