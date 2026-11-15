//! JSON-independent use-case and replay results. No transport status, cookies or keys.
use crate::modules::{
    anthropometry::domain::Measurement,
    assessment::domain::{AssessmentType, Attempt},
    learning::domain::*,
};
use chrono::{DateTime, Utc};
use uuid::Uuid;
#[derive(Clone)]
pub struct StudentProfile {
    pub student: Student,
    pub enrollment: Enrollment,
}
#[derive(Clone)]
pub struct OpenedLesson {
    pub lesson_id: Uuid,
    pub opened_at: Option<DateTime<Utc>>,
    pub visited_section_ids: Vec<Uuid>,
}
#[derive(Clone)]
pub struct CompletedLesson {
    pub lesson_id: Uuid,
    pub completed_at: Option<DateTime<Utc>>,
    pub progress: Progress,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Creation {
    Created,
    Existing,
}
/// Creation/replay exposes identity and lifecycle only, never private persisted scores.
#[derive(Clone)]
pub struct AttemptDescriptor {
    pub id: Uuid,
    pub assessment_type: AssessmentType,
    pub attempt_number: i32,
    pub status: String,
    pub snapshot_id: Uuid,
}
impl From<Attempt> for AttemptDescriptor {
    fn from(a: Attempt) -> Self {
        Self {
            id: a.id,
            assessment_type: a.assessment_type,
            attempt_number: a.attempt_number,
            status: a.status,
            snapshot_id: a.snapshot_id,
        }
    }
}
#[derive(Clone)]
pub struct AttemptCreation {
    pub outcome: Creation,
    pub attempt: AttemptDescriptor,
}
#[derive(Clone)]
pub struct Submission {
    pub attempt_id: Uuid,
    pub submitted_at: Option<DateTime<Utc>>,
    pub score: Option<i32>,
    pub progress: Progress,
}
#[derive(Clone)]
pub struct MeasurementCreation {
    pub measurement: Measurement,
    pub progress: Progress,
}
#[derive(Clone)]
pub struct MeasurementHistory {
    pub measurements: Vec<Measurement>,
    pub limit: i64,
    pub offset: i64,
    pub total: i64,
}
#[derive(Clone)]
pub struct Summary {
    pub profile: StudentProfile,
    pub scores: Vec<(AssessmentType, Option<i32>)>,
    pub latest_measurement: Option<Measurement>,
    pub progress: Progress,
}
/// Version-independent safe facts; the infrastructure adapter owns its storage schema.
#[derive(Clone)]
pub enum OperationResult {
    Enrollment(StudentProfile),
    Attempt(AttemptCreation),
    Submission(Submission),
    Measurement(MeasurementCreation),
}
