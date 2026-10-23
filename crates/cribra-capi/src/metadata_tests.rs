//! Core parity, borrowed lifetimes and structured construction failures.
use crate::*;
use std::{ptr, slice, str};

fn view(s: &str) -> CribraStringView {
    CribraStringView {
        ptr: s.as_ptr(),
        len: s.len(),
    }
}
unsafe fn text<'a>(v: CribraStringView) -> &'a str {
    if v.len == 0 {
        return "";
    }
    // SAFETY: tests only read views while their documented owning handle is alive.
    str::from_utf8(unsafe { slice::from_raw_parts(v.ptr, v.len) }).unwrap()
}
fn config(kind: u32, id: &str, value: &str, remediation: u32) -> CribraRuleConfig {
    CribraRuleConfig {
        kind,
        id: view(id),
        value: view(value),
        severity: CRIBRA_SEVERITY_HIGH,
        remediation,
    }
}

unsafe fn compare(scanner: *const CribraScanner, core: &cribra::Scanner) {
    let mut count = usize::MAX;
    assert_eq!(
        unsafe { cribra_scanner_rule_count(scanner, &mut count) },
        CRIBRA_OK
    );
    assert_eq!(count, core.rule_metadata().len());
    for (i, expected) in core.rule_metadata().enumerate() {
        let mut actual = CribraRuleMetadataView::default();
        assert_eq!(
            unsafe { cribra_scanner_rule_at(scanner, i, &mut actual) },
            CRIBRA_OK
        );
        assert_eq!(unsafe { text(actual.rule_id) }, expected.id());
        assert_eq!(
            actual.kind,
            match expected.kind() {
                cribra::RuleKind::Literal => 0,
                cribra::RuleKind::Prefix => 1,
                cribra::RuleKind::Suffix => 2,
                cribra::RuleKind::Pattern => 3,
            }
        );
        assert_eq!(
            actual.detection_mode,
            match expected.detection_mode() {
                cribra::DetectionMode::MatcherOnly => 1,
                cribra::DetectionMode::Deterministic => 2,
                cribra::DetectionMode::Contextual => 3,
            }
        );
        assert_eq!(
            actual.severity,
            match expected.severity() {
                cribra::Severity::Info => 0,
                cribra::Severity::Low => 1,
                cribra::Severity::Medium => 2,
                cribra::Severity::High => 3,
                cribra::Severity::Critical => 4,
            }
        );
        assert_eq!(
            actual.remediation,
            match expected.remediation() {
                None => 0,
                Some(cribra::Remediation::RevokeAndRotateCredential) => 1,
                Some(cribra::Remediation::RotateCredential) => 2,
                Some(cribra::Remediation::RotatePassword) => 3,
                Some(cribra::Remediation::ReplacePrivateKey) => 4,
                Some(cribra::Remediation::RemoveSensitiveValue) => 5,
                Some(cribra::Remediation::ReviewSensitiveHash) => 6,
                Some(cribra::Remediation::ReviewPasswordVerifier) => 7,
                Some(_) => u32::MAX,
            }
        );
    }
    let mut invalid = CribraRuleMetadataView {
        rule_id: view("old"),
        ..Default::default()
    };
    for index in [count, usize::MAX] {
        assert_eq!(
            unsafe { cribra_scanner_rule_at(scanner, index, &mut invalid) },
            CRIBRA_OUT_OF_RANGE
        );
        assert!(invalid.rule_id.ptr.is_null());
        assert_eq!(
            (
                invalid.kind,
                invalid.detection_mode,
                invalid.severity,
                invalid.remediation
            ),
            (0, 0, 0, 0)
        );
    }
}

