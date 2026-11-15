//! Compiler-checked editor allowlists, distinct from core and versioned JSONB storage.
use super::super::application::{LessonEditor, PathEditor};
use crate::transport::http::editor::{VersionDto, version_dto};
use serde::Serialize;
use uuid::Uuid;
#[derive(Serialize)]
pub struct SectionDraftDto {
    id: Uuid,
    title: String,
    body: String,
    required: bool,
}
#[derive(Serialize)]
pub struct LessonDraftDto {
    title: String,
    description: String,
    sections: Vec<SectionDraftDto>,
}
#[derive(Serialize)]
pub struct LessonEditorDto {
    id: Uuid,
    state: &'static str,
    revision: i64,
    position: Option<i32>,
    draft: LessonDraftDto,
    versions: Vec<VersionDto>,
}
#[derive(Serialize)]
pub struct PathLessonDto {
    lesson_id: Uuid,
    lesson_version_id: Uuid,
    position: i32,
}
#[derive(Serialize)]
pub struct PathManifestDto {
    id: Uuid,
    version: i32,
    paired_bank_version_id: Uuid,
    quiz_bank_version_id: Uuid,
    lessons: Vec<PathLessonDto>,
}
#[derive(Serialize)]
pub struct PathEditorDto {
    id: Uuid,
    revision: i64,
    lesson_ids: Vec<Uuid>,
    paired_bank_id: Uuid,
    quiz_bank_id: Uuid,
    versions: Vec<VersionDto>,
    published: Option<PathManifestDto>,
}
pub fn editor_dto(l: LessonEditor) -> LessonEditorDto {
    LessonEditorDto {
        id: l.id,
        state: l.state.as_str(),
        revision: l.revision,
        position: l.position,
        draft: LessonDraftDto {
            title: l.draft.title,
            description: l.draft.description,
            sections: l
                .draft
                .sections
                .into_iter()
                .map(|s| SectionDraftDto {
                    id: s.id,
                    title: s.title,
                    body: s.body,
                    required: s.required,
                })
                .collect(),
        },
        versions: l.versions.into_iter().map(version_dto).collect(),
    }
}
pub fn path_dto(p: PathEditor) -> PathEditorDto {
    PathEditorDto {
        id: p.id,
        revision: p.revision,
        lesson_ids: p.lesson_ids,
        paired_bank_id: p.paired_bank_id,
        quiz_bank_id: p.quiz_bank_id,
        versions: p.versions.into_iter().map(version_dto).collect(),
        published: p.published.map(|m| PathManifestDto {
            id: m.id,
            version: m.version,
            paired_bank_version_id: m.paired_bank_version_id,
            quiz_bank_version_id: m.quiz_bank_version_id,
            lessons: m
                .lessons
                .into_iter()
                .map(|(lesson_id, lesson_version_id, position)| PathLessonDto {
                    lesson_id,
                    lesson_version_id,
                    position,
                })
                .collect(),
        }),
    }
}
