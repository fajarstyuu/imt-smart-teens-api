//! Typed settings reject unsafe configuration without exposing secret values.
use imt_smart_teens_backend::config::Settings;
fn settings(extra: &[(&str, &str)]) -> anyhow::Result<Settings> {
    Settings::from_lookup(|key| {
        extra
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| v.to_string())
            .or_else(|| match key {
                "DATABASE_URL" => Some("postgres://synthetic@127.0.0.1:1/db".into()),
                "CSRF_SIGNING_KEY" => Some("test-only-key-with-at-least-thirty-two-bytes".into()),
                _ => None,
            })
    })
}
#[test]
fn positive_bounded_config_and_production_security_are_enforced() {
    for pair in [
        ("DATABASE_MAX_CONNECTIONS", "0"),
        ("REQUEST_TIMEOUT_SECS", "0"),
        ("HTTP_PORT", "65536"),
        ("DATABASE_ACQUIRE_TIMEOUT_SECS", "-1"),
        ("SESSION_IDLE_TTL_SECS", "999999"),
        ("APP_ENV", "unknown"),
        ("SESSION_COOKIE_SECURE", "yes"),
        ("CSRF_SIGNING_KEY", "short"),
        ("CORS_ALLOWED_ORIGINS", "*"),
        ("CORS_ALLOWED_ORIGINS", "https://app.invalid/path"),
    ] {
        assert!(settings(&[pair]).is_err(), "{} should be rejected", pair.0);
    }
    assert!(settings(&[("APP_ENV", "production")]).is_err());
    assert!(
        settings(&[
            ("APP_ENV", "production"),
            ("SESSION_COOKIE_SECURE", "true"),
            ("CORS_ALLOWED_ORIGINS", "https://app.invalid")
        ])
        .is_ok()
    );
    assert!(settings(&[("SESSION_ABSOLUTE_TTL_SECS", "60")]).is_err());
    assert_eq!(settings(&[]).unwrap().database.max_connections, 5);
}