#[test]
fn metadata_matches_core_for_all_catalog_selections_and_custom_families() {
    unsafe {
        for mask in 0..8 {
            for custom in [false, true] {
                let mut native = ptr::null_mut();
                let mut scanner = ptr::null_mut();
                assert_eq!(cribra_builder_new(&mut native), CRIBRA_OK);
                let mut core = cribra::Scanner::builder();
                if mask & 1 != 0 {
                    assert_eq!(cribra_builder_add_current_builtins(native), CRIBRA_OK);
                    core = core.builtins(cribra::builtins::CURRENT);
                }
                if mask & 2 != 0 {
                    assert_eq!(cribra_builder_add_financial_builtins(native), CRIBRA_OK);
                    core = core.builtins(cribra::builtins::financial::CURRENT);
                }
                if mask & 4 != 0 {
                    assert_eq!(cribra_builder_add_personal_builtins(native), CRIBRA_OK);
                    core = core.builtins(cribra::builtins::personal::CURRENT);
                }
                if custom {
                    for (kind, value) in
                        [(0, "SECRET"), (1, "pre_"), (2, "_END"), (3, "code[0-9]{2}")]
                    {
                        let id = format!("custom/clé-🔑/{kind}");
                        let cfg = config(kind, &id, value, if kind == 0 { 7 } else { 0 });
                        assert_eq!(
                            cribra_builder_add_rule(native, &cfg, ptr::null_mut()),
                            CRIBRA_OK
                        );
                        let rule = match kind {
                            0 => cribra::Rule::literal(id.as_str(), value, cribra::Severity::High)
                                .with_remediation(cribra::Remediation::ReviewPasswordVerifier),
                            1 => cribra::Rule::prefix(id.as_str(), value, cribra::Severity::High),
                            2 => cribra::Rule::suffix(id.as_str(), value, cribra::Severity::High),
                            _ => cribra::Rule::pattern(id.as_str(), value, cribra::Severity::High)
                                .unwrap(),
                        };
                        core = core.rule(rule);
                    }
                }
                assert_eq!(
                    cribra_builder_build(native, &mut scanner, ptr::null_mut()),
                    CRIBRA_OK
                );
                compare(scanner, &core.build().unwrap());
                cribra_scanner_free(scanner);
            }
        }
        let mut scanner = ptr::null_mut();
        assert_eq!(cribra_scanner_new_current(&mut scanner), CRIBRA_OK);
        compare(scanner, &cribra::Scanner::default());
        cribra_scanner_free(scanner);
    }
}

#[test]
fn metadata_borrows_scanner_storage_across_scans_and_report_frees() {
    unsafe {
        let mut builder = ptr::null_mut();
        let mut scanner = ptr::null_mut();
        cribra_builder_new(&mut builder);
        {
            let id = String::from("custom/clé-🔑");
            let value = String::from("SECRET");
            let cfg = config(0, &id, &value, 0);
            assert_eq!(
                cribra_builder_add_rule(builder, &cfg, ptr::null_mut()),
                CRIBRA_OK
            );
        }
        assert_eq!(
            cribra_builder_build(builder, &mut scanner, ptr::null_mut()),
            CRIBRA_OK
        );
        let mut metadata = CribraRuleMetadataView::default();
        cribra_scanner_rule_at(scanner, 0, &mut metadata);
        let mut report = ptr::null_mut();
        assert_eq!(
            cribra_scanner_scan(scanner, b"SECRET".as_ptr(), 6, &mut report, ptr::null_mut()),
            CRIBRA_OK
        );
        cribra_report_free(report);
        assert_eq!(text(metadata.rule_id), "custom/clé-🔑");
        let mut again = CribraRuleMetadataView::default();
        cribra_scanner_rule_at(scanner, 0, &mut again);
        assert_eq!(metadata.rule_id.ptr, again.rule_id.ptr);
        let copy = text(metadata.rule_id).to_owned();
        cribra_scanner_free(scanner);
        assert_eq!(copy, "custom/clé-🔑"); // Only the explicit copy may outlive scanner free.
    }
}

#[test]
fn metadata_and_error_accessors_validate_nulls_and_clear_output() {
    unsafe {
        let mut count = 7;
        assert_eq!(
            cribra_scanner_rule_count(ptr::null(), &mut count),
            CRIBRA_INVALID_ARGUMENT
        );
        assert_eq!(count, 0);
        assert_eq!(
            cribra_scanner_rule_count(ptr::null(), ptr::null_mut()),
            CRIBRA_INVALID_ARGUMENT
        );
        let mut rule = CribraRuleMetadataView {
            rule_id: view("old"),
            ..Default::default()
        };
        assert_eq!(
            cribra_scanner_rule_at(ptr::null(), 0, &mut rule),
            CRIBRA_INVALID_ARGUMENT
        );
        assert!(rule.rule_id.ptr.is_null());
        assert_eq!(
            cribra_scanner_rule_at(ptr::null(), 0, ptr::null_mut()),
            CRIBRA_INVALID_ARGUMENT
        );
        let mut info = CribraBuildErrorView {
            code: 7,
            rule_id: view("old"),
        };
        assert_eq!(
            cribra_error_build_info(ptr::null(), &mut info),
            CRIBRA_INVALID_ARGUMENT
        );
        assert_eq!(info.code, 0);
        assert!(info.rule_id.ptr.is_null());
        assert_eq!(
            cribra_error_build_info(ptr::null(), ptr::null_mut()),
            CRIBRA_INVALID_ARGUMENT
        );
        let error = Box::into_raw(Box::new(CribraError::from_status(CRIBRA_INVALID_UTF8)));
        assert_eq!(cribra_error_build_info(error, &mut info), CRIBRA_OK);
        assert_eq!(info.code, CRIBRA_BUILD_CODE_NONE);
        assert!(info.rule_id.ptr.is_null());
        assert_eq!(
            cribra_error_build_info(error, ptr::null_mut()),
            CRIBRA_INVALID_ARGUMENT
        );
        cribra_error_free(error);
    }
}

