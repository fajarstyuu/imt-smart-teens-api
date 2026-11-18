//! Snapshot references and attempt/answer row mapping. No scoring policy lives in SQL.
use super::PgStudentTransaction;
use crate::{
    modules::{
        assessment::domain::*,
        learning::{application::ports::*, domain::StudentScope},
    },
    platform::database::failure,
    shared::error::{AppError, AppResult},
};
use async_trait::async_trait;
use sqlx::{Row, postgres::PgRow};
use uuid::Uuid;
fn map_attempt(r: PgRow) -> AppResult<Attempt> {
    let typ: String = r.try_get("assessment_type").map_err(failure)?;
    let assessment_type = match typ.as_str() {
        "pre_test" => AssessmentType::PreTest,
        "quiz" => AssessmentType::Quiz,
        "post_test" => AssessmentType::PostTest,
        _ => return Err(AppError::Internal),
    };
    Ok(Attempt {
        id: r.try_get("id").map_err(failure)?,
        assessment_type,
        attempt_number: r.try_get("attempt_number").map_err(failure)?,
        status: r.try_get("status").map_err(failure)?,
        snapshot_id: r.try_get("snapshot_id").map_err(failure)?,
        submitted_at: r.try_get("submitted_at").map_err(failure)?,
        score: r.try_get("score").map_err(failure)?,
    })
}
impl PgStudentTransaction {
    pub(super) async fn attempts_for(&mut self, enrollment: Uuid) -> AppResult<Vec<Attempt>> {
        sqlx::query("SELECT *,score_percent::integer AS score FROM assessment_attempts WHERE enrollment_id=$1 ORDER BY created_at,id").bind(enrollment).fetch_all(&mut *self.tx).await.map_err(failure)?.into_iter().map(map_attempt).collect()
    }
}
#[async_trait]
impl AssessmentRepository for PgStudentTransaction {
    async fn pin_snapshots(&mut self, enrollment: Uuid, path: &PublishedPath) -> AppResult<()> {
        let snapshot = Uuid::new_v4();
        sqlx::query("INSERT INTO assessment_snapshots(id,enrollment_id,kind,bank_version_id) VALUES($1,$2,'paired',$3)")
            .bind(snapshot).bind(enrollment).bind(path.paired_bank).execute(&mut *self.tx).await.map_err(failure)?;
        let rows: Vec<(Uuid,i32)> = sqlx::query_as("SELECT question_version_id,position FROM assessment_bank_items WHERE bank_version_id=$1 ORDER BY position")
            .bind(path.paired_bank).fetch_all(&mut *self.tx).await.map_err(failure)?;
        for (version, position) in rows {
            sqlx::query("INSERT INTO assessment_snapshot_questions(id,snapshot_id,question_version_id,position) VALUES($1,$2,$3,$4)")
                .bind(Uuid::new_v4()).bind(snapshot).bind(version).bind(position).execute(&mut *self.tx).await.map_err(failure)?;
        }
        Ok(())
    }
    async fn create_attempt(
        &mut self,
        enrollment: Uuid,
        typ: AssessmentType,
    ) -> AppResult<Attempt> {
        let r=sqlx::query("INSERT INTO assessment_attempts(id,enrollment_id,snapshot_id,assessment_type,attempt_number,status) SELECT $1,$2,id,$3,1,'in_progress' FROM assessment_snapshots WHERE enrollment_id=$2 AND kind=$4 RETURNING *,score_percent::integer AS score").bind(Uuid::new_v4()).bind(enrollment).bind(typ.as_str()).bind(typ.kind()).fetch_one(&mut *self.tx).await.map_err(failure)?;
        map_attempt(r)
    }
    async fn lock_attempt(&mut self, s: StudentScope, id: Uuid) -> AppResult<Attempt> {
        let r=sqlx::query("SELECT a.*,a.score_percent::integer AS score FROM assessment_attempts a JOIN learning_enrollments e ON e.id=a.enrollment_id WHERE a.id=$1 AND a.enrollment_id=$2 AND e.student_id=$3 FOR UPDATE OF a").bind(id).bind(s.enrollment_id).bind(s.student_id).fetch_optional(&mut *self.tx).await.map_err(failure)?.ok_or(AppError::NotFound)?;
        map_attempt(r)
    }
    async fn questions(&mut self, snapshot: Uuid) -> AppResult<Vec<Question>> {
        let rows=sqlx::query("SELECT sq.id,sq.position,q.id AS question_version_id,q.question_id,q.text,q.type,q.required,(q.max_points*10000)::bigint AS max_units FROM assessment_snapshot_questions sq JOIN assessment_question_versions q ON q.id=sq.question_version_id WHERE sq.snapshot_id=$1 ORDER BY sq.position,sq.id").bind(snapshot).fetch_all(&mut *self.tx).await.map_err(failure)?;
        // One batched option query preserves immutable option order without per-question round trips.
        let options=sqlx::query("SELECT o.id,o.question_version_id,o.text,o.is_correct FROM assessment_options o JOIN assessment_snapshot_questions sq ON sq.question_version_id=o.question_version_id WHERE sq.snapshot_id=$1 ORDER BY o.position,o.id").bind(snapshot).fetch_all(&mut *self.tx).await.map_err(failure)?;
        let mut out = vec![];
        for r in rows {
            let version: Uuid = r.try_get("question_version_id").map_err(failure)?;
            let mut safe = vec![];
            let mut keys = vec![];
            for o in &options {
                if o.try_get::<Uuid, _>("question_version_id")
                    .map_err(failure)?
                    != version
                {
                    continue;
                }
                let id = o.try_get("id").map_err(failure)?;
                if o.try_get::<bool, _>("is_correct").map_err(failure)? {
                    keys.push(id);
                }
                safe.push(SafeOption {
                    id,
                    text: o.try_get("text").map_err(failure)?,
                });
            }
            out.push(Question {
                safe: SafeQuestion {
                    id: r.try_get("id").map_err(failure)?,
                    question_id: r.try_get("question_id").map_err(failure)?,
                    question_version_id: version,
                    position: r.try_get("position").map_err(failure)?,
                    text: r.try_get("text").map_err(failure)?,
                    question_type: r.try_get("type").map_err(failure)?,
                    required: r.try_get("required").map_err(failure)?,
                    options: safe,
                },
                correct_option_ids: keys,
                max_units: r.try_get("max_units").map_err(failure)?,
            });
        }
        Ok(out)
    }
    async fn answers(&mut self, attempt: Uuid) -> AppResult<Vec<Answer>> {
        let rows=sqlx::query("SELECT a.snapshot_question_id,a.revision,a.saved_at,COALESCE(array_agg(o.option_id ORDER BY o.option_id) FILTER(WHERE o.option_id IS NOT NULL),'{}'::uuid[]) AS option_ids FROM assessment_answers a LEFT JOIN assessment_answer_options o ON o.attempt_id=a.attempt_id AND o.snapshot_question_id=a.snapshot_question_id WHERE a.attempt_id=$1 GROUP BY a.snapshot_question_id,a.revision,a.saved_at ORDER BY a.snapshot_question_id").bind(attempt).fetch_all(&mut *self.tx).await.map_err(failure)?;
        rows.into_iter()
            .map(|r| {
                Ok(Answer {
                    snapshot_question_id: r.try_get("snapshot_question_id").map_err(failure)?,
                    option_ids: r.try_get("option_ids").map_err(failure)?,
                    revision: r.try_get("revision").map_err(failure)?,
                    saved_at: r.try_get("saved_at").map_err(failure)?,
                })
            })
            .collect()
    }
    async fn write_answer(
        &mut self,
        a: &Attempt,
        q: &Question,
        ids: &[Uuid],
        revision: i64,
    ) -> AppResult<Answer> {
        let saved_at=sqlx::query_scalar("INSERT INTO assessment_answers(attempt_id,snapshot_id,snapshot_question_id,revision) VALUES($1,$2,$3,$4) ON CONFLICT(attempt_id,snapshot_question_id) DO UPDATE SET revision=$4,saved_at=now() RETURNING saved_at").bind(a.id).bind(a.snapshot_id).bind(q.safe.id).bind(revision).fetch_one(&mut *self.tx).await.map_err(failure)?;
        sqlx::query(
            "DELETE FROM assessment_answer_options WHERE attempt_id=$1 AND snapshot_question_id=$2",
        )
        .bind(a.id)
        .bind(q.safe.id)
        .execute(&mut *self.tx)
        .await
        .map_err(failure)?;
        for id in ids {
            sqlx::query("INSERT INTO assessment_answer_options(attempt_id,snapshot_id,snapshot_question_id,question_version_id,option_id) VALUES($1,$2,$3,$4,$5)").bind(a.id).bind(a.snapshot_id).bind(q.safe.id).bind(q.safe.question_version_id).bind(id).execute(&mut *self.tx).await.map_err(failure)?;
        }
        let mut option_ids = ids.to_vec();
        option_ids.sort();
        Ok(Answer {
            snapshot_question_id: q.safe.id,
            option_ids,
            revision,
            saved_at,
        })
    }
    async fn score_answer(
        &mut self,
        a: Uuid,
        q: Uuid,
        earned: i64,
        correct: bool,
    ) -> AppResult<()> {
        sqlx::query("UPDATE assessment_answers SET earned_points=$3::bigint::numeric/10000,is_correct=$4 WHERE attempt_id=$1 AND snapshot_question_id=$2").bind(a).bind(q).bind(earned).bind(correct).execute(&mut *self.tx).await.map_err(failure)?;
        Ok(())
    }
    async fn submit_attempt(
        &mut self,
        id: Uuid,
        earned: i64,
        max: i64,
        percent: i32,
    ) -> AppResult<()> {
        sqlx::query("UPDATE assessment_attempts SET status='submitted',submitted_at=now(),earned_points=$2::bigint::numeric/10000,max_points=$3::bigint::numeric/10000,score_percent=$4 WHERE id=$1").bind(id).bind(earned).bind(max).bind(percent).execute(&mut *self.tx).await.map_err(failure)?;
        Ok(())
    }
}
