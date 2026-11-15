//! Cross-context registration: school lock, identity, pins, pre-test, session and replay commit together.
use crate::{
    modules::{
        assessment::domain::AssessmentType,
        identity::{
            application::{IssuedSession, SessionSubject},
            domain::{FormDate, StudentName},
        },
        learning::{
            application::{
                Operation, OperationResult, StudentProfile, StudentService, fingerprint_fields,
            },
            domain::StudentScope,
        },
    },
    shared::error::{AppError, AppResult},
};
use uuid::Uuid;
pub struct Registration {
    pub name: String,
    pub school_id: Uuid,
    pub form_date: String,
}
pub struct RegistrationContext {
    pub operation: Operation,
    pub binding: Vec<u8>,
    pub previous_student_hash: Option<Vec<u8>>,
    pub admin_hash: Option<Vec<u8>>,
}
impl StudentService {
    /// Create a new identity or securely replay the same browser operation; all writes commit together.
    /// Any school/content/persistence failure drops the transaction without issuing a cookie.
    pub async fn register(
        &self,
        input: Registration,
        mut context: RegistrationContext,
    ) -> AppResult<(StudentProfile, IssuedSession)> {
        let name = StudentName::parse(&input.name)?;
        let date = FormDate::parse(&input.form_date)?;
        // Fingerprint semantic, validated input rather than whitespace/Unicode spelling.
        context.operation = Operation::new(
            context.operation.scope,
            context.operation.key,
            &fingerprint_fields(&[
                name.as_str().as_bytes(),
                input.school_id.as_bytes(),
                date.value().to_string().as_bytes(),
            ]),
            context.operation.request_id,
        )?;
        let mut tx = self.uow.begin().await?;
        let replay = context.operation.replay(&mut *tx, "enrollment").await?;
        let (scope, body) = if let Some(replay) = replay {
            let OperationResult::Enrollment(body) = replay else {
                return Err(AppError::Internal);
            };
            let scope = StudentScope {
                student_id: body.student.id,
                enrollment_id: body.enrollment.id,
            };
            (scope, body)
        } else {
            let (label, active) = tx
                .lock_school(input.school_id)
                .await?
                .ok_or(AppError::Invalid("SCHOOL_NOT_AVAILABLE"))?;
            if !active {
                return Err(AppError::Invalid("SCHOOL_NOT_AVAILABLE"));
            }
            let path = tx
                .published_path()
                .await?
                .ok_or(AppError::Conflict("CONTENT_NOT_AVAILABLE"))?;
            path.require_available()?;
            let scope = StudentScope {
                student_id: Uuid::new_v4(),
                enrollment_id: Uuid::new_v4(),
            };
            tx.insert_student(scope.student_id, name.as_str()).await?;
            tx.insert_enrollment(scope, input.school_id, &label, date.value(), path.id)
                .await?;
            tx.pin_snapshots(scope.enrollment_id, &path).await?;
            tx.create_attempt(scope.enrollment_id, AssessmentType::PreTest)
                .await?;
            let j = tx.journey(scope).await?;
            let body = StudentProfile {
                student: j.student,
                enrollment: j.enrollment,
            };
            context
                .operation
                .remember(
                    &mut *tx,
                    "enrollment",
                    OperationResult::Enrollment(body.clone()),
                )
                .await?;
            tx.audit(
                "student.enroll",
                scope.enrollment_id,
                &context.operation.request_id,
            )
            .await?;
            (scope, body)
        };
        let issued = self.sessions.prepare(
            SessionSubject::Student {
                student_id: scope.student_id,
                enrollment_id: scope.enrollment_id,
            },
            context.binding,
        )?;
        tx.insert_session(
            &issued.record,
            context.previous_student_hash.as_deref(),
            context.admin_hash.as_deref(),
        )
        .await?;
        tx.commit().await?;
        Ok((body, issued))
    }
}
