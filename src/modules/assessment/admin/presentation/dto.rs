//! Admin-only DTOs. Private draft keys are explicit here and never reused by students/reports.
use super::super::application::{BankEditor, QuestionEditor};
use crate::transport::http::editor::{VersionDto, version_dto};
use serde::Serialize;
use uuid::Uuid;
#[derive(Serialize)]
pub struct BankEditorDto {
    id: Uuid,
    kind: String,
    state: &'static str,
    revision: i64,
    versions: Vec<VersionDto>,
}
#[derive(Serialize)]
pub struct OptionDraftDto {
    id: Uuid,
    text: String,
}
#[derive(Serialize)]
pub struct QuestionDraftDto {
    text: String,
    #[serde(rename = "type")]
    question_type: &'static str,
    options: Vec<OptionDraftDto>,
    correct_option_ids: Vec<Uuid>,
    required: bool,
}
#[derive(Serialize)]
pub struct QuestionEditorDto {
    id: Uuid,
    bank_id: Uuid,
    state: &'static str,
    revision: i64,
    position: i32,
    draft: QuestionDraftDto,
    versions: Vec<VersionDto>,
}
pub fn bank_dto(b: BankEditor) -> BankEditorDto {
    BankEditorDto {
        id: b.id,
        kind: b.kind,
        state: b.state.as_str(),
        revision: b.revision,
        versions: b.versions.into_iter().map(version_dto).collect(),
    }
}
pub fn question_dto(q: QuestionEditor) -> QuestionEditorDto {
    QuestionEditorDto {
        id: q.id,
        bank_id: q.bank_id,
        state: q.state.as_str(),
        revision: q.revision,
        position: q.position,
        draft: QuestionDraftDto {
            text: q.draft.text,
            question_type: q.draft.question_type.as_str(),
            options: q
                .draft
                .options
                .into_iter()
                .map(|o| OptionDraftDto {
                    id: o.id,
                    text: o.text,
                })
                .collect(),
            correct_option_ids: q.draft.correct_option_ids,
            required: q.draft.required,
        },
        versions: q.versions.into_iter().map(version_dto).collect(),
    }
}
