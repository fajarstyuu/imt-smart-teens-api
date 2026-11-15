//! PostgreSQL editor persistence. Every mutation and its redacted audit commit atomically.
use super::application::*;
use crate::{
    modules::identity::application::AuditActor,
    platform::database::failure,
    shared::error::{AppError, AppResult},
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sqlx::{PgConnection, PgPool, Row};
use uuid::Uuid;
#[derive(Clone)]
pub struct PgLessonCms {
    pool: PgPool,
}
impl PgLessonCms {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}
// Versioned storage is deliberately independent of the editor HTTP DTO.
#[derive(Serialize, Deserialize)]
struct DraftV1 {
    schema: u8,
    title: String,
    description: String,
    sections: Vec<SectionV1>,
}
#[derive(Serialize, Deserialize)]
struct SectionV1 {
    id: Uuid,
    title: String,
    body: String,
    required: bool,
}
fn encode(d: &LessonDraft) -> AppResult<serde_json::Value> {
    serde_json::to_value(DraftV1 {
        schema: 1,
        title: d.title.clone(),
        description: d.description.clone(),
        sections: d
            .sections
            .iter()
            .map(|s| SectionV1 {
                id: s.id,
                title: s.title.clone(),
                body: s.body.clone(),
                required: s.required,
            })
            .collect(),
    })
    .map_err(|_| AppError::Internal)
}
fn decode(v: serde_json::Value) -> AppResult<LessonDraft> {
    let d: DraftV1 = serde_json::from_value(v).map_err(|_| AppError::Internal)?;
    if d.schema != 1 {
        return Err(AppError::Internal);
    }
    Ok(LessonDraft {
        title: d.title,
        description: d.description,
        sections: d
            .sections
            .into_iter()
            .map(|s| SectionDraft {
                id: s.id,
                title: s.title,
                body: s.body,
                required: s.required,
            })
            .collect(),
    })
}
pub(crate) async fn audit(
    c: &mut PgConnection,
    a: &AuditActor,
    action: &str,
    kind: &str,
    id: Uuid,
    rev: i64,
) -> AppResult<()> {
    sqlx::query("INSERT INTO platform_audit_events(id,actor_session_id,actor_admin_id,action,resource_type,resource_id,resource_version,request_id,safe_metadata) VALUES($1,$2,$3,$4,$5,$6,$7,$8,'{}')").bind(Uuid::new_v4()).bind(a.session_id).bind(a.admin_id).bind(action).bind(kind).bind(id).bind(rev.to_string()).bind(&a.request_id).execute(c).await.map_err(failure)?;
    Ok(())
}
pub(crate) async fn versions(c: &mut PgConnection, sql: &str, id: Uuid) -> AppResult<Vec<Version>> {
    let rows = sqlx::query(sql)
        .bind(id)
        .fetch_all(c)
        .await
        .map_err(failure)?;
    rows.into_iter()
        .map(|r| {
            Ok(Version {
                id: r.try_get("id").map_err(failure)?,
                version: r.try_get("version").map_err(failure)?,
                published_at: r.try_get("published_at").map_err(failure)?,
            })
        })
        .collect()
}
async fn lesson(c: &mut PgConnection, id: Uuid, lock: bool) -> AppResult<LessonEditor> {
    let sql = if lock {
        "SELECT * FROM learning_lessons WHERE id=$1 FOR UPDATE"
    } else {
        "SELECT * FROM learning_lessons WHERE id=$1"
    };
    let r = sqlx::query(sql)
        .bind(id)
        .fetch_optional(&mut *c)
        .await
        .map_err(failure)?
        .ok_or(AppError::NotFound)?;
    let state = ContentState::parse(r.try_get("state").map_err(failure)?)?;
    let position: Option<i32> = sqlx::query_scalar(
        "SELECT array_position(lesson_ids,$1) FROM learning_path_drafts ORDER BY id LIMIT 1",
    )
    .bind(id)
    .fetch_optional(&mut *c)
    .await
    .map_err(failure)?
    .flatten();
    Ok(LessonEditor{id,state,revision:r.try_get("revision").map_err(failure)?,position,draft:decode(r.try_get("draft_payload").map_err(failure)?)?,versions:versions(c,"SELECT id,version,published_at FROM learning_lesson_versions WHERE lesson_id=$1 ORDER BY version DESC",id).await?})
}
async fn path(c: &mut PgConnection, lock: bool) -> AppResult<PathEditor> {
    let sql = if lock {
        "SELECT * FROM learning_path_drafts ORDER BY id FOR UPDATE"
    } else {
        "SELECT * FROM learning_path_drafts ORDER BY id"
    };
    let rows = sqlx::query(sql).fetch_all(&mut *c).await.map_err(failure)?;
    if rows.len() != 1 {
        return Err(AppError::Conflict("CONTENT_CONFIGURATION_REQUIRED"));
    }
    let r = &rows[0];
    let id = r.try_get("id").map_err(failure)?;
    let vs=versions(c,"SELECT id,version,published_at FROM learning_path_versions WHERE $1::uuid IS NOT NULL ORDER BY version DESC",id).await?;
    let published = if let Some(v) = vs.first() {
        let row=sqlx::query("SELECT paired_bank_version_id,quiz_bank_version_id FROM learning_path_versions WHERE id=$1").bind(v.id).fetch_one(&mut *c).await.map_err(failure)?;
        Some(PathManifest{id:v.id,version:v.version,paired_bank_version_id:row.try_get(0).map_err(failure)?,quiz_bank_version_id:row.try_get(1).map_err(failure)?,lessons:sqlx::query_as("SELECT v.lesson_id,i.lesson_version_id,i.position FROM learning_path_items i JOIN learning_lesson_versions v ON v.id=i.lesson_version_id WHERE i.path_version_id=$1 ORDER BY i.position").bind(v.id).fetch_all(&mut *c).await.map_err(failure)?})
    } else {
        None
    };
    Ok(PathEditor {
        id,
        revision: r.try_get("revision").map_err(failure)?,
        lesson_ids: r.try_get("lesson_ids").map_err(failure)?,
        paired_bank_id: r.try_get("paired_bank_id").map_err(failure)?,
        quiz_bank_id: r.try_get("quiz_bank_id").map_err(failure)?,
        versions: vs,
        published,
    })
}
async fn save_order(c: &mut PgConnection, p: &PathEditor, ids: &[Uuid]) -> AppResult<()> {
    sqlx::query("UPDATE learning_path_drafts SET lesson_ids=$1,revision=revision+1,updated_at=now() WHERE id=$2").bind(ids).bind(p.id).execute(c).await.map_err(failure)?;
    Ok(())
}
fn editable(l: &LessonEditor, rev: i64) -> AppResult<()> {
    check_revision(l.revision, rev)?;
    if l.state == ContentState::Archived {
        return Err(AppError::Conflict("CONTENT_ARCHIVED"));
    }
    Ok(())
}
#[async_trait]
impl LessonStore for PgLessonCms {
    async fn initialize_empty(&self) -> AppResult<()> {
        let mut tx = self.pool.begin().await.map_err(failure)?;
        // Serialize explicit concurrent initializers; never replace roots or draft membership.
        sqlx::query("SELECT pg_advisory_xact_lock(718304291)")
            .execute(&mut *tx)
            .await
            .map_err(failure)?;
        for kind in ["paired", "quiz"] {
            sqlx::query("INSERT INTO assessment_banks(id,kind,state) VALUES($1,$2,'draft') ON CONFLICT(kind) DO NOTHING").bind(Uuid::new_v4()).bind(kind).execute(&mut *tx).await.map_err(failure)?;
        }
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM learning_path_drafts")
            .fetch_one(&mut *tx)
            .await
            .map_err(failure)?;
        if count > 1 {
            return Err(AppError::Conflict("CONTENT_CONFIGURATION_REQUIRED"));
        }
        if count == 0 {
            sqlx::query("INSERT INTO learning_path_drafts(id,lesson_ids,paired_bank_id,quiz_bank_id) SELECT $1,'{}',p.id,q.id FROM assessment_banks p,assessment_banks q WHERE p.kind='paired' AND q.kind='quiz'").bind(Uuid::new_v4()).execute(&mut *tx).await.map_err(failure)?;
        }
        tx.commit().await.map_err(failure)
    }
    async fn list(&self, q: ContentQuery) -> AppResult<Page<LessonEditor>> {
        let mut tx = self.pool.begin().await.map_err(failure)?;
        sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
            .execute(&mut *tx)
            .await
            .map_err(failure)?;
        let status = q.status.map(ContentState::as_str);
        let total = sqlx::query_scalar(
            "SELECT count(*) FROM learning_lessons WHERE ($1::text IS NULL OR state=$1)",
        )
        .bind(status)
        .fetch_one(&mut *tx)
        .await
        .map_err(failure)?;
        let ids:Vec<Uuid>=sqlx::query_scalar("SELECT l.id FROM learning_lessons l LEFT JOIN learning_path_drafts p ON l.id=ANY(p.lesson_ids) WHERE ($1::text IS NULL OR l.state=$1) ORDER BY array_position(p.lesson_ids,l.id) NULLS LAST,l.id LIMIT $2 OFFSET $3").bind(status).bind(q.limit).bind(q.offset).fetch_all(&mut *tx).await.map_err(failure)?;
        let mut rows = Vec::new();
        for id in ids {
            rows.push(lesson(&mut tx, id, false).await?);
        }
        tx.commit().await.map_err(failure)?;
        Ok(Page { rows, total })
    }
    async fn get(&self, id: Uuid) -> AppResult<LessonEditor> {
        let mut tx = self.pool.begin().await.map_err(failure)?;
        let out = lesson(&mut tx, id, false).await?;
        tx.commit().await.map_err(failure)?;
        Ok(out)
    }
    async fn create(
        &self,
        draft: LessonDraft,
        position: i32,
        a: AuditActor,
    ) -> AppResult<LessonEditor> {
        let mut tx = self.pool.begin().await.map_err(failure)?;
        let p = path(&mut tx, true).await?;
        // Creation appends only; moving existing membership requires the revisioned order endpoint.
        if position as usize != p.lesson_ids.len() + 1 {
            return Err(AppError::Conflict("USE_PATH_ORDER"));
        }
        let id = Uuid::new_v4();
        sqlx::query("INSERT INTO learning_lessons(id,state,draft_payload) VALUES($1,'draft',$2)")
            .bind(id)
            .bind(encode(&draft)?)
            .execute(&mut *tx)
            .await
            .map_err(failure)?;
        let mut ids = p.lesson_ids.clone();
        ids.push(id);
        save_order(&mut tx, &p, &ids).await?;
        audit(&mut tx, &a, "lesson.create", "lesson", id, 1).await?;
        let out = lesson(&mut tx, id, false).await?;
        tx.commit().await.map_err(failure)?;
        Ok(out)
    }
    async fn update(
        &self,
        id: Uuid,
        draft: LessonDraft,
        position: i32,
        rev: i64,
        a: AuditActor,
    ) -> AppResult<LessonEditor> {
        let mut tx = self.pool.begin().await.map_err(failure)?;
        let _p = path(&mut tx, true).await?;
        let l = lesson(&mut tx, id, true).await?;
        editable(&l, rev)?;
        if l.position != Some(position) {
            return Err(AppError::Conflict("USE_PATH_ORDER"));
        }
        sqlx::query("UPDATE learning_lessons SET draft_payload=$1,state='draft',revision=revision+1,updated_at=now() WHERE id=$2").bind(encode(&draft)?).bind(id).execute(&mut *tx).await.map_err(failure)?;
        audit(&mut tx, &a, "lesson.update", "lesson", id, rev + 1).await?;
        let out = lesson(&mut tx, id, false).await?;
        tx.commit().await.map_err(failure)?;
        Ok(out)
    }
    async fn publish(&self, id: Uuid, rev: i64, a: AuditActor) -> AppResult<LessonEditor> {
        let mut tx = self.pool.begin().await.map_err(failure)?;
        let _p = path(&mut tx, true).await?;
        let l = lesson(&mut tx, id, true).await?;
        editable(&l, rev)?;
        l.draft.validate(true)?;
        let v = Uuid::new_v4();
        let number = l.versions.first().map_or(1, |v| v.version + 1);
        sqlx::query("INSERT INTO learning_lesson_versions(id,lesson_id,version,title,description) VALUES($1,$2,$3,$4,$5)").bind(v).bind(id).bind(number).bind(&l.draft.title).bind(&l.draft.description).execute(&mut *tx).await.map_err(failure)?;
        for (i, s) in l.draft.sections.iter().enumerate() {
            sqlx::query("INSERT INTO learning_sections(id,lesson_version_id,section_key,position,title,body,required) VALUES($1,$2,$3,$4,$5,$6,$7)").bind(Uuid::new_v4()).bind(v).bind(s.id).bind(i as i32+1).bind(&s.title).bind(&s.body).bind(s.required).execute(&mut *tx).await.map_err(failure)?;
        }
        sqlx::query("UPDATE learning_lessons SET state='published',revision=revision+1,updated_at=now() WHERE id=$1").bind(id).execute(&mut *tx).await.map_err(failure)?;
        audit(&mut tx, &a, "lesson.publish", "lesson", id, rev + 1).await?;
        let out = lesson(&mut tx, id, false).await?;
        tx.commit().await.map_err(failure)?;
        Ok(out)
    }
    async fn archive(&self, id: Uuid, rev: i64, a: AuditActor) -> AppResult<LessonEditor> {
        let mut tx = self.pool.begin().await.map_err(failure)?;
        let p = path(&mut tx, true).await?;
        let l = lesson(&mut tx, id, true).await?;
        editable(&l, rev)?;
        sqlx::query("UPDATE learning_lessons SET state='archived',revision=revision+1,updated_at=now() WHERE id=$1").bind(id).execute(&mut *tx).await.map_err(failure)?;
        let ids: Vec<_> = p.lesson_ids.iter().copied().filter(|x| *x != id).collect();
        save_order(&mut tx, &p, &ids).await?;
        audit(&mut tx, &a, "lesson.archive", "lesson", id, rev + 1).await?;
        let out = lesson(&mut tx, id, false).await?;
        tx.commit().await.map_err(failure)?;
        Ok(out)
    }
    async fn publish_path(&self, rev: i64, a: AuditActor) -> AppResult<PathEditor> {
        let mut tx = self.pool.begin().await.map_err(failure)?;
        let p = path(&mut tx, true).await?;
        check_revision(p.revision, rev)?;
        if p.lesson_ids.is_empty() {
            return Err(AppError::Invalid("EMPTY_LEARNING_PATH"));
        }
        let unique: std::collections::HashSet<_> = p.lesson_ids.iter().collect();
        if unique.len() != p.lesson_ids.len() {
            return Err(AppError::Invalid("INVALID_PATH_ORDER"));
        }
        // Global CMS lock order: path -> banks sorted by UUID -> lesson roots sorted by UUID.
        let mut banks = [p.paired_bank_id, p.quiz_bank_id];
        banks.sort();
        for bank in banks {
            sqlx::query("SELECT id FROM assessment_banks WHERE id=$1 FOR UPDATE")
                .bind(bank)
                .execute(&mut *tx)
                .await
                .map_err(failure)?;
        }
        let mut bank_versions = vec![];
        for (bank, kind) in [(p.paired_bank_id, "paired"), (p.quiz_bank_id, "quiz")] {
            let version:Uuid=sqlx::query_scalar("SELECT v.id FROM assessment_bank_versions v JOIN assessment_banks b ON b.id=v.bank_id WHERE b.id=$1 AND b.kind=$2 AND b.state<>'archived' ORDER BY v.version DESC LIMIT 1").bind(bank).bind(kind).fetch_optional(&mut *tx).await.map_err(failure)?.ok_or(AppError::Conflict("CONTENT_NOT_AVAILABLE"))?;
            crate::modules::assessment::admin::infrastructure::validate_published_bank(
                &mut tx, version, bank,
            )
            .await?;
            bank_versions.push(version);
        }
        let mut sorted = p.lesson_ids.clone();
        sorted.sort();
        for id in sorted {
            sqlx::query("SELECT id FROM learning_lessons WHERE id=$1 FOR UPDATE")
                .bind(id)
                .execute(&mut *tx)
                .await
                .map_err(failure)?;
        }
        let mut items = vec![];
        for id in &p.lesson_ids {
            let row=sqlx::query("SELECT v.id,v.title,v.description FROM learning_lesson_versions v JOIN learning_lessons l ON l.id=v.lesson_id WHERE l.id=$1 AND l.state<>'archived' ORDER BY v.version DESC LIMIT 1").bind(id).fetch_optional(&mut *tx).await.map_err(failure)?.ok_or(AppError::Conflict("CONTENT_NOT_AVAILABLE"))?;
            let version: Uuid = row.try_get("id").map_err(failure)?;
            let rows=sqlx::query("SELECT section_key,title,body,required FROM learning_sections WHERE lesson_version_id=$1 ORDER BY position").bind(version).fetch_all(&mut *tx).await.map_err(failure)?;
            let mut sections = vec![];
            for r in rows {
                sections.push(SectionDraft {
                    id: r.try_get("section_key").map_err(failure)?,
                    title: r.try_get("title").map_err(failure)?,
                    body: r.try_get("body").map_err(failure)?,
                    required: r.try_get("required").map_err(failure)?,
                });
            }
            LessonDraft {
                title: row.try_get("title").map_err(failure)?,
                description: row.try_get("description").map_err(failure)?,
                sections,
            }
            .validate(true)?;
            items.push(version);
        }
        let id = Uuid::new_v4();
        let number = p.versions.first().map_or(1, |v| v.version + 1);
        sqlx::query("INSERT INTO learning_path_versions(id,version,paired_bank_version_id,quiz_bank_version_id) VALUES($1,$2,$3,$4)").bind(id).bind(number).bind(bank_versions[0]).bind(bank_versions[1]).execute(&mut *tx).await.map_err(failure)?;
        for (i, v) in items.iter().enumerate() {
            sqlx::query("INSERT INTO learning_path_items(path_version_id,lesson_version_id,position) VALUES($1,$2,$3)").bind(id).bind(v).bind(i as i32+1).execute(&mut *tx).await.map_err(failure)?;
        }
        sqlx::query(
            "UPDATE learning_path_drafts SET revision=revision+1,updated_at=now() WHERE id=$1",
        )
        .bind(p.id)
        .execute(&mut *tx)
        .await
        .map_err(failure)?;
        audit(&mut tx, &a, "path.publish", "path", p.id, rev + 1).await?;
        let out = path(&mut tx, false).await?;
        tx.commit().await.map_err(failure)?;
        Ok(out)
    }
    async fn path(&self) -> AppResult<PathEditor> {
        let mut tx = self.pool.begin().await.map_err(failure)?;
        let out = path(&mut tx, false).await?;
        tx.commit().await.map_err(failure)?;
        Ok(out)
    }
    async fn order(&self, ids: Vec<Uuid>, rev: i64, a: AuditActor) -> AppResult<PathEditor> {
        let mut tx = self.pool.begin().await.map_err(failure)?;
        let p = path(&mut tx, true).await?;
        check_revision(p.revision, rev)?;
        let mut old = p.lesson_ids.clone();
        let mut new = ids.clone();
        old.sort();
        new.sort();
        if old != new {
            return Err(AppError::Invalid("INVALID_PATH_ORDER"));
        }
        save_order(&mut tx, &p, &ids).await?;
        audit(&mut tx, &a, "path.order", "path", p.id, rev + 1).await?;
        let out = path(&mut tx, false).await?;
        tx.commit().await.map_err(failure)?;
        Ok(out)
    }
}
