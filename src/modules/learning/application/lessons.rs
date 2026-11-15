//! Read/visit/complete pinned lessons under an enrollment lock, preserving event timestamps.
use super::*;
use crate::modules::{assessment::domain::AssessmentType, learning::domain::*};
use uuid::Uuid;
impl StudentService {
    /// Restore identity and the enrollment-time school snapshot from the authenticated scope.
    pub async fn me(&self, scope: StudentScope) -> AppResult<StudentProfile> {
        let mut tx = self.uow.begin().await?;
        let j = tx.journey(scope).await?;
        let out = StudentProfile {
            student: j.student,
            enrollment: j.enrollment,
        };
        tx.commit().await?;
        Ok(out)
    }
    /// Derive stage locks from committed attempt, lesson and measurement records, not mutable flags.
    pub async fn progress(&self, scope: StudentScope) -> AppResult<Progress> {
        let mut tx = self.uow.begin().await?;
        let out = tx.journey(scope).await?.progress();
        tx.commit().await?;
        Ok(out)
    }
    /// Protect body reads using the same ordered-path policy as mutations.
    pub async fn lesson(&self, scope: StudentScope, id: Uuid) -> AppResult<LessonView> {
        let mut tx = self.uow.begin().await?;
        let j = tx.journey(scope).await?;
        let l = j.lesson(id)?;
        let out = tx.lesson_view(scope, l).await?;
        tx.commit().await?;
        Ok(out)
    }
    /// Record an idempotent opening/optional visit without conflating it with completion.
    pub async fn open_lesson(
        &self,
        scope: StudentScope,
        id: Uuid,
        section: Option<Uuid>,
    ) -> AppResult<OpenedLesson> {
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
        let out = OpenedLesson {
            lesson_id: id,
            opened_at: v.lesson.opened_at,
            visited_section_ids: v.visited_section_ids,
        };
        tx.commit().await?;
        Ok(out)
    }
    /// Complete only after required pinned sections were visited; repeats keep the first timestamp.
    pub async fn complete_lesson(
        &self,
        scope: StudentScope,
        id: Uuid,
    ) -> AppResult<CompletedLesson> {
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
        let out = CompletedLesson {
            lesson_id: id,
            completed_at: j.lesson(id)?.completed_at,
            progress: j.progress(),
        };
        tx.commit().await?;
        Ok(out)
    }
    /// Reveal own paired results only after the post-test committed.
    pub async fn summary(&self, scope: StudentScope) -> AppResult<Summary> {
        let mut tx = self.uow.begin().await?;
        let j = tx.journey(scope).await?;
        if !j.submitted(AssessmentType::PostTest) {
            return Err(AppError::Conflict("LEARNING_PREREQUISITE_NOT_MET"));
        }
        let scores = j
            .attempts
            .iter()
            .map(|a| (a.assessment_type, j.visible_score(a)))
            .collect();
        let progress = j.progress();
        let out = Summary {
            profile: StudentProfile {
                student: j.student,
                enrollment: j.enrollment,
            },
            scores,
            latest_measurement: j.latest_measurement,
            progress,
        };
        tx.commit().await?;
        Ok(out)
    }
}
