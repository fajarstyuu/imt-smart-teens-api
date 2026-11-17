//! Student/enrollment summaries, filters and detail selection share the same scoped relation.
use super::{super::application::*, read};
use crate::shared::editing::Page;
use crate::{
    modules::{
        anthropometry::domain::{Bmi, Measurement, PositiveInput},
        learning::domain::{Enrollment, Lesson, Student},
    },
    platform::database::failure,
    shared::error::{AppError, AppResult},
};
use sqlx::{PgPool, Postgres, QueryBuilder, Row, postgres::PgRow};
use uuid::Uuid;
// This is a read projection of the durable Journey policy, not a separately stored stage.
// An active admin actor is required before every count/order/limit; no school grants.
fn filtered<'a>(admin: Uuid, f: &'a Filters) -> QueryBuilder<'a, Postgres> {
    let mut b = QueryBuilder::new(
        "WITH scoped AS (SELECT e.*,s.display_name AS student_name,s.created_at AS student_created_at,CASE WHEN NOT EXISTS(SELECT 1 FROM assessment_attempts a WHERE a.enrollment_id=e.id AND a.assessment_type='pre_test' AND a.status='submitted') THEN 'pre_test' WHEN NOT EXISTS(SELECT 1 FROM learning_path_items i WHERE i.path_version_id=e.path_version_id) OR EXISTS(SELECT 1 FROM learning_path_items i LEFT JOIN learning_lesson_progress p ON p.enrollment_id=e.id AND p.lesson_version_id=i.lesson_version_id WHERE i.path_version_id=e.path_version_id AND p.completed_at IS NULL) THEN 'lessons' WHEN NOT EXISTS(SELECT 1 FROM assessment_attempts a WHERE a.enrollment_id=e.id AND a.assessment_type='quiz' AND a.status='submitted') THEN 'quiz' WHEN NOT EXISTS(SELECT 1 FROM anthropometry_measurements m WHERE m.enrollment_id=e.id) THEN 'calculator' WHEN NOT EXISTS(SELECT 1 FROM assessment_attempts a WHERE a.enrollment_id=e.id AND a.assessment_type='post_test' AND a.status='submitted') THEN 'post_test' ELSE 'completed' END AS stage,(SELECT max(a.submitted_at) FROM assessment_attempts a WHERE a.enrollment_id=e.id) AS latest_submitted_at,(SELECT a.score_percent::integer FROM assessment_attempts a WHERE a.enrollment_id=e.id AND a.assessment_type='pre_test' AND a.status='submitted' ORDER BY a.submitted_at DESC,a.id DESC LIMIT 1) AS pre_test_score,(SELECT a.score_percent::integer FROM assessment_attempts a WHERE a.enrollment_id=e.id AND a.assessment_type='post_test' AND a.status='submitted' ORDER BY a.submitted_at DESC,a.id DESC LIMIT 1) AS post_test_score,GREATEST(e.created_at,e.completed_at,(SELECT max(a.submitted_at) FROM assessment_attempts a WHERE a.enrollment_id=e.id),(SELECT max(a.saved_at) FROM assessment_answers a JOIN assessment_attempts t ON t.id=a.attempt_id WHERE t.enrollment_id=e.id),(SELECT max(GREATEST(p.opened_at,p.completed_at)) FROM learning_lesson_progress p WHERE p.enrollment_id=e.id),(SELECT max(m.created_at) FROM anthropometry_measurements m WHERE m.enrollment_id=e.id)) AS latest_activity_at FROM learning_enrollments e JOIN identity_students s ON s.id=e.student_id WHERE EXISTS(SELECT 1 FROM identity_admins WHERE id=",
    );
    b.push_bind(admin).push(" AND active)");
    if let Some(q) = &f.q {
        // Literal substring matching: percent/underscore are text, never wildcard operators.
        b.push(" AND strpos(lower(s.display_name),lower(")
            .push_bind(q.trim())
            .push("))>0");
    }
    if let Some(id) = f.school_id {
        b.push(" AND e.school_id=").push_bind(id);
    }
    if let Some(d) = f.form_date_from {
        b.push(" AND e.form_date>=").push_bind(d);
    }
    if let Some(d) = f.form_date_to {
        b.push(" AND e.form_date<=").push_bind(d);
    }
    b.push("), visible AS (SELECT * FROM scoped WHERE TRUE");
    if let Some(s) = f.stage {
        b.push(" AND stage=").push_bind(stage_name(s));
    }
    if f.assessment_type.is_some() || f.submitted_from.is_some() || f.submitted_to.is_some() {
        b.push(" AND EXISTS(SELECT 1 FROM assessment_attempts a WHERE a.enrollment_id=scoped.id");
        if let Some(t) = f.assessment_type {
            b.push(" AND a.assessment_type=").push_bind(t.as_str());
        }
        if let Some(t) = f.submitted_from {
            b.push(" AND a.submitted_at>=").push_bind(t);
        }
        if let Some(t) = f.submitted_to {
            b.push(" AND a.submitted_at<").push_bind(t);
        }
        b.push(")");
    }
    b.push(")");
    b
}
fn summary(r: PgRow) -> AppResult<StudentSummary> {
    Ok(StudentSummary {
        pre_test_score: r.try_get("pre_test_score").map_err(failure)?,
        post_test_score: r.try_get("post_test_score").map_err(failure)?,
        latest_activity_at: r.try_get("latest_activity_at").map_err(failure)?,
        student: Student {
            id: r.try_get("student_id").map_err(failure)?,
            name: r.try_get("student_name").map_err(failure)?,
        },
        enrollment: Enrollment {
            id: r.try_get("id").map_err(failure)?,
            school_id: r.try_get("school_id").map_err(failure)?,
            school_name_at_enrollment: r.try_get("school_name_at_enrollment").map_err(failure)?,
            form_date: r.try_get("form_date").map_err(failure)?,
            path_version_id: r.try_get("path_version_id").map_err(failure)?,
            created_at: r.try_get("created_at").map_err(failure)?,
            completed_at: r.try_get("completed_at").map_err(failure)?,
        },
        stage: parse_stage(r.try_get("stage").map_err(failure)?)?,
        latest_submitted_at: r.try_get("latest_submitted_at").map_err(failure)?,
    })
}
pub(super) async fn list(
    pool: &PgPool,
    admin: Uuid,
    f: Filters,
    w: Window,
) -> AppResult<Page<StudentSummary>> {
    let mut tx = read(pool).await?;
    let total = filtered(admin, &f)
        .push(" SELECT count(*) FROM visible")
        .build_query_scalar()
        .fetch_one(&mut *tx)
        .await
        .map_err(failure)?;
    let rows=filtered(admin,&f).push(" SELECT * FROM visible ORDER BY student_created_at DESC,student_id DESC,created_at DESC,id DESC LIMIT ").push_bind(w.limit).push(" OFFSET ").push_bind(w.offset).build().fetch_all(&mut *tx).await.map_err(failure)?;
    let rows = rows
        .into_iter()
        .map(summary)
        .collect::<AppResult<Vec<_>>>()?;
    tx.commit().await.map_err(failure)?;
    Ok(Page { rows, total })
}
pub(super) async fn results(
    pool: &PgPool,
    admin: Uuid,
    student: Uuid,
    enrollment: Option<Uuid>,
) -> AppResult<StudentResults> {
    let mut tx = read(pool).await?;
    let f = Filters::default();
    let mut q = filtered(admin, &f);
    q.push(" SELECT * FROM visible WHERE student_id=")
        .push_bind(student);
    if let Some(e) = enrollment {
        q.push(" AND id=").push_bind(e);
    }
    q.push(" ORDER BY created_at DESC,id DESC LIMIT 2");
    let mut rows = q.build().fetch_all(&mut *tx).await.map_err(failure)?;
    if rows.is_empty() {
        return Err(AppError::NotFound);
    }
    if rows.len() != 1 {
        return Err(AppError::Conflict("ENROLLMENT_SELECTION_REQUIRED"));
    }
    let s = summary(rows.remove(0))?;
    let eid = s.enrollment.id;
    let rows=sqlx::query("SELECT id,snapshot_id,assessment_type,attempt_number,status,created_at,submitted_at,(earned_points*10000)::bigint AS earned,(max_points*10000)::bigint AS maximum,score_percent::integer AS score FROM assessment_attempts WHERE enrollment_id=$1 ORDER BY submitted_at DESC NULLS LAST,id DESC").bind(eid).fetch_all(&mut *tx).await.map_err(failure)?;
    let mut attempts = vec![];
    for r in rows {
        attempts.push(AttemptSummary {
            id: r.try_get("id").map_err(failure)?,
            snapshot_id: r.try_get("snapshot_id").map_err(failure)?,
            assessment_type: parse_type(r.try_get("assessment_type").map_err(failure)?)?,
            attempt_number: r.try_get("attempt_number").map_err(failure)?,
            status: r.try_get("status").map_err(failure)?,
            created_at: r.try_get("created_at").map_err(failure)?,
            submitted_at: r.try_get("submitted_at").map_err(failure)?,
            earned_points: r
                .try_get::<Option<i64>, _>("earned")
                .map_err(failure)?
                .map(Points::from_units)
                .transpose()?,
            max_points: r
                .try_get::<Option<i64>, _>("maximum")
                .map_err(failure)?
                .map(Points::from_units)
                .transpose()?,
            score_percent: r.try_get("score").map_err(failure)?,
        });
    }
    let rows=sqlx::query("SELECT v.lesson_id,v.id AS lesson_version_id,v.title,i.position,p.opened_at,p.completed_at FROM learning_path_items i JOIN learning_lesson_versions v ON v.id=i.lesson_version_id LEFT JOIN learning_lesson_progress p ON p.lesson_version_id=v.id AND p.enrollment_id=$1 WHERE i.path_version_id=$2 ORDER BY i.position,v.id").bind(eid).bind(s.enrollment.path_version_id).fetch_all(&mut *tx).await.map_err(failure)?;
    let mut lessons = vec![];
    for r in rows {
        lessons.push(Lesson {
            lesson_id: r.try_get("lesson_id").map_err(failure)?,
            lesson_version_id: r.try_get("lesson_version_id").map_err(failure)?,
            title: r.try_get("title").map_err(failure)?,
            position: r.try_get("position").map_err(failure)?,
            opened_at: r.try_get("opened_at").map_err(failure)?,
            completed_at: r.try_get("completed_at").map_err(failure)?,
        });
    }
    let rows=sqlx::query("SELECT id,weight_kg::text AS weight,height_cm::text AS height,bmi::text AS bmi,calculation_rule_version,created_at FROM anthropometry_measurements WHERE enrollment_id=$1 ORDER BY created_at DESC,id DESC").bind(eid).fetch_all(&mut *tx).await.map_err(failure)?;
    let mut measurements = vec![];
    for r in rows {
        let weight = PositiveInput::parse_text(r.try_get("weight").map_err(failure)?)?;
        let height = PositiveInput::parse_text(r.try_get("height").map_err(failure)?)?;
        measurements.push(Measurement {
            id: r.try_get("id").map_err(failure)?,
            weight_kg: weight,
            height_cm: height,
            bmi: Bmi::parse_text(r.try_get("bmi").map_err(failure)?)?,
            bmi_display: Bmi::calculate(weight, height)?.display(),
            calculation_rule_version: r.try_get("calculation_rule_version").map_err(failure)?,
            created_at: r.try_get("created_at").map_err(failure)?,
        });
    }
    tx.commit().await.map_err(failure)?;
    Ok(StudentResults {
        summary: s,
        attempts,
        lessons,
        measurements,
    })
}
