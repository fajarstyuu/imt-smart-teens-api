//! Assessment SQL/lock mapping used by the shared publication transaction adapter.
//! This adapter borrows the caller's transaction connection; it cannot commit or start one.
use super::super::application::*;
use crate::{
    platform::database::failure,
    shared::{editing::ContentState, error::AppResult},
};
use async_trait::async_trait;
use sqlx::{PgConnection, Row};
use uuid::Uuid;

pub(crate) struct PgPublishedBankReader<'a> {
    connection: &'a mut PgConnection,
}
impl<'a> PgPublishedBankReader<'a> {
    pub(crate) fn new(connection: &'a mut PgConnection) -> Self {
        Self { connection }
    }
}
#[async_trait]
impl PublishedBankReader for PgPublishedBankReader<'_> {
    async fn lock_bank_roots(&mut self, sorted_ids: &[Uuid]) -> AppResult<()> {
        for id in sorted_ids {
            sqlx::query("SELECT id FROM assessment_banks WHERE id=$1 FOR UPDATE")
                .bind(id)
                .execute(&mut *self.connection)
                .await
                .map_err(failure)?;
        }
        Ok(())
    }
    async fn published_bank(&mut self, id: Uuid) -> AppResult<Option<PublishedBank>> {
        let c = &mut *self.connection;
        // Return raw availability/archive facts rather than filtering policy in SQL.
        let Some(root) = sqlx::query("SELECT b.id,b.kind,b.state,v.id AS version_id,v.bank_id AS version_bank_id FROM assessment_banks b LEFT JOIN LATERAL (SELECT id,bank_id FROM assessment_bank_versions WHERE bank_id=b.id ORDER BY version DESC LIMIT 1) v ON true WHERE b.id=$1")
            .bind(id).fetch_optional(&mut *c).await.map_err(failure)? else { return Ok(None); };
        let version_id: Option<Uuid> = root.try_get("version_id").map_err(failure)?;
        let rows=sqlx::query("SELECT v.id,v.text,v.type,v.required,v.scoring_rule,(v.max_points*10000)::bigint AS units,q.bank_id,q.state,i.position FROM assessment_bank_items i JOIN assessment_question_versions v ON v.id=i.question_version_id JOIN assessment_questions q ON q.id=v.question_id WHERE i.bank_version_id=$1 ORDER BY i.position").bind(version_id).fetch_all(&mut *c).await.map_err(failure)?;
        let mut questions = vec![];
        for r in rows {
            let version: Uuid = r.try_get("id").map_err(failure)?;
            let options=sqlx::query("SELECT id,text,is_correct FROM assessment_options WHERE question_version_id=$1 ORDER BY position").bind(version).fetch_all(&mut *c).await.map_err(failure)?;
            let mut opts = vec![];
            let mut keys = vec![];
            for o in options {
                let oid = o.try_get("id").map_err(failure)?;
                opts.push(OptionDraft {
                    id: oid,
                    text: o.try_get("text").map_err(failure)?,
                });
                if o.try_get::<bool, _>("is_correct").map_err(failure)? {
                    keys.push(oid);
                }
            }
            questions.push(PublishedQuestion {
                bank_id: r.try_get("bank_id").map_err(failure)?,
                state: ContentState::parse(r.try_get("state").map_err(failure)?)?,
                position: r.try_get("position").map_err(failure)?,
                scoring_rule: r.try_get("scoring_rule").map_err(failure)?,
                max_points_units: r.try_get("units").map_err(failure)?,
                draft: QuestionDraft {
                    text: r.try_get("text").map_err(failure)?,
                    question_type: QuestionType::parse(r.try_get("type").map_err(failure)?)?,
                    required: r.try_get("required").map_err(failure)?,
                    options: opts,
                    correct_option_ids: keys,
                },
            });
        }
        Ok(Some(PublishedBank {
            id: root.try_get("id").map_err(failure)?,
            kind: BankKind::parse(root.try_get("kind").map_err(failure)?)?,
            state: ContentState::parse(root.try_get("state").map_err(failure)?)?,
            version_id,
            version_bank_id: root.try_get("version_bank_id").map_err(failure)?,
            questions,
        }))
    }
}
