//! Identity and pinned-learning persistence within the student transaction.
use super::PgStudentTransaction;
use crate::{
    modules::{
        assessment::lesson_quiz::LessonQuizDraft,
        identity::{application::NewSession, infrastructure::PgIdentityStore},
        learning::{
            application::{
                ports::*,
                results::{LessonQuizAnswerState, LessonQuizOptionView, LessonQuizView},
            },
            domain::*,
        },
    },
    platform::database::failure,
    shared::error::{AppError, AppResult},
};
use async_trait::async_trait;
use chrono::NaiveDate;
use sqlx::Row;
use uuid::Uuid;
#[async_trait]
impl IdentityRepository for PgStudentTransaction {
    async fn lock_school(&mut self, id: Uuid) -> AppResult<Option<(String, bool)>> {
        sqlx::query_as(
            "SELECT name,archived_at IS NULL FROM identity_schools WHERE id=$1 FOR UPDATE",
        )
        .bind(id)
        .fetch_optional(&mut *self.tx)
        .await
        .map_err(failure)
    }
    async fn insert_student(&mut self, id: Uuid, name: &str) -> AppResult<()> {
        sqlx::query("INSERT INTO identity_students(id,display_name) VALUES($1,$2)")
            .bind(id)
            .bind(name)
            .execute(&mut *self.tx)
            .await
            .map_err(failure)?;
        Ok(())
    }
    async fn insert_session(
        &mut self,
        s: &NewSession,
        old: Option<&[u8]>,
        sibling: Option<&[u8]>,
    ) -> AppResult<()> {
        // Match reviewed admin login's global role order, avoiding a rotation deadlock:
        // both roles take the admin session row before the student session row.
        if let Some(hash) = sibling {
            sqlx::query("UPDATE identity_sessions SET csrf_binding_hash=$2 WHERE token_hash=$1 AND role='admin' AND revoked_at IS NULL AND idle_expires_at>now() AND absolute_expires_at>now()")
                .bind(hash).bind(&s.csrf_binding_hash).execute(&mut *self.tx).await.map_err(failure)?;
        }
        if let Some(hash) = old {
            sqlx::query("UPDATE identity_sessions SET revoked_at=now() WHERE token_hash=$1 AND role='student' AND revoked_at IS NULL").bind(hash).execute(&mut *self.tx).await.map_err(failure)?;
        }
        PgIdentityStore::insert_in_transaction(&mut self.tx, s).await
    }
}
#[async_trait]
impl LearningRepository for PgStudentTransaction {
    async fn published_path(&mut self) -> AppResult<Option<PublishedPath>> {
        let row=sqlx::query("SELECT p.id,p.paired_bank_version_id,p.quiz_bank_version_id,
            pb.kind AS paired_kind,
            (SELECT count(*) FROM learning_path_items i WHERE i.path_version_id=p.id) AS lesson_count,
            (SELECT count(*) FROM assessment_bank_items i WHERE i.bank_version_id=p.paired_bank_version_id) AS paired_question_count
            FROM learning_path_versions p
            JOIN assessment_bank_versions pv ON pv.id=p.paired_bank_version_id
            JOIN assessment_banks pb ON pb.id=pv.bank_id
            ORDER BY p.version DESC LIMIT 1").fetch_optional(&mut *self.tx).await.map_err(failure)?;
        let Some(r) = row else {
            return Ok(None);
        };
        let paired_bank: Uuid = r.try_get("paired_bank_version_id").map_err(failure)?;
        let quiz_bank: Option<Uuid> = r.try_get("quiz_bank_version_id").map_err(failure)?;
        // Enrollment starts only from the immutable paired pre/post snapshot.
        let paired_weights = sqlx::query_scalar::<_, i64>("SELECT (q.max_points*10000)::bigint FROM assessment_bank_items i JOIN assessment_question_versions q ON q.id=i.question_version_id WHERE i.bank_version_id=$1 ORDER BY i.position")
            .bind(paired_bank).fetch_all(&mut *self.tx).await.map_err(failure)?;
        Ok(Some(PublishedPath {
            id: r.try_get("id").map_err(failure)?,
            paired_bank,
            quiz_bank,
            paired_kind: r.try_get("paired_kind").map_err(failure)?,
            quiz_kind: None,
            lesson_count: r.try_get("lesson_count").map_err(failure)?,
            paired_question_count: r.try_get("paired_question_count").map_err(failure)?,
            quiz_question_count: None,
            paired_weights,
            quiz_weights: vec![],
        }))
    }
    async fn insert_enrollment(
        &mut self,
        s: StudentScope,
        school: Uuid,
        label: &str,
        date: NaiveDate,
        path: Uuid,
    ) -> AppResult<()> {
        sqlx::query("INSERT INTO learning_enrollments(id,student_id,school_id,school_name_at_enrollment,form_date,path_version_id) VALUES($1,$2,$3,$4,$5,$6)").bind(s.enrollment_id).bind(s.student_id).bind(school).bind(label).bind(date).bind(path).execute(&mut *self.tx).await.map_err(failure)?;
        Ok(())
    }
    async fn journey(&mut self, s: StudentScope) -> AppResult<Journey> {
        let r=sqlx::query("SELECT e.*,s.display_name FROM learning_enrollments e JOIN identity_students s ON s.id=e.student_id WHERE e.id=$1 AND e.student_id=$2 FOR UPDATE OF e").bind(s.enrollment_id).bind(s.student_id).fetch_optional(&mut *self.tx).await.map_err(failure)?.ok_or(AppError::NotFound)?;
        let enrollment = Enrollment {
            id: s.enrollment_id,
            school_id: r.try_get("school_id").map_err(failure)?,
            school_name_at_enrollment: r.try_get("school_name_at_enrollment").map_err(failure)?,
            form_date: r.try_get("form_date").map_err(failure)?,
            path_version_id: r.try_get("path_version_id").map_err(failure)?,
            created_at: r.try_get("created_at").map_err(failure)?,
            completed_at: r.try_get("completed_at").map_err(failure)?,
        };
        let student = Student {
            id: s.student_id,
            name: r.try_get("display_name").map_err(failure)?,
        };
        let rows=sqlx::query("SELECT v.lesson_id,v.id AS lesson_version_id,v.title,i.position,p.opened_at,p.reading_completed_at,a.mastered_at AS completed_at FROM learning_path_items i JOIN learning_lesson_versions v ON v.id=i.lesson_version_id LEFT JOIN learning_lesson_progress p ON p.lesson_version_id=v.id AND p.enrollment_id=$1 LEFT JOIN learning_lesson_quiz_attempts a ON a.lesson_version_id=v.id AND a.enrollment_id=$1 WHERE i.path_version_id=$2 ORDER BY i.position,v.id").bind(s.enrollment_id).bind(enrollment.path_version_id).fetch_all(&mut *self.tx).await.map_err(failure)?;
        let lessons = rows
            .into_iter()
            .map(|r| {
                Ok(Lesson {
                    lesson_id: r.try_get("lesson_id").map_err(failure)?,
                    lesson_version_id: r.try_get("lesson_version_id").map_err(failure)?,
                    title: r.try_get("title").map_err(failure)?,
                    position: r.try_get("position").map_err(failure)?,
                    opened_at: r.try_get("opened_at").map_err(failure)?,
                    reading_completed_at: r.try_get("reading_completed_at").map_err(failure)?,
                    completed_at: r.try_get("completed_at").map_err(failure)?,
                })
            })
            .collect::<AppResult<Vec<_>>>()?;
        let attempts = self.attempts_for(s.enrollment_id).await?;
        let (history, _) = self.measurement_history(s, 1, 0).await?;
        Ok(Journey {
            student,
            enrollment,
            lessons,
            attempts,
            latest_measurement: history.into_iter().next(),
        })
    }
    async fn lesson_view(&mut self, s: StudentScope, l: &Lesson) -> AppResult<LessonView> {
        let description =
            sqlx::query_scalar("SELECT description FROM learning_lesson_versions WHERE id=$1")
                .bind(l.lesson_version_id)
                .fetch_one(&mut *self.tx)
                .await
                .map_err(failure)?;
        let rows=sqlx::query("SELECT id,title,body,required,position FROM learning_sections WHERE lesson_version_id=$1 ORDER BY position,id").bind(l.lesson_version_id).fetch_all(&mut *self.tx).await.map_err(failure)?;
        let sections = rows
            .into_iter()
            .map(|r| {
                Ok(Section {
                    id: r.try_get("id").map_err(failure)?,
                    title: r.try_get("title").map_err(failure)?,
                    body: r.try_get("body").map_err(failure)?,
                    required: r.try_get("required").map_err(failure)?,
                    position: r.try_get("position").map_err(failure)?,
                })
            })
            .collect::<AppResult<Vec<_>>>()?;
        let visited_section_ids=sqlx::query_scalar("SELECT section_id FROM learning_section_visits WHERE enrollment_id=$1 AND lesson_version_id=$2 ORDER BY section_id").bind(s.enrollment_id).bind(l.lesson_version_id).fetch_all(&mut *self.tx).await.map_err(failure)?;
        Ok(LessonView {
            lesson: l.clone(),
            description,
            sections,
            visited_section_ids,
        })
    }
    async fn lesson_quiz_view(&mut self, s: StudentScope, l: &Lesson) -> AppResult<LessonQuizView> {
        let row = sqlx::query("SELECT v.quiz_payload,a.first_option_id,a.attempt_count,a.revision,a.mastered FROM learning_lesson_versions v LEFT JOIN learning_lesson_quiz_attempts a ON a.lesson_version_id=v.id AND a.enrollment_id=$2 WHERE v.id=$1")
            .bind(l.lesson_version_id)
            .bind(s.enrollment_id)
            .fetch_optional(&mut *self.tx)
            .await.map_err(failure)?.ok_or(AppError::NotFound)?;
        let payload: Option<serde_json::Value> = row.try_get("quiz_payload").map_err(failure)?;
        let payload = payload.ok_or(AppError::Internal)?;
        let question = payload
            .get("text")
            .and_then(|v| v.as_str())
            .ok_or(AppError::Internal)?
            .to_owned();
        let options = payload
            .get("options")
            .and_then(|v| v.as_array())
            .ok_or(AppError::Internal)?
            .iter()
            .map(|option| {
                Ok(LessonQuizOptionView {
                    id: option
                        .get("id")
                        .and_then(|v| v.as_str())
                        .and_then(|v| Uuid::parse_str(v).ok())
                        .ok_or(AppError::Internal)?,
                    text: option
                        .get("text")
                        .and_then(|v| v.as_str())
                        .ok_or(AppError::Internal)?
                        .to_owned(),
                })
            })
            .collect::<AppResult<Vec<_>>>()?;
        Ok(LessonQuizView {
            lesson_id: l.lesson_id,
            lesson_version_id: l.lesson_version_id,
            question,
            options,
            first_answer_option_id: row.try_get("first_option_id").map_err(failure)?,
            attempt_count: row
                .try_get::<Option<i32>, _>("attempt_count")
                .map_err(failure)?
                .unwrap_or(0),
            revision: row
                .try_get::<Option<i64>, _>("revision")
                .map_err(failure)?
                .unwrap_or(0),
            mastered: row
                .try_get::<Option<bool>, _>("mastered")
                .map_err(failure)?
                .unwrap_or(false),
        })
    }
    async fn lesson_quiz_content(&mut self, version: Uuid) -> AppResult<LessonQuizDraft> {
        let payload: Option<serde_json::Value> =
            sqlx::query_scalar("SELECT quiz_payload FROM learning_lesson_versions WHERE id=$1")
                .bind(version)
                .fetch_optional(&mut *self.tx)
                .await
                .map_err(failure)?
                .ok_or(AppError::NotFound)?;
        serde_json::from_value(payload.ok_or(AppError::Internal)?).map_err(|_| AppError::Internal)
    }
    async fn record_lesson_quiz_answer(
        &mut self,
        s: StudentScope,
        version: Uuid,
        option: Uuid,
        expected: i64,
        correct: bool,
    ) -> AppResult<LessonQuizAnswerState> {
        let id = Uuid::new_v4();
        sqlx::query("INSERT INTO learning_lesson_quiz_attempts(id,enrollment_id,lesson_version_id) VALUES($1,$2,$3) ON CONFLICT(enrollment_id,lesson_version_id) DO NOTHING")
            .bind(id).bind(s.enrollment_id).bind(version).execute(&mut *self.tx).await.map_err(failure)?;
        let row = sqlx::query("SELECT id,first_option_id,attempt_count,revision,mastered FROM learning_lesson_quiz_attempts WHERE enrollment_id=$1 AND lesson_version_id=$2 FOR UPDATE")
            .bind(s.enrollment_id).bind(version).fetch_one(&mut *self.tx).await.map_err(failure)?;
        let attempt_id: Uuid = row.try_get("id").map_err(failure)?;
        let current_revision: i64 = row.try_get("revision").map_err(failure)?;
        let count: i32 = row.try_get("attempt_count").map_err(failure)?;
        let mastered: bool = row.try_get("mastered").map_err(failure)?;
        if mastered {
            return Err(AppError::Conflict("LESSON_QUIZ_ALREADY_MASTERED"));
        }
        if current_revision != expected {
            return Err(AppError::Conflict("REVISION_CONFLICT"));
        }
        let revision = current_revision.checked_add(1).ok_or(AppError::Internal)?;
        let count = count.checked_add(1).ok_or(AppError::Internal)?;
        sqlx::query("INSERT INTO learning_lesson_quiz_answers(attempt_id,revision,option_id,is_correct) VALUES($1,$2,$3,$4)")
            .bind(attempt_id).bind(revision).bind(option).bind(correct).execute(&mut *self.tx).await.map_err(failure)?;
        let row = sqlx::query("UPDATE learning_lesson_quiz_attempts SET first_option_id=COALESCE(first_option_id,$2),attempt_count=$3,revision=$4,mastered=$5,first_answered_at=COALESCE(first_answered_at,now()),mastered_at=CASE WHEN $5 THEN COALESCE(mastered_at,now()) ELSE NULL END WHERE id=$1 RETURNING first_option_id,attempt_count,revision,mastered")
            .bind(attempt_id).bind(option).bind(count).bind(revision).bind(correct).fetch_one(&mut *self.tx).await.map_err(failure)?;
        Ok(LessonQuizAnswerState {
            first_answer_option_id: row.try_get("first_option_id").map_err(failure)?,
            attempt_count: row.try_get("attempt_count").map_err(failure)?,
            revision: row.try_get("revision").map_err(failure)?,
            mastered: row.try_get("mastered").map_err(failure)?,
        })
    }
    async fn open_lesson(
        &mut self,
        s: StudentScope,
        version: Uuid,
        section: Option<Uuid>,
    ) -> AppResult<()> {
        sqlx::query("INSERT INTO learning_lesson_progress(enrollment_id,lesson_version_id,opened_at) VALUES($1,$2,now()) ON CONFLICT DO NOTHING").bind(s.enrollment_id).bind(version).execute(&mut *self.tx).await.map_err(failure)?;
        if let Some(section) = section {
            sqlx::query("INSERT INTO learning_section_visits(enrollment_id,lesson_version_id,section_id) VALUES($1,$2,$3) ON CONFLICT DO NOTHING").bind(s.enrollment_id).bind(version).bind(section).execute(&mut *self.tx).await.map_err(failure)?;
        }
        Ok(())
    }
    async fn complete_lesson(&mut self, s: StudentScope, version: Uuid) -> AppResult<()> {
        sqlx::query("UPDATE learning_lesson_progress SET reading_completed_at=COALESCE(reading_completed_at,GREATEST(clock_timestamp(),opened_at)) WHERE enrollment_id=$1 AND lesson_version_id=$2").bind(s.enrollment_id).bind(version).execute(&mut *self.tx).await.map_err(failure)?;
        Ok(())
    }
    async fn complete_enrollment(&mut self, s: StudentScope) -> AppResult<()> {
        sqlx::query("UPDATE learning_enrollments SET completed_at=COALESCE(completed_at,now()) WHERE id=$1 AND student_id=$2").bind(s.enrollment_id).bind(s.student_id).execute(&mut *self.tx).await.map_err(failure)?;
        Ok(())
    }
}
