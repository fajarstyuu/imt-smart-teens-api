//! Behavior checks for the lesson-linked quiz domain contract.
use imt_smart_teens_backend::modules::{
    assessment::lesson_quiz::{LessonQuizDraft, LessonQuizOption},
    learning::admin::application::{LessonDraft, SectionDraft},
};
use uuid::Uuid;

#[path = "common/mod.rs"]
mod common;
#[path = "fixtures/student.rs"]
mod fixture;
#[path = "fixtures/http_student.rs"]
mod student;

fn valid_quiz() -> LessonQuizDraft {
    LessonQuizDraft {
        text: "Manakah jawaban yang benar?".into(),
        options: vec![
            LessonQuizOption {
                id: Uuid::from_u128(1),
                text: "A".into(),
            },
            LessonQuizOption {
                id: Uuid::from_u128(2),
                text: "B".into(),
            },
        ],
        correct_option_id: Some(Uuid::from_u128(1)),
    }
}

#[test]
fn lesson_cannot_publish_without_its_own_quiz() {
    let lesson = LessonDraft {
        title: "Materi".into(),
        description: "Pengantar".into(),
        sections: vec![SectionDraft {
            id: Uuid::from_u128(4),
            title: "Bagian".into(),
            body: "Isi materi".into(),
            required: true,
        }],
        quiz: None,
    };

    assert!(lesson.validate(true).is_err());
    assert!(lesson.validate(false).is_ok());
}

#[test]
fn publishable_lesson_quiz_requires_two_ordered_options_and_one_key() {
    let quiz = valid_quiz();
    assert!(quiz.validate(true).is_ok());

    let mut missing_key = valid_quiz();
    missing_key.correct_option_id = None;
    assert!(missing_key.validate(true).is_err());

    let mut three_options = valid_quiz();
    three_options.options.push(LessonQuizOption {
        id: Uuid::from_u128(3),
        text: "C".into(),
    });
    assert!(three_options.validate(true).is_err());
}

#[test]
fn incomplete_lesson_quiz_is_allowed_only_while_drafting() {
    let mut quiz = valid_quiz();
    quiz.correct_option_id = None;
    assert!(quiz.validate(false).is_ok());
    assert!(quiz.validate(true).is_err());
}

#[sqlx::test(migrations = "./migrations")]
async fn lesson_quiz_read_requires_completed_pretest_and_reading(pool: sqlx::PgPool) {
    let (school, lessons) = fixture::content(&pool).await;
    let app = common::app(pool).await;
    let (cookie, token, _) = student::enroll(&app, school).await;
    let (status, body, _) = student::call(
        &app,
        "GET",
        &format!("/api/v1/student/lessons/{}/quiz", lessons[0]),
        serde_json::Value::Null,
        &cookie,
        &token,
        "",
    )
    .await;

    assert_eq!(status, 409, "{body}");
}

