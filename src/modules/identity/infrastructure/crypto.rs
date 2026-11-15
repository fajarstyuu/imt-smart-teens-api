//! Password work is memory-bounded and moved off Tokio worker threads.
use crate::{
    modules::identity::application::{PasswordPort, TokenPort},
    shared::application::{AppError, AppResult},
};
use argon2::{
    Algorithm, Argon2, Params, PasswordHash, PasswordHasher, PasswordVerifier, Version,
    password_hash::SaltString,
};
use async_trait::async_trait;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use rand::{RngCore, rngs::OsRng};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use tokio::sync::Semaphore;

pub struct ArgonPasswords {
    slots: Arc<Semaphore>,
}
impl ArgonPasswords {
    pub fn new(max_parallel: usize) -> Self {
        Self {
            slots: Arc::new(Semaphore::new(max_parallel.max(1))),
        }
    }
}
fn algorithm() -> AppResult<Argon2<'static>> {
    // OWASP Argon2id baseline: 19 MiB, two passes, one lane. Parameters live in each PHC hash.
    let params = Params::new(19456, 2, 1, None).map_err(|_| AppError::Internal)?;
    Ok(Argon2::new(Algorithm::Argon2id, Version::V0x13, params))
}
#[async_trait]
impl PasswordPort for ArgonPasswords {
    async fn hash(&self, password: String) -> AppResult<String> {
        let permit = self
            .slots
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| AppError::Unavailable)?;
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let salt = SaltString::generate(&mut OsRng);
            algorithm()?
                .hash_password(password.as_bytes(), &salt)
                .map(|h| h.to_string())
                .map_err(|_| AppError::Internal)
        })
        .await
        .map_err(|_| AppError::Internal)?
    }
    async fn verify(&self, password: String, encoded: String) -> AppResult<bool> {
        let permit = self
            .slots
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| AppError::Unavailable)?;
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let hash = PasswordHash::new(&encoded).map_err(|_| AppError::Internal)?;
            if hash.algorithm.as_str() != "argon2id" {
                return Err(AppError::Internal);
            }
            match algorithm()?.verify_password(password.as_bytes(), &hash) {
                Ok(()) => Ok(true),
                Err(argon2::password_hash::Error::Password) => Ok(false),
                Err(_) => Err(AppError::Internal),
            }
        })
        .await
        .map_err(|_| AppError::Internal)?
    }
}
pub struct SecureTokens;
impl TokenPort for SecureTokens {
    fn generate(&self) -> AppResult<String> {
        let mut bytes = [0_u8; 32];
        OsRng
            .try_fill_bytes(&mut bytes)
            .map_err(|_| AppError::Internal)?;
        Ok(URL_SAFE_NO_PAD.encode(bytes))
    }
    fn digest(&self, value: &str) -> Vec<u8> {
        Sha256::digest(value.as_bytes()).to_vec()
    }
}
