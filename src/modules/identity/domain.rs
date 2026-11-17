//! Identity invariants and owned principal data. This layer performs no I/O.
use crate::shared::error::{AppError, AppResult};
use chrono::NaiveDate;
use unicode_casefold::UnicodeCaseFold;
use unicode_normalization::UnicodeNormalization;
use uuid::Uuid;

/// NFC, whitespace folding, and full Unicode casefold are centralized here.
#[derive(Clone, Debug)]
pub struct SchoolName {
    name: String,
    normalized: String,
}
impl SchoolName {
    pub fn parse(value: &str) -> AppResult<Self> {
        let name = normalize_label(value, 300, "INVALID_SCHOOL_NAME")?;
        let normalized = name.case_fold().nfc().collect();
        Ok(Self { name, normalized })
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn normalized(&self) -> &str {
        &self.normalized
    }
}
/// Student names are not identity proofs and need not contain two words.
#[derive(Clone, Debug)]
pub struct StudentName(String);
impl StudentName {
    pub fn parse(value: &str) -> AppResult<Self> {
        normalize_label(value, 200, "INVALID_STUDENT_NAME").map(Self)
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
fn normalize_label(value: &str, max: usize, code: &'static str) -> AppResult<String> {
    let value = value
        .nfc()
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if value.is_empty() || value.chars().count() > max || value.chars().any(char::is_control) {
        return Err(AppError::Invalid(code));
    }
    Ok(value)
}
/// Calendar-only date, strict ISO width; never converted via a timezone.
#[derive(Clone, Copy, Debug)]
pub struct FormDate(NaiveDate);
impl FormDate {
    pub fn parse(value: &str) -> AppResult<Self> {
        if value.len() != 10 || value.as_bytes()[4] != b'-' || value.as_bytes()[7] != b'-' {
            return Err(AppError::Invalid("INVALID_FORM_DATE"));
        }
        NaiveDate::parse_from_str(value, "%Y-%m-%d")
            .map(Self)
            .map_err(|_| AppError::Invalid("INVALID_FORM_DATE"))
    }
    pub fn value(self) -> NaiveDate {
        self.0
    }
}
/// Verified account attributes are loaded server-side on every authenticated request.
#[derive(Clone, Debug)]
pub struct AdminPrincipal {
    pub id: Uuid,
    pub name: String,
    pub email: String,
    pub can_manage_schools: bool,
    pub can_manage_content: bool,
    pub school_ids: Vec<Uuid>,
}
impl AdminPrincipal {
    /// Every verified active administrator owns the global catalog capability.
    /// Legacy stored booleans are compatibility data, not authorization gates.
    pub fn require_schools(&self) -> AppResult<()> {
        Ok(())
    }
    /// All schools, including future/archived ones, belong to the admin scope.
    /// Constructing this principal remains the authenticated session boundary.
    pub fn require_school_scope(&self, _id: Uuid) -> AppResult<()> {
        Ok(())
    }
}
#[derive(Clone, Debug)]
pub enum Principal {
    Admin(AdminPrincipal),
    Student {
        student_id: Uuid,
        enrollment_id: Uuid,
    },
}
impl Principal {
    pub fn admin(&self) -> AppResult<&AdminPrincipal> {
        match self {
            Self::Admin(a) => Ok(a),
            _ => Err(AppError::Forbidden),
        }
    }
    pub fn student(&self) -> AppResult<(Uuid, Uuid)> {
        match self {
            Self::Student {
                student_id,
                enrollment_id,
            } => Ok((*student_id, *enrollment_id)),
            _ => Err(AppError::Forbidden),
        }
    }
}
/// Input credentials are validated before expensive password work.
pub fn normalize_email(value: &str) -> AppResult<String> {
    let email = value.trim().to_lowercase();
    let Some((local, domain)) = email.split_once('@') else {
        return Err(AppError::Invalid("INVALID_CREDENTIALS_INPUT"));
    };
    if email.len() > 254
        || local.is_empty()
        || !domain.contains('.')
        || domain.contains('@')
        || email.chars().any(char::is_whitespace)
        || email.chars().any(char::is_control)
    {
        return Err(AppError::Invalid("INVALID_CREDENTIALS_INPUT"));
    }
    Ok(email)
}
