//! Proxy identity is accepted only across an explicitly trusted socket boundary.
mod common;
use axum::{body::Body, extract::ConnectInfo, http::Request};
use imt_smart_teens_backend::{
    bootstrap::assemble, config::Settings, transport::http::build_router,
};
use sqlx::PgPool;
use std::net::SocketAddr;
use tower::ServiceExt;

fn configured(trusted: &str) -> Result<Settings, anyhow::Error> {
    Settings::from_lookup(|key| match key {
        "DATABASE_URL" => Some("postgres://unused@127.0.0.1:1/unused".into()),
        "CSRF_SIGNING_KEY" => Some("synthetic-proxy-test-key-not-a-live-secret-000000".into()),
        "CORS_ALLOWED_ORIGINS" => Some("http://localhost:5173".into()),
        "TRUSTED_PROXY_IPS" => Some(trusted.into()),
        _ => None,
    })
}

#[test]
fn proxy_allowlist_rejects_wildcards_cidr_and_invalid_addresses() {
    for invalid in ["*", "172.30.251.0/28", "not-an-ip", "172.30.251.2,garbage"] {
        assert!(
            configured(invalid).is_err(),
            "invalid trusted proxy accepted"
        );
    }
    assert!(configured("").is_ok());
    assert!(configured("172.30.251.2, 2001:db8::2").is_ok());
}

async fn login_status(app: &axum::Router, peer: &str, headers: &[(&str, &str)]) -> u16 {
    let mut request = Request::builder()
        .method("POST")
        .uri("/api/v1/admin/sessions")
        .header("origin", "http://localhost:5173")
        .header("content-type", "application/json");
    for (key, value) in headers {
        request = request.header(*key, *value);
    }
    let mut request = request.body(Body::from("{}")).unwrap();
    request
        .extensions_mut()
        .insert(ConnectInfo(peer.parse::<SocketAddr>().unwrap()));
    app.clone()
        .oneshot(request)
        .await
        .unwrap()
        .status()
        .as_u16()
}

#[sqlx::test]
async fn trusted_proxy_separates_visitors_without_weakening_login_budget(pool: PgPool) {
    let app = build_router(
        assemble(pool, configured("172.30.251.2").unwrap())
            .await
            .unwrap(),
    );
    for _ in 0..10 {
        assert_eq!(
            login_status(&app, "172.30.251.2:1234", &[("x-real-ip", "198.51.100.10")]).await,
            403
        );
    }
    assert_eq!(
        login_status(&app, "172.30.251.2:1234", &[("x-real-ip", "198.51.100.10")]).await,
        429
    );
    assert_eq!(
        login_status(&app, "172.30.251.2:1234", &[("x-real-ip", "2001:db8::10")]).await,
        403
    );
}

#[sqlx::test]
async fn untrusted_and_default_peers_cannot_evade_limits_with_forged_headers(pool: PgPool) {
    for trusted in ["", "172.30.251.2"] {
        let app = build_router(
            assemble(pool.clone(), configured(trusted).unwrap())
                .await
                .unwrap(),
        );
        for n in 0..10 {
            let address = format!("198.51.100.{}", n + 1);
            assert_eq!(
                login_status(
                    &app,
                    "192.0.2.15:1234",
                    &[("x-real-ip", &address), ("x-forwarded-for", &address)]
                )
                .await,
                403
            );
        }
        assert_eq!(
            login_status(
                &app,
                "192.0.2.15:1234",
                &[
                    ("x-real-ip", "203.0.113.1"),
                    ("forwarded", "for=203.0.113.2")
                ]
            )
            .await,
            429
        );
    }
}

#[sqlx::test]
async fn trusted_proxy_malformed_duplicate_or_missing_ip_uses_peer_bucket(pool: PgPool) {
    let app = build_router(
        assemble(pool, configured("172.30.251.2").unwrap())
            .await
            .unwrap(),
    );
    for _ in 0..10 {
        assert_eq!(
            login_status(
                &app,
                "172.30.251.2:1234",
                &[("x-real-ip", "198.51.100.1, 198.51.100.2")]
            )
            .await,
            403
        );
    }
    assert_eq!(
        login_status(
            &app,
            "172.30.251.2:1234",
            &[("x-real-ip", "198.51.100.1"), ("x-real-ip", "198.51.100.2")]
        )
        .await,
        429
    );
    assert_eq!(
        login_status(
            &app,
            "172.30.251.2:1234",
            &[("x-forwarded-for", "198.51.100.3")]
        )
        .await,
        429
    );
    assert_eq!(
        login_status(&app, "172.30.251.2:1234", &[("x-real-ip", "bad")]).await,
        429
    );
    assert_eq!(
        login_status(&app, "172.30.251.2:1234", &[("x-real-ip", "198.51.100.4")]).await,
        403
    );
}
