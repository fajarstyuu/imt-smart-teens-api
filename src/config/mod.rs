//! Typed, centrally validated configuration. Secret-bearing types deliberately omit Debug.
use anyhow::{Result, bail};
use std::{
    net::{IpAddr, SocketAddr},
    time::Duration,
};

#[derive(Clone)]
pub struct Settings {
    pub environment: String,
    pub bind: SocketAddr,
    pub database: DatabaseSettings,
    pub http: HttpSettings,
    pub session: SessionSettings,
    pub csrf_key: Vec<u8>,
    pub shutdown_timeout: Duration,
}
#[derive(Clone)]
pub struct DatabaseSettings {
    pub url: String,
    pub max_connections: u32,
    pub acquire_timeout: Duration,
    pub statement_timeout: Duration,
}
#[derive(Clone)]
pub struct HttpSettings {
    pub allowed_origins: Vec<String>,
    pub trusted_proxy_ips: Vec<IpAddr>,
    pub request_timeout: Duration,
    pub body_limit: usize,
}
#[derive(Clone)]
pub struct SessionSettings {
    pub secure: bool,
    pub idle_seconds: i64,
    pub student_absolute_seconds: i64,
    pub admin_absolute_seconds: i64,
}
impl Settings {
    pub fn load() -> Result<Self> {
        Self::from_lookup(|key| std::env::var(key).ok())
    }
    /// Injectable lookup keeps tests deterministic without process-global environment mutation.
    pub fn from_lookup(mut get: impl FnMut(&str) -> Option<String>) -> Result<Self> {
        let environment = get("APP_ENV").unwrap_or_else(|| "development".into());
        if !["development", "test", "production"].contains(&environment.as_str()) {
            bail!("APP_ENV is invalid");
        }
        let host = get("HTTP_HOST")
            .unwrap_or_else(|| "127.0.0.1".into())
            .parse::<IpAddr>()
            .map_err(|_| anyhow::anyhow!("HTTP_HOST is invalid"))?;
        let port = number(&mut get, "HTTP_PORT", 4180, 1, 65535)? as u16;
        let url = get("DATABASE_URL")
            .filter(|s| s.starts_with("postgres://") || s.starts_with("postgresql://"))
            .ok_or_else(|| anyhow::anyhow!("DATABASE_URL is required and must be PostgreSQL"))?;
        let max_connections = number(&mut get, "DATABASE_MAX_CONNECTIONS", 5, 1, 50)? as u32;
        let acquire_timeout =
            Duration::from_secs(number(&mut get, "DATABASE_ACQUIRE_TIMEOUT_SECS", 3, 1, 30)?);
        let statement_timeout = Duration::from_secs(number(
            &mut get,
            "DATABASE_STATEMENT_TIMEOUT_SECS",
            10,
            1,
            120,
        )?);
        let request_timeout =
            Duration::from_secs(number(&mut get, "REQUEST_TIMEOUT_SECS", 15, 1, 120)?);
        let body_limit =
            number(&mut get, "REQUEST_BODY_LIMIT_BYTES", 65536, 1024, 1048576)? as usize;
        let secure = match get("SESSION_COOKIE_SECURE").as_deref().unwrap_or("false") {
            "true" => true,
            "false" => false,
            _ => bail!("SESSION_COOKIE_SECURE is invalid"),
        };
        let idle_seconds = number(&mut get, "SESSION_IDLE_TTL_SECS", 7200, 60, 86400)? as i64;
        let student_absolute_seconds =
            number(&mut get, "SESSION_ABSOLUTE_TTL_SECS", 604800, 60, 2592000)? as i64;
        let admin_absolute_seconds = number(
            &mut get,
            "ADMIN_SESSION_ABSOLUTE_TTL_SECS",
            43200,
            60,
            604800,
        )? as i64;
        if idle_seconds > student_absolute_seconds || idle_seconds > admin_absolute_seconds {
            bail!("session idle lifetime exceeds absolute lifetime");
        }
        let allowed_origins = get("CORS_ALLOWED_ORIGINS")
            .unwrap_or_default()
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect::<Vec<_>>();
        for origin in &allowed_origins {
            let uri = origin
                .parse::<axum::http::Uri>()
                .map_err(|_| anyhow::anyhow!("CORS_ALLOWED_ORIGINS is invalid"))?;
            if !matches!(uri.scheme_str(), Some("http" | "https"))
                || uri.authority().is_none()
                || uri.path() != "/"
                || uri.query().is_some()
                || origin.ends_with('/')
                || origin.contains('@')
            {
                bail!("CORS_ALLOWED_ORIGINS must contain exact origins");
            }
        }
        let proxy_ips = get("TRUSTED_PROXY_IPS").unwrap_or_default();
        let trusted_proxy_ips = if proxy_ips.trim().is_empty() {
            Vec::new()
        } else {
            proxy_ips
                .split(',')
                .map(|ip| ip.trim().parse::<IpAddr>())
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| anyhow::anyhow!("TRUSTED_PROXY_IPS requires exact IP addresses"))?
        };
        let csrf_key = get("CSRF_SIGNING_KEY")
            .filter(|s| s.len() >= 32)
            .ok_or_else(|| anyhow::anyhow!("CSRF_SIGNING_KEY must contain at least 32 bytes"))?
            .into_bytes();
        if environment == "production"
            && (!secure || allowed_origins.iter().any(|s| !s.starts_with("https://")))
        {
            bail!("production requires secure cookies and HTTPS origins");
        }
        let shutdown_timeout =
            Duration::from_secs(number(&mut get, "SHUTDOWN_TIMEOUT_SECS", 20, 1, 120)?);
        Ok(Self {
            environment,
            bind: SocketAddr::new(host, port),
            database: DatabaseSettings {
                url,
                max_connections,
                acquire_timeout,
                statement_timeout,
            },
            http: HttpSettings {
                allowed_origins,
                trusted_proxy_ips,
                request_timeout,
                body_limit,
            },
            session: SessionSettings {
                secure,
                idle_seconds,
                student_absolute_seconds,
                admin_absolute_seconds,
            },
            csrf_key,
            shutdown_timeout,
        })
    }
}
fn number(
    get: &mut impl FnMut(&str) -> Option<String>,
    key: &str,
    default: u64,
    min: u64,
    max: u64,
) -> Result<u64> {
    let value = match get(key) {
        Some(s) => s
            .parse::<u64>()
            .map_err(|_| anyhow::anyhow!("{key} must be an integer"))?,
        None => default,
    };
    if !(min..=max).contains(&value) {
        bail!("{key} is outside the supported range");
    }
    Ok(value)
}