#[test]
fn pattern_errors_keep_status_message_and_borrow_an_owned_constructor_id() {
    unsafe {
        for (pattern, code) in [
            ("[", CRIBRA_BUILD_CODE_INVALID_PATTERN),
            ("a*", CRIBRA_BUILD_CODE_PATTERN_MATCHES_EMPTY),
            (r"\b", CRIBRA_BUILD_CODE_PATTERN_MATCHES_EMPTY),
        ] {
            let mut builder = ptr::null_mut();
            let mut error = ptr::null_mut();
            cribra_builder_new(&mut builder);
            let id = String::from("custom/clé-🔑");
            assert_eq!(
                cribra_builder_add_rule(builder, &config(3, &id, pattern, 0), &mut error),
                CRIBRA_RULE_ERROR
            );
            drop(id);
            cribra_builder_free(builder);
            let mut info = CribraBuildErrorView::default();
            assert_eq!(cribra_error_build_info(error, &mut info), CRIBRA_OK);
            assert_eq!(info.code, code);
            assert_eq!(text(info.rule_id), "custom/clé-🔑");
            let mut message = CribraStringView::default();
            cribra_error_message(error, &mut message);
            assert_eq!(text(message), "custom rule configuration is invalid");
            cribra_error_free(error);
        }
    }
}

#[test]
fn structured_build_failures_preserve_core_precedence_and_error_ownership() {
    unsafe {
        for (id, value, duplicate, expected) in [
            ("", "SECRET", false, 4),
            ("empty", "", false, 6),
            ("same", "", true, 5),
        ] {
            for kind in 0..3 {
                let mut builder = ptr::null_mut();
                let mut scanner = ptr::dangling_mut();
                let mut error = ptr::null_mut();
                cribra_builder_new(&mut builder);
                assert_eq!(
                    cribra_builder_add_rule(builder, &config(kind, id, value, 0), ptr::null_mut()),
                    CRIBRA_OK
                );
                if duplicate {
                    assert_eq!(
                        cribra_builder_add_rule(
                            builder,
                            &config(0, id, "OTHER", 0),
                            ptr::null_mut()
                        ),
                        CRIBRA_OK
                    );
                }
                assert_eq!(
                    cribra_builder_build(builder, &mut scanner, &mut error),
                    CRIBRA_BUILD_ERROR
                );
                assert!(scanner.is_null());
                let mut info = CribraBuildErrorView::default();
                cribra_error_build_info(error, &mut info);
                assert_eq!(info.code, expected);
                if expected == 4 {
                    assert!(info.rule_id.ptr.is_null());
                } else {
                    assert_eq!(text(info.rule_id), id);
                }
                cribra_error_free(error);
            }
        }
        for (catalog, id) in [
            (0, "github.classic-pat"),
            (1, "financial.iban"),
            (2, "personal.us-ssn"),
        ] {
            let mut builder = ptr::null_mut();
            let mut scanner = ptr::null_mut();
            let mut error = ptr::null_mut();
            cribra_builder_new(&mut builder);
            match catalog {
                0 => cribra_builder_add_current_builtins(builder),
                1 => cribra_builder_add_financial_builtins(builder),
                _ => cribra_builder_add_personal_builtins(builder),
            };
            cribra_builder_add_rule(builder, &config(0, id, "SECRET", 0), ptr::null_mut());
            assert_eq!(
                cribra_builder_build(builder, &mut scanner, &mut error),
                CRIBRA_BUILD_ERROR
            );
            let mut info = CribraBuildErrorView::default();
            cribra_error_build_info(error, &mut info);
            assert_eq!(info.code, CRIBRA_BUILD_CODE_DUPLICATE_RULE_ID);
            assert_eq!(text(info.rule_id), id);
            let mut message = CribraStringView::default();
            cribra_error_message(error, &mut message);
            assert_eq!(text(message), "duplicate rule identifier");
            cribra_error_free(error);
        }
    }
}

#[test]
fn rare_build_codes_are_conversion_checks_not_forced_core_failures() {
    for (error, code) in [
        (
            cribra::ScannerBuildError::TooManyRules,
            CRIBRA_BUILD_CODE_TOO_MANY_RULES,
        ),
        (
            cribra::ScannerBuildError::Rule(cribra::RuleError::MissingCaptureGroup {
                name: "absent".into(),
            }),
            CRIBRA_BUILD_CODE_MISSING_CAPTURE_GROUP,
        ),
    ] {
        let error = CribraError::from_status(CRIBRA_BUILD_ERROR).with_build_error(&error);
        let mut info = CribraBuildErrorView::default();
        assert_eq!(
            unsafe { cribra_error_build_info(&error, &mut info) },
            CRIBRA_OK
        );
        assert_eq!(info.code, code);
        assert!(info.rule_id.ptr.is_null());
    }
}
