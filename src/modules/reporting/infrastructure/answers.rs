//! Bounded snapshot-based answer pages, including pinned-but-not-started assessment rows.
use super::{super::application::*, read};
use crate::{
    modules::learning::admin::application::Page,
    platform::database::failure,
    shared::error::{AppError, AppResult},
};
use sqlx::{PgConnection, PgPool, Postgres, QueryBuilder, Row, postgres::PgRow};
use uuid::Uuid;
fn filtered<'a>(
    admin: Uuid,
    attempt: Option<Uuid>,
    selection: Option<&QuestionSelection>,
    f: &'a Filters,
) -> QueryBuilder<'a, Postgres> {
    let mut b = QueryBuilder::new(
        "WITH visible AS (SELECT e.student_id,s.display_name AS student_name,e.id AS enrollment_id,e.school_id,e.school_name_at_enrollment,bv.bank_id,bv.id AS bank_version_id,sn.id AS snapshot_id,sq.id AS snapshot_question_id,qv.question_id,qv.id AS question_version_id,sq.position,qv.text,qv.type AS question_type,typ.assessment_type,a.id AS attempt_id,a.attempt_number,a.status AS attempt_status,a.submitted_at,an.revision AS answer_revision,an.saved_at,an.is_correct AS correctness,(an.earned_points*10000)::bigint AS earned,(qv.max_points*10000)::bigint AS maximum FROM learning_enrollments e JOIN identity_admin_school_grants g ON g.school_id=e.school_id AND g.admin_id=",
    );
    b.push_bind(admin).push(" JOIN identity_students s ON s.id=e.student_id JOIN assessment_snapshots sn ON sn.enrollment_id=e.id JOIN assessment_bank_versions bv ON bv.id=sn.bank_version_id JOIN assessment_snapshot_questions sq ON sq.snapshot_id=sn.id JOIN assessment_question_versions qv ON qv.id=sq.question_version_id JOIN (VALUES ('pre_test','paired'),('post_test','paired'),('quiz','quiz')) AS typ(assessment_type,kind) ON typ.kind=sn.kind LEFT JOIN assessment_attempts a ON a.snapshot_id=sn.id AND a.enrollment_id=e.id AND a.assessment_type=typ.assessment_type LEFT JOIN assessment_answers an ON an.attempt_id=a.id AND an.snapshot_question_id=sq.id WHERE TRUE");
    if let Some(a) = attempt {
        b.push(" AND a.id=").push_bind(a);
    }
    if let Some(s) = selection {
        b.push(" AND qv.id=").push_bind(s.version_id);
        if let Some(q) = s.question_id {
            b.push(" AND qv.question_id=").push_bind(q);
        }
        if let Some(bank) = s.bank_id {
            b.push(" AND bv.bank_id=").push_bind(bank);
        }
    }
    if let Some(id) = f.school_id {
        b.push(" AND e.school_id=").push_bind(id);
    }
    if let Some(t) = f.assessment_type {
        b.push(" AND typ.assessment_type=").push_bind(t.as_str());
    }
    if let Some(t) = f.submitted_from {
        b.push(" AND a.submitted_at>=").push_bind(t);
    }
    if let Some(t) = f.submitted_to {
        b.push(" AND a.submitted_at<").push_bind(t);
    }
    b.push(")");
    b
}
async fn project(c: &mut PgConnection, rows: Vec<PgRow>) -> AppResult<Vec<QuestionAnswer>> {
    // Batch child reads only after the authorized page has been chosen. No per-student
    // answer mega-read and no N+1 option queries; SQL keys come solely from that page.
    let mut versions = vec![];
    let mut attempts = vec![];
    let mut questions = vec![];
    for r in &rows {
        versions.push(
            r.try_get::<Uuid, _>("question_version_id")
                .map_err(failure)?,
        );
        questions.push(
            r.try_get::<Uuid, _>("snapshot_question_id")
                .map_err(failure)?,
        );
        if let Some(a) = r
            .try_get::<Option<Uuid>, _>("attempt_id")
            .map_err(failure)?
        {
            attempts.push(a);
        }
    }
    let options:Vec<(Uuid,Uuid,String)>=sqlx::query_as("SELECT question_version_id,id,text FROM assessment_options WHERE question_version_id=ANY($1) ORDER BY question_version_id,position,id").bind(&versions).fetch_all(&mut *c).await.map_err(failure)?;
    let selected:Vec<(Uuid,Uuid,Uuid)>=sqlx::query_as("SELECT attempt_id,snapshot_question_id,option_id FROM assessment_answer_options WHERE attempt_id=ANY($1) AND snapshot_question_id=ANY($2)").bind(&attempts).bind(&questions).fetch_all(&mut *c).await.map_err(failure)?;
    let mut out = vec![];
    for r in rows {
        let qv: Uuid = r.try_get("question_version_id").map_err(failure)?;
        let attempt: Option<Uuid> = r.try_get("attempt_id").map_err(failure)?;
        let sq = r.try_get("snapshot_question_id").map_err(failure)?;
        let opts: Vec<_> = options
            .iter()
            .filter(|(v, _, _)| *v == qv)
            .map(|(_, id, text)| ReportOption {
                id: *id,
                text: text.clone(),
            })
            .collect();
        let selected_options = opts
            .iter()
            .filter(|o| {
                attempt.is_some_and(|a| {
                    selected
                        .iter()
                        .any(|(at, q, id)| *at == a && *q == sq && *id == o.id)
                })
            })
            .map(|o| ReportOption {
                id: o.id,
                text: o.text.clone(),
            })
            .collect();
        let submitted_at = r
            .try_get::<Option<chrono::DateTime<chrono::Utc>>, _>("submitted_at")
            .map_err(failure)?;
        out.push(QuestionAnswer {
            student_id: r.try_get("student_id").map_err(failure)?,
            student_name: r.try_get("student_name").map_err(failure)?,
            enrollment_id: r.try_get("enrollment_id").map_err(failure)?,
            school_id: r.try_get("school_id").map_err(failure)?,
            school_name_at_enrollment: r.try_get("school_name_at_enrollment").map_err(failure)?,
            bank_id: r.try_get("bank_id").map_err(failure)?,
            bank_version_id: r.try_get("bank_version_id").map_err(failure)?,
            snapshot_id: r.try_get("snapshot_id").map_err(failure)?,
            snapshot_question_id: sq,
            question_id: r.try_get("question_id").map_err(failure)?,
            question_version_id: qv,
            position: r.try_get("position").map_err(failure)?,
            text: r.try_get("text").map_err(failure)?,
            question_type: r.try_get("question_type").map_err(failure)?,
            assessment_type: parse_type(r.try_get("assessment_type").map_err(failure)?)?,
            attempt_id: attempt,
            attempt_number: r.try_get("attempt_number").map_err(failure)?,
            attempt_status: r.try_get("attempt_status").map_err(failure)?,
            answer_revision: r.try_get("answer_revision").map_err(failure)?,
            saved_at: r.try_get("saved_at").map_err(failure)?,
            submitted_at,
            options: opts,
            selected_options,
            correctness: if submitted_at.is_some() {
                r.try_get("correctness").map_err(failure)?
            } else {
                None
            },
            earned_points: if submitted_at.is_some() {
                r.try_get::<Option<i64>, _>("earned")
                    .map_err(failure)?
                    .map(Points::from_units)
                    .transpose()?
            } else {
                None
            },
            max_points: Points::from_units(r.try_get("maximum").map_err(failure)?)?,
        });
    }
    Ok(out)
}
async fn page(
    c: &mut PgConnection,
    admin: Uuid,
    attempt: Option<Uuid>,
    selection: Option<&QuestionSelection>,
    f: &Filters,
    w: Window,
) -> AppResult<Page<QuestionAnswer>> {
    let total = filtered(admin, attempt, selection, f)
        .push(" SELECT count(*) FROM visible")
        .build_query_scalar()
        .fetch_one(&mut *c)
        .await
        .map_err(failure)?;
    let rows=filtered(admin,attempt,selection,f).push(" SELECT * FROM visible ORDER BY submitted_at DESC NULLS LAST,attempt_id DESC NULLS LAST,enrollment_id DESC,assessment_type,position,snapshot_question_id LIMIT ").push_bind(w.limit).push(" OFFSET ").push_bind(w.offset).build().fetch_all(&mut *c).await.map_err(failure)?;
    Ok(Page {
        rows: project(c, rows).await?,
        total,
    })
}
pub(super) async fn attempt(
    pool: &PgPool,
    admin: Uuid,
    attempt: Uuid,
    w: Window,
) -> AppResult<Page<QuestionAnswer>> {
    let mut tx = read(pool).await?;
    let allowed:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM assessment_attempts a JOIN learning_enrollments e ON e.id=a.enrollment_id JOIN identity_admin_school_grants g ON g.school_id=e.school_id AND g.admin_id=$1 WHERE a.id=$2)").bind(admin).bind(attempt).fetch_one(&mut *tx).await.map_err(failure)?;
    if !allowed {
        return Err(AppError::NotFound);
    }
    let out = page(&mut tx, admin, Some(attempt), None, &Filters::default(), w).await?;
    tx.commit().await.map_err(failure)?;
    Ok(out)
}
pub(super) async fn question(
    pool: &PgPool,
    admin: Uuid,
    s: QuestionSelection,
    f: Filters,
    w: Window,
) -> AppResult<Page<QuestionAnswer>> {
    let mut tx = read(pool).await?;
    if s.bank_id.is_some() || s.question_id.is_some() {
        let valid:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM assessment_question_versions v JOIN assessment_questions q ON q.id=v.question_id WHERE v.id=$1 AND ($2::uuid IS NULL OR q.id=$2) AND ($3::uuid IS NULL OR q.bank_id=$3))").bind(s.version_id).bind(s.question_id).bind(s.bank_id).fetch_one(&mut *tx).await.map_err(failure)?;
        if !valid {
            return Err(AppError::Invalid("INVALID_QUESTION_SELECTION"));
        }
    }
    let out = page(&mut tx, admin, None, Some(&s), &f, w).await?;
    tx.commit().await.map_err(failure)?;
    Ok(out)
}
