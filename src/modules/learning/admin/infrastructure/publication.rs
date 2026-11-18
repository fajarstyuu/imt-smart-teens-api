//! Concrete transaction composition for path publication. Assessment owns its SQL and
//! returns typed facts through its application port; both adapters borrow this ONE tx.
use super::*;
use crate::modules::assessment::admin::{
    application::{PublishedBank, PublishedBankReader},
    infrastructure::PgPublishedBankReader,
};
use sqlx::{Postgres, Transaction};

pub(super) struct PgPathPublication {
    tx: Transaction<'static, Postgres>,
}
impl PgPathPublication {
    pub(super) fn new(tx: Transaction<'static, Postgres>) -> Self {
        Self { tx }
    }
}
#[async_trait]
impl PublishedBankReader for PgPathPublication {
    async fn lock_bank_roots(&mut self, sorted_ids: &[Uuid]) -> AppResult<()> {
        PgPublishedBankReader::new(&mut self.tx)
            .lock_bank_roots(sorted_ids)
            .await
    }
    async fn published_bank(&mut self, id: Uuid) -> AppResult<Option<PublishedBank>> {
        PgPublishedBankReader::new(&mut self.tx)
            .published_bank(id)
            .await
    }
}
#[async_trait]
impl PathPublicationTransaction for PgPathPublication {
    async fn lesson_draft(&mut self, id: Uuid) -> AppResult<LessonEditor> {
        lesson(&mut self.tx, id, false).await
    }
    async fn bank_drafts(
        &mut self,
        id: Uuid,
    ) -> AppResult<(
        crate::modules::assessment::admin::application::BankEditor,
        Vec<crate::modules::assessment::admin::application::QuestionEditor>,
    )> {
        crate::modules::assessment::admin::infrastructure::release_drafts(&mut self.tx, id).await
    }
    async fn publish_draft_lesson(
        &mut self,
        id: Uuid,
        revision: i64,
        actor: AuditActor,
    ) -> AppResult<()> {
        super::publish_in_connection(&mut self.tx, id, revision, actor).await?;
        Ok(())
    }
    async fn publish_draft_bank(
        &mut self,
        id: Uuid,
        revision: i64,
        actor: AuditActor,
    ) -> AppResult<()> {
        crate::modules::assessment::admin::infrastructure::publish_in_connection(
            &mut self.tx,
            id,
            revision,
            actor,
        )
        .await?;
        Ok(())
    }
    async fn lock_path(&mut self) -> AppResult<PathEditor> {
        path(&mut self.tx, true).await
    }
    async fn lock_lesson_roots(&mut self, sorted_ids: &[Uuid]) -> AppResult<()> {
        for id in sorted_ids {
            sqlx::query("SELECT id FROM learning_lessons WHERE id=$1 FOR UPDATE")
                .bind(id)
                .execute(&mut *self.tx)
                .await
                .map_err(failure)?;
        }
        Ok(())
    }
    async fn published_lesson(&mut self, id: Uuid) -> AppResult<Option<PublishedLesson>> {
        let c = &mut *self.tx;
        let Some(row) = sqlx::query("SELECT l.state,v.id,v.title,v.description,v.quiz_payload FROM learning_lessons l LEFT JOIN LATERAL (SELECT id,title,description,quiz_payload FROM learning_lesson_versions WHERE lesson_id=l.id ORDER BY version DESC LIMIT 1) v ON true WHERE l.id=$1")
            .bind(id).fetch_optional(&mut *c).await.map_err(failure)? else { return Ok(None); };
        let version: Option<Uuid> = row.try_get("id").map_err(failure)?;
        let rows=sqlx::query("SELECT section_key,title,body,required FROM learning_sections WHERE lesson_version_id=$1 ORDER BY position").bind(version).fetch_all(&mut *c).await.map_err(failure)?;
        let mut sections = vec![];
        for r in rows {
            sections.push(SectionDraft {
                id: r.try_get("section_key").map_err(failure)?,
                title: r.try_get("title").map_err(failure)?,
                body: r.try_get("body").map_err(failure)?,
                required: r.try_get("required").map_err(failure)?,
            });
        }
        Ok(Some(PublishedLesson {
            lesson_id: id,
            state: ContentState::parse(row.try_get("state").map_err(failure)?)?,
            version: version
                .map(|id| -> AppResult<_> {
                    Ok(PublishedLessonVersion {
                        id,
                        draft: LessonDraft {
                            title: row.try_get("title").map_err(failure)?,
                            description: row.try_get("description").map_err(failure)?,
                            sections,
                            quiz: row
                                .try_get::<Option<serde_json::Value>, _>("quiz_payload")
                                .map_err(failure)?
                                .map(serde_json::from_value)
                                .transpose()
                                .map_err(|_| AppError::Internal)?,
                        },
                    })
                })
                .transpose()?,
        }))
    }
    async fn persist_manifest(
        &mut self,
        p: PathPublication,
        a: AuditActor,
    ) -> AppResult<PathEditor> {
        let c = &mut *self.tx;
        sqlx::query("INSERT INTO learning_path_versions(id,version,paired_bank_version_id,quiz_bank_version_id) VALUES($1,$2,$3,$4)")
            .bind(p.id).bind(p.version).bind(p.paired_bank_version_id).bind(p.quiz_bank_version_id).execute(&mut *c).await.map_err(failure)?;
        for (i, v) in p.lesson_versions.iter().enumerate() {
            sqlx::query("INSERT INTO learning_path_items(path_version_id,lesson_version_id,position) VALUES($1,$2,$3)")
                .bind(p.id).bind(v).bind(i as i32+1).execute(&mut *c).await.map_err(failure)?;
        }
        sqlx::query("UPDATE learning_path_drafts SET revision=$1,updated_at=now() WHERE id=$2")
            .bind(p.revision)
            .bind(p.path_id)
            .execute(&mut *c)
            .await
            .map_err(failure)?;
        audit(c, &a, "path.publish", "path", p.path_id, p.revision).await?;
        path(c, false).await
    }
    async fn commit(self: Box<Self>) -> AppResult<()> {
        self.tx.commit().await.map_err(failure)
    }
}
