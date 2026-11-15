//! Boundary policies reject invalid persisted inputs before I/O.
use imt_smart_teens_backend::modules::identity::domain::{FormDate, SchoolName, StudentName};

#[test]
fn school_normalization_matches_unicode_and_full_casefold() {
    let a = SchoolName::parse("  École\t STRAẞE  ").unwrap();
    let b = SchoolName::parse("E\u{301}cole strasse").unwrap();
    assert_eq!(a.name(), "École STRAẞE");
    assert_eq!(a.normalized(), b.normalized());
    assert_eq!(a.normalized(), "école strasse");
}
#[test]
fn names_reject_empty_or_overlong_but_allow_one_word() {
    for value in [" ".to_owned(), "x".repeat(301)] {
        assert!(SchoolName::parse(&value).is_err());
    }
    assert!(StudentName::parse("Ayu").is_ok());
    assert!(StudentName::parse(&"a".repeat(201)).is_err());
    assert!(StudentName::parse("\n ").is_err());
}
#[test]
fn domain_does_not_depend_on_application_error_contracts() {
    // Source-level boundary guard complements value-level validation tests in
    // this single crate, where Rust otherwise permits upward module imports.
    let domain = include_str!("../src/modules/identity/domain.rs");
    assert!(
        !domain.contains("shared::application"),
        "domain imports application layer"
    );
}

#[test]
fn date_requires_real_iso_calendar_date() {
    assert!(FormDate::parse("2024-02-29").is_ok());
    for value in ["2025-02-29", "2026-2-01", "2026-11-15T00:00:00Z", ""] {
        assert!(FormDate::parse(value).is_err());
    }
}
