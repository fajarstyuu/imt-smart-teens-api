//! Attempt lifecycle, serialized autosave, and atomic scoring/progress transitions.
use crate::{
    modules::{
        assessment::domain::*,
        learning::{
            application::{
                AttemptCreation, Creation, Operation, OperationResult, StudentService, Submission,
            },
            domain::StudentScope,
        },
    },
    shared::error::{AppError, AppResult},
};
use uuid::Uuid;
impl StudentService {
    /// Return the enrollment's one attempt, or create it after its stage prerequisite is met.
    pub async fn create_attempt(
        &self,
        scope: StudentScope,
        typ: AssessmentType,
        op: Operation,
    ) -> AppResult<AttemptCreation> {
        let mut tx = self.uow.begin().await?;
        if let Some(out) = op.replay(&mut *tx, "attempt.create").await? {
            let OperationResult::Attempt(out) = out else {
                return Err(AppError::Internal);
            };
            tx.commit().await?;
            return Ok(out);
        }
        let j = tx.journey(scope).await?;
        j.require_attempt(typ)?;
        let (outcome, a) = if let Some(a) = j.attempts.iter().find(|a| a.assessment_type == typ) {
            (Creation::Existing, a.clone())
        } else {
            (
                Creation::Created,
                tx.create_attempt(scope.enrollment_id, typ).await?,
            )
        };
        let out = AttemptCreation {
            outcome,
            attempt: a.into(),
        };
        op.remember(
            &mut *tx,
            "attempt.create",
            OperationResult::Attempt(out.clone()),
        )
        .await?;
        tx.commit().await?;
        Ok(out)
    }
    /// Restore safe pinned questions and own saved answers; no private key fields are serializable.
    pub async fn attempt(&self, scope: StudentScope, id: Uuid) -> AppResult<AttemptView> {
        let mut tx = self.uow.begin().await?;
        let j = tx.journey(scope).await?;
        let mut a = tx.lock_attempt(scope, id).await?;
        j.require_attempt(a.assessment_type)?;
        let questions = tx
            .questions(a.snapshot_id)
            .await?
            .into_iter()
            .map(|q| q.safe)
            .collect();
        let answers = tx.answers(a.id).await?;
        a.score = j.visible_score(&a);
        let out = AttemptView {
            attempt: a,
            questions,
            answers,
        };
        tx.commit().await?;
        Ok(out)
    }
    /// Replace one answer set while open, comparing the committed revision under the attempt lock.
    /// A conflict includes only the latest student-safe answer, never private scoring data.
    pub async fn save_answer(
        &self,
        scope: StudentScope,
        id: Uuid,
        qid: Uuid,
        ids: Vec<Uuid>,
        expected: i64,
    ) -> AppResult<Answer> {
        if expected < 0 || expected == i64::MAX {
            return Err(AppError::Invalid("INVALID_ANSWER_REVISION"));
        }
        let mut tx = self.uow.begin().await?;
        // This path takes only attempt, never enrollment; submission always takes enrollment then attempt.
        let a = tx.lock_attempt(scope, id).await?;
        if a.status == "submitted" {
            return Err(AppError::Conflict("ATTEMPT_SUBMITTED"));
        }
        let questions = tx.questions(a.snapshot_id).await?;
        let q = questions
            .iter()
            .find(|q| q.safe.id == qid)
            .ok_or(AppError::NotFound)?;
        q.validate_answer(&ids)?;
        let answers = tx.answers(id).await?;
        let latest = answers.iter().find(|v| v.snapshot_question_id == qid);
        if latest.map(|v| v.revision).unwrap_or(0) != expected {
            return Err(AppError::ConflictDetails {
                code: "ANSWER_REVISION_CONFLICT",
                details: crate::shared::error::ConflictDetails::AnswerRevision {
                    snapshot_question_id: qid,
                    option_ids: latest.map(|a| a.option_ids.clone()).unwrap_or_default(),
                    revision: latest.map(|a| a.revision).unwrap_or(0),
                    saved_at: latest.map(|a| a.saved_at),
                },
            });
        }
        let out = tx.write_answer(&a, q, &ids, expected + 1).await?;
        tx.commit().await?;
        Ok(out)
    }
    /// Score once and advance durable progress atomically. Lock order: operation, enrollment, attempt.
    /// Repeats reuse the recorded result; missing required answers roll back every scoring write.
    pub async fn submit(
        &self,
        scope: StudentScope,
        id: Uuid,
        op: Operation,
    ) -> AppResult<Submission> {
        let mut tx = self.uow.begin().await?;
        // Reserve the operation before taking business locks. Every retriable mutation follows this order.
        let cached = op.replay(&mut *tx, &format!("attempt.submit:{id}")).await?;
        let j = tx.journey(scope).await?;
        let a = tx.lock_attempt(scope, id).await?;
        j.require_attempt(a.assessment_type)?;
        if let Some(out) = cached {
            let OperationResult::Submission(out) = out else {
                return Err(AppError::Internal);
            };
            tx.commit().await?;
            return Ok(out);
        }
        if a.status != "submitted" {
            let questions = tx.questions(a.snapshot_id).await?;
            let answers = tx.answers(id).await?;
            let mut earned = 0i64;
            // Validate the complete pinned aggregate before even per-answer scoring writes.
            let max = validate_score_budget(questions.iter().map(|q| q.max_units))?;
            for q in &questions {
                let ids = answers
                    .iter()
                    .find(|a| a.snapshot_question_id == q.safe.id)
                    .map(|a| a.option_ids.as_slice())
                    .unwrap_or(&[]);
                let (points, correct) = q.score(ids)?;
                earned = earned.checked_add(points).ok_or(AppError::Internal)?;

                tx.score_answer(id, q.safe.id, points, correct).await?;
            }
            let percent =
                ((i128::from(earned) * 100 + i128::from(max) / 2) / i128::from(max)) as i32;
            tx.submit_attempt(id, earned, max, percent).await?;
            if a.assessment_type == AssessmentType::PostTest {
                tx.complete_enrollment(scope).await?;
            }
            tx.audit("attempt.submit", id, &op.request_id).await?;
        }
        let j = tx.journey(scope).await?;
        let a = j
            .attempts
            .iter()
            .find(|a| a.id == id)
            .ok_or(AppError::Internal)?;
        let out = Submission {
            attempt_id: a.id,
            submitted_at: a.submitted_at,
            score: j.visible_score(a),
            progress: j.progress(),
        };
        op.remember(
            &mut *tx,
            &format!("attempt.submit:{id}"),
            OperationResult::Submission(out.clone()),
        )
        .await?;
        tx.commit().await?;
        Ok(out)
    }
}
