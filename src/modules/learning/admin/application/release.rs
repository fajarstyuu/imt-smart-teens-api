//! One reviewed release coordinates draft validation and immutable publication in one transaction.
//! Root locks use the same path -> sorted banks -> sorted lessons order as existing writers.
use super::*;
use crate::modules::assessment::domain::validate_score_budget;

/// A revision is a review precondition, not permission to fetch a newer draft silently.
pub struct ReviewedRoot {
    pub id: Uuid,
    pub expected_revision: i64,
}
pub struct ReviewedRelease {
    pub expected_revision: i64,
    pub lessons: Vec<ReviewedRoot>,
    pub banks: Vec<ReviewedRoot>,
}

pub async fn release_in_transaction(
    mut tx: Box<dyn PathPublicationTransaction>,
    command: ReviewedRelease,
    actor: AuditActor,
) -> AppResult<PathEditor> {
    let path = tx.lock_path().await?;
    check_revision(path.revision, command.expected_revision)?;
    if path.lesson_ids.is_empty() {
        return Err(AppError::Invalid("EMPTY_LEARNING_PATH"));
    }
    let ids: Vec<_> = command.lessons.iter().map(|r| r.id).collect();
    if ids != path.lesson_ids {
        return Err(AppError::Conflict("STALE_CONTENT_REVIEW"));
    }
    let banks = [path.paired_bank_id];
    let mut reviewed_banks: Vec<_> = command.banks.iter().map(|b| b.id).collect();
    reviewed_banks.sort();
    if reviewed_banks != banks {
        return Err(AppError::Conflict("STALE_CONTENT_REVIEW"));
    }
    tx.lock_bank_roots(&banks).await?;
    let mut sorted = ids.clone();
    sorted.sort();
    tx.lock_lesson_roots(&sorted).await?;
    // Admit every reviewed draft before creating even the first immutable child. The adapter
    // still rolls back all writes/audits on persistence failure after admission.
    for root in &command.lessons {
        revision(root.expected_revision)?;
        let lesson = tx.lesson_draft(root.id).await?;
        check_revision(lesson.revision, root.expected_revision)?;
        if lesson.state == ContentState::Archived {
            return Err(AppError::Conflict("CONTENT_ARCHIVED"));
        }
        lesson.draft.validate(true)?;
    }
    for root in &command.banks {
        revision(root.expected_revision)?;
        let (bank, questions) = tx.bank_drafts(root.id).await?;
        check_revision(bank.revision, root.expected_revision)?;
        if bank.state == ContentState::Archived {
            return Err(AppError::Conflict("CONTENT_ARCHIVED"));
        }
        if root.id != path.paired_bank_id || bank.kind != "paired" {
            return Err(AppError::Conflict("CONTENT_NOT_AVAILABLE"));
        }
        if questions.is_empty() {
            return Err(AppError::Invalid("EMPTY_QUESTION_BANK"));
        }
        let mut positions = HashSet::new();
        for question in &questions {
            question.draft.validate()?;
            if question.position < 1 || !positions.insert(question.position) {
                return Err(AppError::Invalid("INVALID_QUESTION_ORDER"));
            }
        }
        validate_score_budget(questions.iter().map(|_| 10000))?;
    }
    for root in command.lessons {
        tx.publish_draft_lesson(root.id, root.expected_revision, actor.clone())
            .await?;
    }
    for root in command.banks {
        tx.publish_draft_bank(root.id, root.expected_revision, actor.clone())
            .await?;
    }
    // Reuse the existing core admission/sealing workflow, consuming exactly this transaction.
    publish_path_in_transaction(tx, command.expected_revision, actor).await
}
