//! Durable enrollment projections and prerequisite policy, independent of HTTP/SQL.
use crate::{
    modules::{
        anthropometry::domain::Measurement,
        assessment::domain::{AssessmentType, Attempt},
    },
    shared::error::{AppError, AppResult},
};
use chrono::{DateTime, NaiveDate, Utc};
use serde::Serialize;
use uuid::Uuid;
/// Published manifest facts; availability policy belongs here, never inside a SQL expression.
pub struct PublishedPath {
    pub id: Uuid,
    pub paired_bank: Uuid,
    pub quiz_bank: Uuid,
    pub paired_kind: String,
    pub quiz_kind: String,
    pub lesson_count: i64,
    pub paired_question_count: i64,
    pub quiz_question_count: i64,
}
impl PublishedPath {
    /// Reject incomplete/mismatched manifests without creating any enrollment effects.
    pub fn require_available(&self) -> AppResult<()> {
        if self.lesson_count > 0
            && self.paired_question_count > 0
            && self.quiz_question_count > 0
            && self.paired_kind == "paired"
            && self.quiz_kind == "quiz"
        {
            Ok(())
        } else {
            Err(AppError::Conflict("CONTENT_NOT_AVAILABLE"))
        }
    }
}
#[derive(Clone, Copy)]
pub struct StudentScope {
    pub student_id: Uuid,
    pub enrollment_id: Uuid,
}
#[derive(Clone, Serialize)]
pub struct Student {
    pub id: Uuid,
    pub name: String,
}
#[derive(Clone, Serialize)]
pub struct Enrollment {
    pub id: Uuid,
    pub school_id: Uuid,
    pub school_name_at_enrollment: String,
    pub form_date: NaiveDate,
    pub path_version_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}
#[derive(Clone, Serialize)]
pub struct Lesson {
    pub lesson_id: Uuid,
    pub lesson_version_id: Uuid,
    pub title: String,
    pub position: i32,
    pub opened_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}
#[derive(Clone, Serialize)]
pub struct Section {
    pub id: Uuid,
    pub title: String,
    pub body: String,
    pub required: bool,
    pub position: i32,
}
#[derive(Clone, Serialize)]
pub struct LessonView {
    #[serde(flatten)]
    pub lesson: Lesson,
    pub description: String,
    pub sections: Vec<Section>,
    pub visited_section_ids: Vec<Uuid>,
}
/// A single consistent database projection; stage flags are derived, never persisted twice.
pub struct Journey {
    pub student: Student,
    pub enrollment: Enrollment,
    pub lessons: Vec<Lesson>,
    pub attempts: Vec<Attempt>,
    pub latest_measurement: Option<Measurement>,
}
impl Journey {
    pub fn submitted(&self, typ: AssessmentType) -> bool {
        self.attempts
            .iter()
            .any(|a| a.assessment_type == typ && a.status == "submitted")
    }
    pub fn all_lessons_complete(&self) -> bool {
        !self.lessons.is_empty() && self.lessons.iter().all(|l| l.completed_at.is_some())
    }
    pub fn require_attempt(&self, typ: AssessmentType) -> AppResult<()> {
        let allowed = match typ {
            AssessmentType::PreTest => true,
            AssessmentType::Quiz => {
                self.submitted(AssessmentType::PreTest) && self.all_lessons_complete()
            }
            AssessmentType::PostTest => {
                self.submitted(AssessmentType::Quiz) && self.latest_measurement.is_some()
            }
        };
        if allowed {
            Ok(())
        } else {
            Err(AppError::Conflict("LEARNING_PREREQUISITE_NOT_MET"))
        }
    }
    pub fn lesson(&self, id: Uuid) -> AppResult<&Lesson> {
        let index = self
            .lessons
            .iter()
            .position(|l| l.lesson_id == id)
            .ok_or(AppError::NotFound)?;
        if !self.submitted(AssessmentType::PreTest)
            || self.lessons[..index]
                .iter()
                .any(|l| l.completed_at.is_none())
        {
            return Err(AppError::Conflict("LEARNING_PREREQUISITE_NOT_MET"));
        }
        Ok(&self.lessons[index])
    }
    pub fn require_measurement(&self) -> AppResult<()> {
        if self.submitted(AssessmentType::Quiz) && !self.submitted(AssessmentType::PostTest) {
            Ok(())
        } else {
            Err(AppError::Conflict("LEARNING_PREREQUISITE_NOT_MET"))
        }
    }
    pub fn current_stage(&self) -> &'static str {
        if !self.submitted(AssessmentType::PreTest) {
            "pre_test"
        } else if !self.all_lessons_complete() {
            "lessons"
        } else if !self.submitted(AssessmentType::Quiz) {
            "quiz"
        } else if self.latest_measurement.is_none() {
            "calculator"
        } else if !self.submitted(AssessmentType::PostTest) {
            "post_test"
        } else {
            "completed"
        }
    }
    pub fn progress(&self) -> serde_json::Value {
        use serde_json::json;
        let current = self.current_stage();
        let ids = [
            "pre_test",
            "lessons",
            "quiz",
            "calculator",
            "post_test",
            "completed",
        ];
        let index = ids.iter().position(|s| *s == current).unwrap_or(0);
        let stages=ids.iter().enumerate().map(|(i,id)|json!({"id":id,"status":if i<index || current=="completed"{"completed"}else if i==index{"in_progress"}else{"locked"},"locked_reason":if i>index{Some("LEARNING_PREREQUISITE_NOT_MET")}else{None}})).collect::<Vec<_>>();
        let lessons = self
            .lessons
            .iter()
            .map(|l| {
                let mut v = json!(l);
                v["status"] = json!(if self.lesson(l.lesson_id).is_err() {
                    "locked"
                } else if l.completed_at.is_some() {
                    "completed"
                } else if l.opened_at.is_some() {
                    "in_progress"
                } else {
                    "not_started"
                });
                v
            })
            .collect::<Vec<_>>();
        let active = self
            .attempts
            .iter()
            .filter(|a| a.status == "in_progress")
            .map(|a| (a.assessment_type.as_str().to_owned(), json!(a.id)))
            .collect::<serde_json::Map<_, _>>();
        json!({"enrollment_id":self.enrollment.id,"path_version_id":self.enrollment.path_version_id,"current_stage":current,"stages":stages,"lessons":lessons,"active_attempt_ids":active,"latest_measurement_id":self.latest_measurement.as_ref().map(|m|m.id)})
    }
    pub fn visible_score(&self, a: &Attempt) -> Option<i32> {
        if a.assessment_type == AssessmentType::Quiz || self.submitted(AssessmentType::PostTest) {
            a.score
        } else {
            None
        }
    }
}
