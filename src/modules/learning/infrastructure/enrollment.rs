//! Identity and pinned-learning persistence within the student transaction.
use super::PgStudentTransaction;
use crate::{
    modules::{
        identity::{application::NewSession, infrastructure::PgIdentityStore},
        learning::{application::ports::*, domain::*},
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
            pb.kind AS paired_kind,qb.kind AS quiz_kind,
            (SELECT count(*) FROM learning_path_items i WHERE i.path_version_id=p.id) AS lesson_count,
            (SELECT count(*) FROM assessment_bank_items i WHERE i.bank_version_id=p.paired_bank_version_id) AS paired_question_count,
            (SELECT count(*) FROM assessment_bank_items i WHERE i.bank_version_id=p.quiz_bank_version_id) AS quiz_question_count
            FROM learning_path_versions p
            JOIN assessment_bank_versions pv ON pv.id=p.paired_bank_version_id
            JOIN assessment_banks pb ON pb.id=pv.bank_id
            JOIN assessment_bank_versions qv ON qv.id=p.quiz_bank_version_id
            JOIN assessment_banks qb ON qb.id=qv.bank_id
            ORDER BY p.version DESC LIMIT 1").fetch_optional(&mut *self.tx).await.map_err(failure)?;
        row.map(|r| {
            Ok(PublishedPath {
                id: r.try_get("id").map_err(failure)?,
                paired_bank: r.try_get("paired_bank_version_id").map_err(failure)?,
                quiz_bank: r.try_get("quiz_bank_version_id").map_err(failure)?,
                paired_kind: r.try_get("paired_kind").map_err(failure)?,
                quiz_kind: r.try_get("quiz_kind").map_err(failure)?,
                lesson_count: r.try_get("lesson_count").map_err(failure)?,
                paired_question_count: r.try_get("paired_question_count").map_err(failure)?,
                quiz_question_count: r.try_get("quiz_question_count").map_err(failure)?,
            })
        })
        .transpose()
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
        let rows=sqlx::query("SELECT v.lesson_id,v.id AS lesson_version_id,v.title,i.position,p.opened_at,p.completed_at FROM learning_path_items i JOIN learning_lesson_versions v ON v.id=i.lesson_version_id LEFT JOIN learning_lesson_progress p ON p.lesson_version_id=v.id AND p.enrollment_id=$1 WHERE i.path_version_id=$2 ORDER BY i.position,v.id").bind(s.enrollment_id).bind(enrollment.path_version_id).fetch_all(&mut *self.tx).await.map_err(failure)?;
        let lessons = rows
            .into_iter()
            .map(|r| {
                Ok(Lesson {
                    lesson_id: r.try_get("lesson_id").map_err(failure)?,
                    lesson_version_id: r.try_get("lesson_version_id").map_err(failure)?,
                    title: r.try_get("title").map_err(failure)?,
                    position: r.try_get("position").map_err(failure)?,
                    opened_at: r.try_get("opened_at").map_err(failure)?,
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
        sqlx::query("UPDATE learning_lesson_progress SET completed_at=COALESCE(completed_at,now()) WHERE enrollment_id=$1 AND lesson_version_id=$2").bind(s.enrollment_id).bind(version).execute(&mut *self.tx).await.map_err(failure)?;
        Ok(())
    }
    async fn complete_enrollment(&mut self, s: StudentScope) -> AppResult<()> {
        sqlx::query("UPDATE learning_enrollments SET completed_at=COALESCE(completed_at,now()) WHERE id=$1 AND student_id=$2").bind(s.enrollment_id).bind(s.student_id).execute(&mut *self.tx).await.map_err(failure)?;
        Ok(())
    }
}
