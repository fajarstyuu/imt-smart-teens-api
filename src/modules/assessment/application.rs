//! Attempt lifecycle, serialized autosave, and atomic scoring/progress transitions.
use crate::{
    modules::{
        assessment::domain::*,
        learning::{
            application::{Operation, StudentService},
            domain::StudentScope,
        },
    },
    shared::error::{AppError, AppResult},
};
use serde_json::{Value, json};
use uuid::Uuid;
impl StudentService {
    /// Return the enrollment's one attempt, or create it after its stage prerequisite is met.
    pub async fn create_attempt(
        &self,
        scope: StudentScope,
        typ: AssessmentType,
        op: Operation,
    ) -> AppResult<(u16, Value)> {
        let mut tx = self.uow.begin().await?;
        if let Some(out) = op.replay(&mut *tx, "attempt.create").await? {
            tx.commit().await?;
            return Ok(out);
        }
        let j = tx.journey(scope).await?;
        j.require_attempt(typ)?;
        let (status, a) = if let Some(a) = j.attempts.iter().find(|a| a.assessment_type == typ) {
            (200, a.clone())
        } else {
            (201, tx.create_attempt(scope.enrollment_id, typ).await?)
        };
        let out = json!({"attempt_id":a.id,"assessment_type":a.assessment_type,"attempt_number":a.attempt_number,"status":a.status,"snapshot_id":a.snapshot_id});
        op.remember(&mut *tx, "attempt.create", status, out.clone())
            .await?;
        tx.commit().await?;
        Ok((status, out))
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
            let safe_latest = latest.map(|answer| json!(answer)).unwrap_or_else(
                || json!({"snapshot_question_id":qid,"option_ids":[],"revision":0,"saved_at":null}),
            );
            return Err(AppError::ConflictDetails {
                code: "ANSWER_REVISION_CONFLICT",
                details: json!({"latest_answer":safe_latest}),
            });
        }
        let out = tx.write_answer(&a, q, &ids, expected + 1).await?;
        tx.commit().await?;
        Ok(out)
    }
    /// Score once and advance durable progress atomically. Lock order: operation, enrollment, attempt.
    /// Repeats reuse the recorded result; missing required answers roll back every scoring write.
    pub async fn submit(&self, scope: StudentScope, id: Uuid, op: Operation) -> AppResult<Value> {
        let mut tx = self.uow.begin().await?;
        // Reserve the operation before taking business locks. Every retriable mutation follows this order.
        let cached = op.replay(&mut *tx, &format!("attempt.submit:{id}")).await?;
        let j = tx.journey(scope).await?;
        let a = tx.lock_attempt(scope, id).await?;
        j.require_attempt(a.assessment_type)?;
        if let Some((_, out)) = cached {
            tx.commit().await?;
            return Ok(out);
        }
        if a.status != "submitted" {
            let questions = tx.questions(a.snapshot_id).await?;
            let answers = tx.answers(id).await?;
            let mut earned = 0i64;
            let mut max = 0i64;
            for q in &questions {
                let ids = answers
                    .iter()
                    .find(|a| a.snapshot_question_id == q.safe.id)
                    .map(|a| a.option_ids.as_slice())
                    .unwrap_or(&[]);
                let (points, correct) = q.score(ids)?;
                earned = earned.checked_add(points).ok_or(AppError::Internal)?;
                max = max.checked_add(q.max_units).ok_or(AppError::Internal)?;
                tx.score_answer(id, q.safe.id, points, correct).await?;
            }
            if max <= 0 {
                return Err(AppError::Conflict("CONTENT_NOT_AVAILABLE"));
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
        let out = json!({"attempt_id":a.id,"status":a.status,"submitted_at":a.submitted_at,"score":j.visible_score(a),"progress":j.progress()});
        op.remember(&mut *tx, &format!("attempt.submit:{id}"), 200, out.clone())
            .await?;
        tx.commit().await?;
        Ok(out)
    }
}
