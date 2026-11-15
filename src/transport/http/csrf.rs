//! Signed, short-lived double-submit contexts, additionally bound to stored sessions.
//! HMAC and constant-time comparison use maintained crates, never a custom cipher.
use crate::{
    modules::identity::{application::TokenPort, infrastructure::SecureTokens},
    shared::application::{AppError, AppResult},
};
use axum::http::{HeaderMap, HeaderValue, header};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
#[derive(Clone)]
pub struct Csrf {
    key: Vec<u8>,
    secure: bool,
    allowed_origins: Vec<String>,
}
impl Csrf {
    pub fn new(key: Vec<u8>, secure: bool, allowed_origins: Vec<String>) -> Self {
        Self {
            key,
            secure,
            allowed_origins,
        }
    }
    pub fn issue(&self) -> AppResult<String> {
        let payload = format!(
            "{}.{}",
            SecureTokens.generate()?,
            chrono::Utc::now().timestamp() + 7200
        );
        let mut mac = Hmac::<Sha256>::new_from_slice(&self.key).map_err(|_| AppError::Internal)?;
        mac.update(payload.as_bytes());
        Ok(format!(
            "{payload}.{}",
            URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes())
        ))
    }
    pub fn valid(&self, token: &str) -> bool {
        if token.len() > 256 {
            return false;
        }
        let Some((payload, signature)) = token.rsplit_once('.') else {
            return false;
        };
        let Some((nonce, expires)) = payload.split_once('.') else {
            return false;
        };
        let Ok(expires) = expires.parse::<i64>() else {
            return false;
        };
        let now = chrono::Utc::now().timestamp();
        if nonce.len() != 43 || expires <= now || expires > now + 7200 {
            return false;
        }
        let Ok(signature) = URL_SAFE_NO_PAD.decode(signature) else {
            return false;
        };
        let Ok(mut mac) = Hmac::<Sha256>::new_from_slice(&self.key) else {
            return false;
        };
        mac.update(payload.as_bytes());
        mac.verify_slice(&signature).is_ok()
    }
    pub fn validate(&self, headers: &HeaderMap) -> AppResult<Vec<u8>> {
        let origin = headers
            .get(header::ORIGIN)
            .and_then(|v| v.to_str().ok())
            .ok_or(AppError::Forbidden)?;
        if !self.allowed_origins.iter().any(|s| s == origin) {
            return Err(AppError::Forbidden);
        }
        let context = cookie(headers, "imt_csrf").ok_or(AppError::Forbidden)?;
        let token = headers
            .get("x-csrf-token")
            .and_then(|v| v.to_str().ok())
            .ok_or(AppError::Forbidden)?;
        if !bool::from(context.as_bytes().ct_eq(token.as_bytes())) || !self.valid(context) {
            return Err(AppError::Forbidden);
        }
        Ok(Self::binding(context))
    }
    pub fn binding(token: &str) -> Vec<u8> {
        Sha256::digest(token.as_bytes()).to_vec()
    }
    pub fn cookie(&self, token: &str) -> AppResult<HeaderValue> {
        cookie_value("imt_csrf", token, 7200, self.secure)
    }
}
/// Reject ambiguous duplicate cookie names instead of choosing an attacker-controlled value.
pub fn cookie<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    let mut found = None;
    for header in headers.get_all(header::COOKIE) {
        for part in header.to_str().ok()?.split(';') {
            if let Some((key, value)) = part.trim().split_once('=')
                && key == name
            {
                if found.is_some() {
                    return None;
                }
                found = Some(value);
            }
        }
    }
    found
}
pub fn cookie_value(name: &str, value: &str, max_age: i64, secure: bool) -> AppResult<HeaderValue> {
    HeaderValue::from_str(&format!(
        "{name}={value}; Path=/api/v1; HttpOnly; SameSite=Lax; Max-Age={max_age}{}",
        if secure { "; Secure" } else { "" }
    ))
    .map_err(|_| AppError::Internal)
}