#[sqlx::test(migrations = "./migrations")]
async fn wrong_answer_is_audited_and_next_lesson_unlocks_only_after_mastery(pool: sqlx::PgPool) {
    use serde_json::{Value, json};

    let (school, lessons) = fixture::content(&pool).await;
    let app = common::app(pool.clone()).await;
    let (cookie, token, _) = student::enroll(&app, school).await;
    let (status, pre, _) = student::call(
        &app,
        "POST",
        "/api/v1/student/attempts",
        json!({"assessment_type":"pre_test"}),
        &cookie,
        &token,
        "pre-create",
    )
    .await;
    assert_eq!(status, 200, "{pre}");
    let pre = pre["data"].clone();
    let (status, view, _) = student::call(
        &app,
        "GET",
        &format!(
            "/api/v1/student/attempts/{}",
            pre["attempt_id"].as_str().unwrap()
        ),
        Value::Null,
        &cookie,
        &token,
        "",
    )
    .await;
    assert_eq!(status, 200, "{view}");
    for question in view["data"]["questions"].as_array().unwrap() {
        let ids = question["options"].as_array().unwrap()[0]["id"].clone();
        let (status, body, _) = student::call(
            &app,
            "PUT",
            &format!(
                "/api/v1/student/attempts/{}/answers/{}",
                pre["attempt_id"].as_str().unwrap(),
                question["id"].as_str().unwrap()
            ),
            json!({"option_ids":[ids],"expected_revision":0}),
            &cookie,
            &token,
            "",
        )
        .await;
        assert_eq!(status, 200, "{body}");
    }
    let (status, body, _) = student::call(
        &app,
        "POST",
        &format!(
            "/api/v1/student/attempts/{}/submission",
            pre["attempt_id"].as_str().unwrap()
        ),
        json!({}),
        &cookie,
        &token,
        "pre-submit",
    )
    .await;
    assert_eq!(status, 200, "{body}");

    // A newly published path must not retarget this enrollment's lesson quiz.
    fixture::publish_future(&pool).await;

    let lesson_path = format!("/api/v1/student/lessons/{}", lessons[0]);
    let (status, lesson, _) =
        student::call(&app, "GET", &lesson_path, Value::Null, &cookie, &token, "").await;
    assert_eq!(status, 200, "{lesson}");
    let section_id = lesson["data"]["sections"][0]["id"].clone();
    let (status, body, _) = student::call(
        &app,
        "POST",
        &format!("{lesson_path}/opened"),
        json!({"section_id":section_id}),
        &cookie,
        &token,
        "",
    )
    .await;
    assert_eq!(status, 200, "{body}");
    let (status, body, _) = student::call(
        &app,
        "POST",
        &format!("{lesson_path}/completion"),
        json!({}),
        &cookie,
        &token,
        "",
    )
    .await;
    assert_eq!(status, 200, "{body}");

    let (status, legacy_quiz, _) = student::call(
        &app,
        "POST",
        "/api/v1/student/attempts",
        json!({"assessment_type":"quiz"}),
        &cookie,
        &token,
        "legacy-global-quiz",
    )
    .await;
    assert_ne!(
        status, 200,
        "legacy global quiz cannot bypass lesson mastery: {legacy_quiz}"
    );
    let (status, early_measurement, _) = student::call(
        &app,
        "POST",
        "/api/v1/student/measurements",
        json!({"weight_kg":60.5,"height_cm":170}),
        &cookie,
        &token,
        "early-measurement",
    )
    .await;
    assert_eq!(
        status, 409,
        "calculator/measurement remains locked before lesson mastery: {early_measurement}"
    );

    let (status, quiz, _) = student::call(
        &app,
        "GET",
        &format!("{lesson_path}/quiz"),
        Value::Null,
        &cookie,
        &token,
        "",
    )
    .await;
    assert_eq!(status, 200, "{quiz}");
    assert!(!quiz.to_string().contains("correct_option_id"));
    assert_ne!(quiz["data"]["question"], "Future synthetic lesson question");
    let wrong = quiz["data"]["options"][1]["id"].clone();
    let (status, wrong_result, _) = student::call(
        &app,
        "POST",
        &format!("{lesson_path}/quiz/answers"),
        json!({"option_id":wrong,"expected_revision":0}),
        &cookie,
        &token,
        "lesson-one-wrong",
    )
    .await;
    assert_eq!(status, 200, "{wrong_result}");
    assert_eq!(wrong_result["data"]["correct"], false);
    assert_eq!(wrong_result["data"]["attempt_count"], 1);
    assert_eq!(
        wrong_result["data"]["feedback"],
        "jawablah dengan benar untuk melanjutkan materi selanjutnya"
    );
    let (status, cross_lesson_replay, _) = student::call(
        &app,
        "POST",
        &format!("/api/v1/student/lessons/{}/quiz/answers", lessons[1]),
        json!({"option_id":wrong,"expected_revision":0}),
        &cookie,
        &token,
        "lesson-one-wrong",
    )
    .await;
    assert_eq!(
        status, 409,
        "same idempotency key cannot replay a different lesson: {cross_lesson_replay}"
    );
    assert_eq!(cross_lesson_replay["error"]["code"], "IDEMPOTENCY_CONFLICT");
    let (status, invalid_lesson_replay, _) = student::call(
        &app,
        "POST",
        &format!("/api/v1/student/lessons/{}/quiz/answers", Uuid::new_v4()),
        json!({"option_id":wrong,"expected_revision":0}),
        &cookie,
        &token,
        "lesson-one-wrong",
    )
    .await;
    assert_eq!(
        status, 409,
        "invalid lesson cannot replay another resource's cached response: {invalid_lesson_replay}"
    );
    assert_eq!(
        invalid_lesson_replay["error"]["code"],
        "IDEMPOTENCY_CONFLICT"
    );
    let (status, _, _) = student::call(
        &app,
        "GET",
        &format!("/api/v1/student/lessons/{}", lessons[1]),
        Value::Null,
        &cookie,
        &token,
        "",
    )
    .await;
    assert_eq!(
        status, 409,
        "lesson 2 must remain locked after wrong answer"
    );

    let (status, reloaded, _) = student::call(
        &app,
        "GET",
        &format!("{lesson_path}/quiz"),
        Value::Null,
        &cookie,
        &token,
        "",
    )
    .await;
    assert_eq!(status, 200, "{reloaded}");
    assert_eq!(reloaded["data"]["attempt_count"], 1);
    assert_eq!(reloaded["data"]["first_answer_option_id"], wrong);
    let invalid = Uuid::new_v4();
    let (status, invalid_answer, _) = student::call(
        &app,
        "POST",
        &format!("{lesson_path}/quiz/answers"),
        json!({"option_id":invalid,"expected_revision":1}),
        &cookie,
        &token,
        "invalid-option",
    )
    .await;
    assert_eq!(
        status, 422,
        "an option outside this lesson quiz is rejected: {invalid_answer}"
    );
    let right = reloaded["data"]["options"][0]["id"].clone();
    let (status, right_result, _) = student::call(
        &app,
        "POST",
        &format!("{lesson_path}/quiz/answers"),
        json!({"option_id":right,"expected_revision":1}),
        &cookie,
        &token,
        "lesson-one-right",
    )
    .await;
    assert_eq!(status, 200, "{right_result}");
    assert_eq!(right_result["data"]["correct"], true);
    assert_eq!(right_result["data"]["attempt_count"], 2);
    assert_eq!(right_result["data"]["first_answer_option_id"], wrong);
    let (status, replay, _) = student::call(
        &app,
        "POST",
        &format!("{lesson_path}/quiz/answers"),
        json!({"option_id":right,"expected_revision":1}),
        &cookie,
        &token,
        "lesson-one-right",
    )
    .await;
    assert_eq!(status, 200, "{replay}");
    assert_eq!(replay["data"], right_result["data"]);
    let changed_payload = student::call(
        &app,
        "POST",
        &format!("{lesson_path}/quiz/answers"),
        json!({"option_id":wrong,"expected_revision":1}),
        &cookie,
        &token,
        "lesson-one-right",
    )
    .await;
    assert_eq!(
        changed_payload.0, 409,
        "same operation key cannot replay a changed answer payload"
    );
    let stale_revision = student::call(
        &app,
        "POST",
        &format!("{lesson_path}/quiz/answers"),
        json!({"option_id":right,"expected_revision":1}),
        &cookie,
        &token,
        "stale-revision-after-mastery",
    )
    .await;
    assert_eq!(
        stale_revision.0, 409,
        "stale revision cannot overwrite mastered answer"
    );
    let (status, _, _) = student::call(
        &app,
        "GET",
        &format!("/api/v1/student/lessons/{}", lessons[1]),
        Value::Null,
        &cookie,
        &token,
        "",
    )
    .await;
    assert_eq!(status, 200);

    let second_path = format!("/api/v1/student/lessons/{}", lessons[1]);
    let (status, second, _) =
        student::call(&app, "GET", &second_path, Value::Null, &cookie, &token, "").await;
    assert_eq!(
        status, 200,
        "second lesson is unlocked after lesson-one mastery: {second}"
    );
    let second_section = second["data"]["sections"][0]["id"].clone();
    let (status, opened, _) = student::call(
        &app,
        "POST",
        &format!("{second_path}/opened"),
        json!({"section_id":second_section}),
        &cookie,
        &token,
        "",
    )
    .await;
    assert_eq!(status, 200, "{opened}");
    let (status, completion, _) = student::call(
        &app,
        "POST",
        &format!("{second_path}/completion"),
        json!({}),
        &cookie,
        &token,
        "",
    )
    .await;
    assert_eq!(status, 200, "{completion}");
    let (status, second_quiz, _) = student::call(
        &app,
        "GET",
        &format!("{second_path}/quiz"),
        Value::Null,
        &cookie,
        &token,
        "",
    )
    .await;
    assert_eq!(status, 200, "{second_quiz}");
    let second_wrong = second_quiz["data"]["options"][1]["id"].clone();
    let (status, _, _) = student::call(
        &app,
        "POST",
        &format!("{second_path}/quiz/answers"),
        json!({"option_id":second_wrong,"expected_revision":0}),
        &cookie,
        &token,
        "lesson-two-wrong",
    )
    .await;
    assert_eq!(status, 200);
    let (status, still_locked, _) = student::call(
        &app,
        "POST",
        "/api/v1/student/measurements",
        json!({"weight_kg":60.5,"height_cm":170}),
        &cookie,
        &token,
        "still-locked-measurement",
    )
    .await;
    assert_eq!(
        status, 409,
        "all lesson quizzes must be mastered before calculator: {still_locked}"
    );
    let second_right = second_quiz["data"]["options"][0]["id"].clone();
    let (status, mastered, _) = student::call(
        &app,
        "POST",
        &format!("{second_path}/quiz/answers"),
        json!({"option_id":second_right,"expected_revision":1}),
        &cookie,
        &token,
        "lesson-two-right",
    )
    .await;
    assert_eq!(status, 200, "{mastered}");
    assert_eq!(mastered["data"]["mastered"], true);
    let (status, measurement, _) = student::call(
        &app,
        "POST",
        "/api/v1/student/measurements",
        json!({"weight_kg":60.5,"height_cm":170}),
        &cookie,
        &token,
        "final-measurement",
    )
    .await;
    assert_eq!(
        status, 201,
        "calculator opens after all lesson mastery: {measurement}"
    );
    let (status, post, _) = student::call(
        &app,
        "POST",
        "/api/v1/student/attempts",
        json!({"assessment_type":"post_test"}),
        &cookie,
        &token,
        "post-test-after-masteries",
    )
    .await;
    assert_eq!(status, 201, "post-test opens after measurement: {post}");

    let stored: (Uuid, i32, bool) = sqlx::query_as("SELECT first_option_id,attempt_count,mastered FROM learning_lesson_quiz_attempts WHERE lesson_version_id=(SELECT i.lesson_version_id FROM learning_path_items i JOIN learning_lesson_versions v ON v.id=i.lesson_version_id JOIN learning_enrollments e ON e.path_version_id=i.path_version_id WHERE v.lesson_id=$1 AND e.id=(SELECT id FROM learning_enrollments ORDER BY created_at DESC LIMIT 1)) AND enrollment_id=(SELECT id FROM learning_enrollments ORDER BY created_at DESC LIMIT 1)")
        .bind(lessons[0]).fetch_one(&pool).await.unwrap();
    assert_eq!(stored.0, Uuid::parse_str(wrong.as_str().unwrap()).unwrap());
    assert_eq!(stored.1, 2);
    assert!(stored.2);
}
