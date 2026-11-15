//! Credential verification and revocable browser sessions.
use crate::{
    modules::identity::domain::{AdminPrincipal, Principal, normalize_email},
    shared::{
        application::{AppError, AppResult},
        clock::Clock,
    },
};
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use std::sync::Arc;
use uuid::Uuid;

pub struct AdminCredentials {
    pub principal: AdminPrincipal,
    pub password_hash: String,
}
#[derive(Clone)]
pub struct Authenticated {
    pub session_id: Uuid,
    pub principal: Principal,
    pub csrf_binding_hash: Vec<u8>,
}
/// Subject ownership is retained in session insertion; student registration must insert
/// this row in the SAME transaction as its enrollment (do not call a second repository).
#[derive(Clone)]
pub enum SessionSubject {
    Admin(Uuid),
    Student {
        student_id: Uuid,
        enrollment_id: Uuid,
    },
}
pub struct NewSession {
    pub id: Uuid,
    pub token_hash: Vec<u8>,
    pub subject: SessionSubject,
    pub csrf_binding_hash: Vec<u8>,
    pub created_at: DateTime<Utc>,
    pub idle_expires_at: DateTime<Utc>,
    pub absolute_expires_at: DateTime<Utc>,
}
pub struct IssuedSession {
    pub token: String,
    pub record: NewSession,
}
#[derive(Clone)]
pub struct SessionPolicy {
    pub idle_seconds: i64,
    pub student_absolute_seconds: i64,
    pub admin_absolute_seconds: i64,
}
#[async_trait]
pub trait PasswordPort: Send + Sync {
    async fn hash(&self, password: String) -> AppResult<String>;
    async fn verify(&self, password: String, encoded: String) -> AppResult<bool>;
}
pub trait TokenPort: Send + Sync {
    fn generate(&self) -> AppResult<String>;
    fn digest(&self, value: &str) -> Vec<u8>;
}
#[async_trait]
pub trait IdentityStore: Send + Sync {
    async fn admin_by_email(&self, email: &str) -> AppResult<Option<AdminCredentials>>;
    /// Insert, revoke the previous browser session, and audit login atomically.
    async fn insert_session(
        &self,
        record: &NewSession,
        old_hash: Option<Vec<u8>>,
        sibling_hash: Option<Vec<u8>>,
        request_id: &str,
    ) -> AppResult<()>;
    async fn authenticate(
        &self,
        hash: &[u8],
        now: DateTime<Utc>,
        idle_seconds: i64,
    ) -> AppResult<Option<Authenticated>>;
    async fn revoke(&self, hash: &[u8], request_id: &str) -> AppResult<()>;
    async fn bind_csrf(&self, session_id: Uuid, binding: &[u8]) -> AppResult<()>;
}
#[derive(Clone)]
pub struct SessionService {
    store: Arc<dyn IdentityStore>,
    tokens: Arc<dyn TokenPort>,
    clock: Arc<dyn Clock>,
    policy: SessionPolicy,
}
impl SessionService {
    pub fn new(
        store: Arc<dyn IdentityStore>,
        tokens: Arc<dyn TokenPort>,
        clock: Arc<dyn Clock>,
        policy: SessionPolicy,
    ) -> Self {
        Self {
            store,
            tokens,
            clock,
            policy,
        }
    }
    /// Produces a session without side effects for insertion by a cross-context unit of work.
    pub fn prepare(
        &self,
        subject: SessionSubject,
        csrf_binding_hash: Vec<u8>,
    ) -> AppResult<IssuedSession> {
        let token = self.tokens.generate()?;
        let now = self.clock.now();
        let absolute = match subject {
            SessionSubject::Admin(_) => self.policy.admin_absolute_seconds,
            SessionSubject::Student { .. } => self.policy.student_absolute_seconds,
        };
        Ok(IssuedSession {
            record: NewSession {
                id: Uuid::new_v4(),
                token_hash: self.tokens.digest(&token),
                subject,
                csrf_binding_hash,
                created_at: now,
                idle_expires_at: now + Duration::seconds(self.policy.idle_seconds.min(absolute)),
                absolute_expires_at: now + Duration::seconds(absolute),
            },
            token,
        })
    }
    pub async fn authenticate(&self, token: &str) -> AppResult<Authenticated> {
        if token.len() != 43 {
            return Err(AppError::Unauthorized);
        }
        self.store
            .authenticate(
                &self.tokens.digest(token),
                self.clock.now(),
                self.policy.idle_seconds,
            )
            .await?
            .ok_or(AppError::Unauthorized)
    }
    pub async fn revoke(&self, token: Option<&str>, request_id: &str) -> AppResult<()> {
        if let Some(token) = token {
            self.store
                .revoke(&self.tokens.digest(token), request_id)
                .await?;
        }
        Ok(())
    }
    pub async fn bind_csrf(&self, id: Uuid, binding: &[u8]) -> AppResult<()> {
        self.store.bind_csrf(id, binding).await
    }
}
/// Browser context for atomic login rotation; raw cookie values never implement Debug.
pub struct AdminLoginContext {
    pub binding: Vec<u8>,
    pub previous_admin_token: Option<String>,
    pub student_token: Option<String>,
    pub request_id: String,
}
#[derive(Clone)]
pub struct AdminIdentity {
    store: Arc<dyn IdentityStore>,
    passwords: Arc<dyn PasswordPort>,
    sessions: SessionService,
    dummy_hash: String,
}
impl AdminIdentity {
    pub fn new(
        store: Arc<dyn IdentityStore>,
        passwords: Arc<dyn PasswordPort>,
        sessions: SessionService,
        dummy_hash: String,
    ) -> Self {
        Self {
            store,
            passwords,
            sessions,
            dummy_hash,
        }
    }
    pub async fn login(
        &self,
        email: &str,
        password: String,
        context: AdminLoginContext,
    ) -> AppResult<(AdminPrincipal, IssuedSession)> {
        let email = normalize_email(email)?;
        if password.is_empty() || password.len() > 1024 {
            return Err(AppError::Invalid("INVALID_CREDENTIALS_INPUT"));
        }
        let admin = self.store.admin_by_email(&email).await?;
        // Unknown accounts incur the same password work; generic errors avoid account enumeration.
        let hash = admin
            .as_ref()
            .map(|a| a.password_hash.clone())
            .unwrap_or_else(|| self.dummy_hash.clone());
        let valid = self.passwords.verify(password, hash).await?;
        let Some(admin) = admin.filter(|_| valid) else {
            return Err(AppError::Unauthorized);
        };
        let issued = self
            .sessions
            .prepare(SessionSubject::Admin(admin.principal.id), context.binding)?;
        let old_hash = context
            .previous_admin_token
            .map(|s| self.sessions.tokens.digest(&s));
        let sibling_hash = context
            .student_token
            .map(|s| self.sessions.tokens.digest(&s));
        self.store
            .insert_session(&issued.record, old_hash, sibling_hash, &context.request_id)
            .await?;
        Ok((admin.principal, issued))
    }
}
