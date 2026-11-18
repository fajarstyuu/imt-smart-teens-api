//! Bank-root then question locks serialize publication with every edit; histories are append-only.
mod published;
use super::application::*;
use crate::{
    modules::{assessment::domain::validate_score_budget, identity::application::AuditActor},
    platform::{
        database::failure,
        editor::{audit, versions},
    },
    shared::editing::{ContentQuery, ContentState, Page, check_revision},
    shared::error::{AppError, AppResult},
};
use async_trait::async_trait;
pub(crate) use published::PgPublishedBankReader;
use serde::{Deserialize, Serialize};
use sqlx::{PgConnection, PgPool, Row};
use std::collections::HashSet;
use uuid::Uuid;
#[derive(Clone)]
pub struct PgBankCms {
    pool: PgPool,
}
impl PgBankCms {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}
#[derive(Serialize, Deserialize)]
struct DraftV1 {
    schema: u8,
    text: String,
    question_type: String,
    options: Vec<OptionV1>,
    correct_option_ids: Vec<Uuid>,
    required: bool,
}
#[derive(Serialize, Deserialize)]
struct OptionV1 {
    id: Uuid,
    text: String,
}
fn encode(d: &QuestionDraft) -> AppResult<serde_json::Value> {
    serde_json::to_value(DraftV1 {
        schema: 1,
        text: d.text.clone(),
        question_type: d.question_type.as_str().into(),
        options: d
            .options
            .iter()
            .map(|o| OptionV1 {
                id: o.id,
                text: o.text.clone(),
            })
            .collect(),
        correct_option_ids: d.correct_option_ids.clone(),
        required: d.required,
    })
    .map_err(|_| AppError::Internal)
}
fn decode(v: serde_json::Value) -> AppResult<QuestionDraft> {
    let d: DraftV1 =
        serde_json::from_value(v).map_err(|_| AppError::Invalid("INVALID_QUESTION_DRAFT"))?;
    if d.schema != 1 {
        return Err(AppError::Invalid("INVALID_QUESTION_DRAFT"));
    }
    Ok(QuestionDraft {
        text: d.text,
        question_type: QuestionType::parse(&d.question_type)?,
        options: d
            .options
            .into_iter()
            .map(|o| OptionDraft {
                id: o.id,
                text: o.text,
            })
            .collect(),
        correct_option_ids: d.correct_option_ids,
        required: d.required,
    })
}
async fn bank(c: &mut PgConnection, id: Uuid, lock: bool) -> AppResult<BankEditor> {
    let r = sqlx::query(if lock {
        "SELECT * FROM assessment_banks WHERE id=$1 FOR UPDATE"
    } else {
        "SELECT * FROM assessment_banks WHERE id=$1"
    })
    .bind(id)
    .fetch_optional(&mut *c)
    .await
    .map_err(failure)?
    .ok_or(AppError::NotFound)?;
    Ok(BankEditor{id,kind:r.try_get("kind").map_err(failure)?,state:ContentState::parse(r.try_get("state").map_err(failure)?)?,revision:r.try_get("revision").map_err(failure)?,versions:versions(c,"SELECT id,version,published_at FROM assessment_bank_versions WHERE bank_id=$1 ORDER BY version DESC",id).await?})
}
async fn question(
    c: &mut PgConnection,
    bank: Uuid,
    id: Uuid,
    lock: bool,
) -> AppResult<QuestionEditor> {
    let r = sqlx::query(if lock {
        "SELECT * FROM assessment_questions WHERE bank_id=$1 AND id=$2 FOR UPDATE"
    } else {
        "SELECT * FROM assessment_questions WHERE bank_id=$1 AND id=$2"
    })
    .bind(bank)
    .bind(id)
    .fetch_optional(&mut *c)
    .await
    .map_err(failure)?
    .ok_or(AppError::NotFound)?;
    Ok(QuestionEditor{id,bank_id:bank,state:ContentState::parse(r.try_get("state").map_err(failure)?)?,revision:r.try_get("revision").map_err(failure)?,position:r.try_get("position").map_err(failure)?,draft:decode(r.try_get("draft_payload").map_err(failure)?)?,versions:versions(c,"SELECT v.id,v.version,min(b.published_at) AS published_at FROM assessment_question_versions v JOIN assessment_bank_items i ON i.question_version_id=v.id JOIN assessment_bank_versions b ON b.id=i.bank_version_id WHERE v.question_id=$1 GROUP BY v.id,v.version ORDER BY v.version DESC",id).await?})
}
async fn dirty(c: &mut PgConnection, bank: Uuid) -> AppResult<()> {
    sqlx::query("UPDATE assessment_banks SET revision=revision+1,state='draft' WHERE id=$1")
        .bind(bank)
        .execute(c)
        .await
        .map_err(failure)?;
    Ok(())
}
fn editable(q: &QuestionEditor, rev: i64) -> AppResult<()> {
    check_revision(q.revision, rev)?;
    if q.state == ContentState::Archived {
        return Err(AppError::Conflict("CONTENT_ARCHIVED"));
    }
    Ok(())
}
#[async_trait]
impl BankStore for PgBankCms {
    async fn banks(&self) -> AppResult<Vec<BankEditor>> {
        let mut tx = self.pool.begin().await.map_err(failure)?;
        let ids: Vec<Uuid> =
            sqlx::query_scalar("SELECT id FROM assessment_banks WHERE kind='paired' ORDER BY id")
                .fetch_all(&mut *tx)
                .await
                .map_err(failure)?;
        if ids.len() != 1 {
            return Err(AppError::Conflict("CONTENT_CONFIGURATION_REQUIRED"));
        }
        let mut out = vec![];
        for id in ids {
            out.push(bank(&mut tx, id, false).await?);
        }
        tx.commit().await.map_err(failure)?;
        Ok(out)
    }
    async fn questions(&self, id: Uuid, q: ContentQuery) -> AppResult<Page<QuestionEditor>> {
        let mut tx = self.pool.begin().await.map_err(failure)?;
        sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
            .execute(&mut *tx)
            .await
            .map_err(failure)?;
        bank(&mut tx, id, false).await?;
        let status = q.status.map(ContentState::as_str);
        let total=sqlx::query_scalar("SELECT count(*) FROM assessment_questions WHERE bank_id=$1 AND ($2::text IS NULL OR state=$2)").bind(id).bind(status).fetch_one(&mut *tx).await.map_err(failure)?;
        let records=sqlx::query("SELECT * FROM assessment_questions WHERE bank_id=$1 AND ($2::text IS NULL OR state=$2) ORDER BY position,id LIMIT $3 OFFSET $4").bind(id).bind(status).bind(q.limit).bind(q.offset).fetch_all(&mut *tx).await.map_err(failure)?;
        let ids = records
            .iter()
            .map(|r| r.try_get::<Uuid, _>("id").map_err(failure))
            .collect::<AppResult<Vec<_>>>()?;
        let history=sqlx::query("SELECT v.question_id,v.id,v.version,min(b.published_at) AS published_at FROM assessment_question_versions v JOIN assessment_bank_items i ON i.question_version_id=v.id JOIN assessment_bank_versions b ON b.id=i.bank_version_id WHERE v.question_id=ANY($1) GROUP BY v.question_id,v.id,v.version ORDER BY v.question_id,v.version DESC").bind(&ids).fetch_all(&mut *tx).await.map_err(failure)?;
        let mut histories =
            std::collections::HashMap::<Uuid, Vec<crate::shared::editing::Version>>::new();
        for r in history {
            histories
                .entry(r.try_get("question_id").map_err(failure)?)
                .or_default()
                .push(crate::shared::editing::Version {
                    id: r.try_get("id").map_err(failure)?,
                    version: r.try_get("version").map_err(failure)?,
                    published_at: r.try_get("published_at").map_err(failure)?,
                });
        }
        let mut rows = vec![];
        for r in records {
            let qid = r.try_get("id").map_err(failure)?;
            rows.push(QuestionEditor {
                id: qid,
                bank_id: id,
                state: ContentState::parse(r.try_get("state").map_err(failure)?)?,
                revision: r.try_get("revision").map_err(failure)?,
                position: r.try_get("position").map_err(failure)?,
                draft: decode(r.try_get("draft_payload").map_err(failure)?)?,
                versions: histories.remove(&qid).unwrap_or_default(),
            });
        }
        tx.commit().await.map_err(failure)?;
        Ok(Page { rows, total })
    }
    async fn question(&self, bank: Uuid, id: Uuid) -> AppResult<QuestionEditor> {
        let mut tx = self.pool.begin().await.map_err(failure)?;
        let out = question(&mut tx, bank, id, false).await?;
        tx.commit().await.map_err(failure)?;
        Ok(out)
    }
    async fn create(
        &self,
        id: Uuid,
        d: QuestionDraft,
        position: i32,
        a: AuditActor,
    ) -> AppResult<QuestionEditor> {
        let mut tx = self.pool.begin().await.map_err(failure)?;
        bank(&mut tx, id, true).await?;
        let q = Uuid::new_v4();
        sqlx::query("INSERT INTO assessment_questions(id,bank_id,state,draft_payload,position) VALUES($1,$2,'draft',$3,$4)").bind(q).bind(id).bind(encode(&d)?).bind(position).execute(&mut *tx).await.map_err(failure)?;
        dirty(&mut tx, id).await?;
        audit(&mut tx, &a, "question.create", "question", q, 1).await?;
        let out = question(&mut tx, id, q, false).await?;
        tx.commit().await.map_err(failure)?;
        Ok(out)
    }
    async fn update(
        &self,
        bank_id: Uuid,
        id: Uuid,
        d: QuestionDraft,
        position: i32,
        rev: i64,
        a: AuditActor,
    ) -> AppResult<QuestionEditor> {
        let mut tx = self.pool.begin().await.map_err(failure)?;
        bank(&mut tx, bank_id, true).await?;
        let q = question(&mut tx, bank_id, id, true).await?;
        editable(&q, rev)?;
        sqlx::query("UPDATE assessment_questions SET draft_payload=$1,position=$2,state='draft',revision=revision+1 WHERE id=$3").bind(encode(&d)?).bind(position).bind(id).execute(&mut *tx).await.map_err(failure)?;
        dirty(&mut tx, bank_id).await?;
        audit(&mut tx, &a, "question.update", "question", id, rev + 1).await?;
        let out = question(&mut tx, bank_id, id, false).await?;
        tx.commit().await.map_err(failure)?;
        Ok(out)
    }
    async fn archive(
        &self,
        bank_id: Uuid,
        id: Uuid,
        rev: i64,
        a: AuditActor,
    ) -> AppResult<QuestionEditor> {
        let mut tx = self.pool.begin().await.map_err(failure)?;
        bank(&mut tx, bank_id, true).await?;
        let q = question(&mut tx, bank_id, id, true).await?;
        editable(&q, rev)?;
        sqlx::query(
            "UPDATE assessment_questions SET state='archived',revision=revision+1 WHERE id=$1",
        )
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(failure)?;
        dirty(&mut tx, bank_id).await?;
        audit(&mut tx, &a, "question.archive", "question", id, rev + 1).await?;
        let out = question(&mut tx, bank_id, id, false).await?;
        tx.commit().await.map_err(failure)?;
        Ok(out)
    }
    async fn publish(&self, bank_id: Uuid, rev: i64, a: AuditActor) -> AppResult<BankEditor> {
        let mut tx = self.pool.begin().await.map_err(failure)?;
        let out = publish_in_connection(&mut tx, bank_id, rev, a).await?;
        tx.commit().await.map_err(failure)?;
        Ok(out)
    }
}
/// Cross-context composition borrows the caller's transaction; this writer never commits.
pub(crate) async fn publish_in_connection(
    c: &mut PgConnection,
    bank_id: Uuid,
    rev: i64,
    a: AuditActor,
) -> AppResult<BankEditor> {
    let b = bank(&mut *c, bank_id, true).await?;
    check_revision(b.revision, rev)?;
    let ids:Vec<Uuid>=sqlx::query_scalar("SELECT id FROM assessment_questions WHERE bank_id=$1 AND state<>'archived' ORDER BY position,id FOR UPDATE").bind(bank_id).fetch_all(&mut *c).await.map_err(failure)?;
    if ids.is_empty() {
        return Err(AppError::Invalid("EMPTY_QUESTION_BANK"));
    }
    let mut questions = vec![];
    let mut positions = HashSet::new();
    for id in ids {
        let q = question(&mut *c, bank_id, id, false).await?;
        q.draft.validate()?;
        if q.position < 1 || !positions.insert(q.position) {
            return Err(AppError::Invalid("INVALID_QUESTION_ORDER"));
        }
        questions.push(q);
    }
    validate_score_budget(questions.iter().map(|_| 10000))?;
    // Construct each complete question before referencing it from bank items. Draft IDs map
    // to fresh immutable row IDs on every publication; private keys follow that mapping.
    let mut items = vec![];
    for q in questions {
        let version = Uuid::new_v4();
        let number = q.versions.first().map_or(1, |v| v.version + 1);
        sqlx::query("INSERT INTO assessment_question_versions(id,question_id,version,text,type,required,scoring_rule,max_points) VALUES($1,$2,$3,$4,$5,$6,'exact_set_v1',1)").bind(version).bind(q.id).bind(number).bind(&q.draft.text).bind(q.draft.question_type.as_str()).bind(q.draft.required).execute(&mut *c).await.map_err(failure)?;
        for (i, o) in q.draft.options.iter().enumerate() {
            sqlx::query("INSERT INTO assessment_options(id,question_version_id,position,text,is_correct) VALUES($1,$2,$3,$4,$5)").bind(Uuid::new_v4()).bind(version).bind(i as i32+1).bind(&o.text).bind(q.draft.correct_option_ids.contains(&o.id)).execute(&mut *c).await.map_err(failure)?;
        }
        sqlx::query(
            "UPDATE assessment_questions SET state='published',revision=revision+1 WHERE id=$1",
        )
        .bind(q.id)
        .execute(&mut *c)
        .await
        .map_err(failure)?;
        items.push((version, q.position));
    }
    let version = Uuid::new_v4();
    let number = b.versions.first().map_or(1, |v| v.version + 1);
    sqlx::query("INSERT INTO assessment_bank_versions(id,bank_id,version) VALUES($1,$2,$3)")
        .bind(version)
        .bind(bank_id)
        .bind(number)
        .execute(&mut *c)
        .await
        .map_err(failure)?;
    for (q, pos) in items {
        sqlx::query("INSERT INTO assessment_bank_items(bank_version_id,question_version_id,position) VALUES($1,$2,$3)").bind(version).bind(q).bind(pos).execute(&mut *c).await.map_err(failure)?;
    }
    sqlx::query("UPDATE assessment_banks SET revision=revision+1,state='published' WHERE id=$1")
        .bind(bank_id)
        .execute(&mut *c)
        .await
        .map_err(failure)?;
    audit(&mut *c, &a, "bank.publish", "bank", bank_id, rev + 1).await?;
    let out = bank(&mut *c, bank_id, false).await?;
    Ok(out)
}

/// Read complete active drafts while the caller holds the bank root lock.
pub(crate) async fn release_drafts(
    c: &mut PgConnection,
    id: Uuid,
) -> AppResult<(BankEditor, Vec<QuestionEditor>)> {
    let b = bank(c, id, false).await?;
    let ids:Vec<Uuid>=sqlx::query_scalar("SELECT id FROM assessment_questions WHERE bank_id=$1 AND state<>'archived' ORDER BY position,id").bind(id).fetch_all(&mut *c).await.map_err(failure)?;
    let mut questions = Vec::with_capacity(ids.len());
    for q in ids {
        questions.push(question(c, id, q, false).await?);
    }
    Ok((b, questions))
}
