//! Read/visit/complete pinned lessons under an enrollment lock, preserving event timestamps.
use super::*;
use crate::modules::{assessment::domain::AssessmentType, learning::domain::*};
use serde_json::{Value, json};
use uuid::Uuid;
impl StudentService {
    pub async fn me(&self, scope: StudentScope) -> AppResult<Value> {
        let mut tx = self.uow.begin().await?;
        let j = tx.journey(scope).await?;
        let out = json!({"student":j.student,"enrollment":j.enrollment});
        tx.commit().await?;
        Ok(out)
    }
    pub async fn progress(&self, scope: StudentScope) -> AppResult<Value> {
        let mut tx = self.uow.begin().await?;
        let out = tx.journey(scope).await?.progress();
        tx.commit().await?;
        Ok(out)
    }
    pub async fn lesson(&self, scope: StudentScope, id: Uuid) -> AppResult<LessonView> {
        let mut tx = self.uow.begin().await?;
        let j = tx.journey(scope).await?;
        let l = j.lesson(id)?;
        let out = tx.lesson_view(scope, l).await?;
        tx.commit().await?;
        Ok(out)
    }
    pub async fn open_lesson(
        &self,
        scope: StudentScope,
        id: Uuid,
        section: Option<Uuid>,
    ) -> AppResult<Value> {
        let mut tx = self.uow.begin().await?;
        let j = tx.journey(scope).await?;
        let l = j.lesson(id)?;
        let view = tx.lesson_view(scope, l).await?;
        if let Some(id) = section
            && !view.sections.iter().any(|s| s.id == id)
        {
            return Err(AppError::Invalid("INVALID_SECTION"));
        }
        tx.open_lesson(scope, l.lesson_version_id, section).await?;
        let j = tx.journey(scope).await?;
        let v = tx.lesson_view(scope, j.lesson(id)?).await?;
        let out = json!({"lesson_id":id,"opened_at":v.lesson.opened_at,"visited_section_ids":v.visited_section_ids});
        tx.commit().await?;
        Ok(out)
    }
    pub async fn complete_lesson(&self, scope: StudentScope, id: Uuid) -> AppResult<Value> {
        let mut tx = self.uow.begin().await?;
        let j = tx.journey(scope).await?;
        let l = j.lesson(id)?;
        let v = tx.lesson_view(scope, l).await?;
        if v.lesson.opened_at.is_none()
            || v.sections
                .iter()
                .any(|s| s.required && !v.visited_section_ids.contains(&s.id))
        {
            return Err(AppError::Conflict("REQUIRED_SECTIONS_NOT_VISITED"));
        }
        tx.complete_lesson(scope, l.lesson_version_id).await?;
        let j = tx.journey(scope).await?;
        let out = json!({"lesson_id":id,"completed_at":j.lesson(id)?.completed_at,"progress":j.progress()});
        tx.commit().await?;
        Ok(out)
    }
    pub async fn summary(&self, scope: StudentScope) -> AppResult<Value> {
        let mut tx = self.uow.begin().await?;
        let j = tx.journey(scope).await?;
        if !j.submitted(AssessmentType::PostTest) {
            return Err(AppError::Conflict("LEARNING_PREREQUISITE_NOT_MET"));
        }
        let scores = j
            .attempts
            .iter()
            .map(|a| {
                (
                    a.assessment_type.as_str().to_owned(),
                    json!(j.visible_score(a)),
                )
            })
            .collect::<serde_json::Map<_, _>>();
        let out = json!({"student":j.student,"enrollment":j.enrollment,"scores":scores,"latest_measurement":j.latest_measurement,"progress":j.progress()});
        tx.commit().await?;
        Ok(out)
    }
}
