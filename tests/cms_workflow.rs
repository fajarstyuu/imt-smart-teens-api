//! End-to-end HTTP CMS publication -> enrollment -> completion with later edits/archives.
#[path = "fixtures/cms.rs"]
mod cms;
mod common;
#[path = "fixtures/http_student.rs"]
mod student;
use common::*;
use serde_json::{Value, json};
use sqlx::PgPool;
#[sqlx::test(migrations = "./migrations")]
async fn path_requires_complete_published_members_and_expected_revision(pool: PgPool) {
    cms::initialize(&pool).await;
    let (app, c, t, _) = cms::editor(&pool).await;
    let (s, v, _) = request(
        &app,
        "POST",
        "/api/v1/admin/learning-path/publication",
        json!({"expected_revision":1}),
        &c,
        &t,
    )
    .await;
    assert_eq!(s, 422, "empty path publication: {v}");
    let content = cms::publish(&app, &c, &t).await;
    assert_eq!(content.path["published"]["version"], 1);
    assert_eq!(
        request(
            &app,
            "POST",
            "/api/v1/admin/learning-path/publication",
            json!({"expected_revision":2}),
            &c,
            &t
        )
        .await
        .0,
        409
    );
    // An incomplete dirty draft must never be silently published into a path.
    let lesson = content.lesson["id"].as_str().unwrap();
    let mut draft = content.lesson["draft"].clone();
    draft["expected_revision"] = json!(2);
    draft["position"] = json!(1);
    draft["sections"] = json!([]);
    assert_eq!(
        request(
            &app,
            "PUT",
            &format!("/api/v1/admin/lessons/{lesson}"),
            draft,
            &c,
            &t
        )
        .await
        .0,
        200
    );
    let (s, p, _) = request(
        &app,
        "POST",
        "/api/v1/admin/learning-path/publication",
        json!({"expected_revision":3}),
        &c,
        &t,
    )
    .await;
    assert_eq!(
        s, 200,
        "explicit path uses latest published lesson, not dirty draft: {p}"
    );
    assert_eq!(
        p["data"]["published"]["lessons"][0]["lesson_version_id"],
        content.lesson["versions"][0]["id"]
    );
    // An archived root is not admitted to a future manifest.
    assert_eq!(
        request(
            &app,
            "POST",
            &format!("/api/v1/admin/lessons/{lesson}/archive"),
            json!({"expected_revision":3}),
            &c,
            &t
        )
        .await
        .0,
        200
    );
    assert_eq!(
        request(
            &app,
            "POST",
            "/api/v1/admin/learning-path/publication",
            json!({"expected_revision":5}),
            &c,
            &t
        )
        .await
        .0,
        422
    );
}
#[sqlx::test(migrations = "./migrations")]
async fn http_cms_publication_student_completion_survives_future_versions_and_archive(
    pool: PgPool,
) {
    cms::initialize(&pool).await;
    let (app, c, t, aid) = cms::editor(&pool).await;
    let content = cms::publish(&app, &c, &t).await;
    let school = cms::school(&pool, "Historical school").await;
    let (sc, st, enrollment) = student::enroll(&app, school).await;
    let pre = student::attempt(&app, &sc, &st, "pre_test").await;
    assert_eq!(
        enrollment["enrollment"]["path_version_id"],
        content.path["published"]["id"]
    );
    assert!(!pre.to_string().contains("correct_option_ids"));
    assert!(!pre.to_string().contains("is_correct"));
    let question = &content.questions[0];
    let bank = question["bank_id"].as_str().unwrap();
    let id = question["id"].as_str().unwrap();
    let qpath = format!("/api/v1/admin/question-banks/{bank}/questions/{id}");
    let (_, q, _) = request(&app, "GET", &qpath, json!(null), &c, &t).await;
    let mut changed = q["data"]["draft"].clone();
    changed["text"] = json!("Future question");
    changed["options"][0]["text"] = json!("Future option");
    changed["expected_revision"] = q["data"]["revision"].clone();
    changed["position"] = json!(1);
    assert_eq!(request(&app, "PUT", &qpath, changed, &c, &t).await.0, 200);
    let (_, banks, _) = request(
        &app,
        "GET",
        "/api/v1/admin/question-banks",
        json!(null),
        &c,
        &t,
    )
    .await;
    let b = banks["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["id"] == question["bank_id"])
        .unwrap();
    assert_eq!(
        request(
            &app,
            "POST",
            &format!("/api/v1/admin/question-banks/{bank}/publication"),
            json!({"expected_revision":b["revision"]}),
            &c,
            &t
        )
        .await
        .0,
        200
    );
    let lesson = content.lesson["id"].as_str().unwrap();
    let base = format!("/api/v1/admin/lessons/{lesson}");
    let mut draft = content.lesson["draft"].clone();
    draft["title"] = json!("Future lesson");
    draft["sections"][0]["body"] = json!("Future body");
    draft["position"] = json!(1);
    draft["expected_revision"] = json!(2);
    assert_eq!(request(&app, "PUT", &base, draft, &c, &t).await.0, 200);
    assert_eq!(
        request(
            &app,
            "POST",
            &format!("{base}/publication"),
            json!({"expected_revision":3}),
            &c,
            &t
        )
        .await
        .0,
        200
    );
    let (s, next, _) = request(
        &app,
        "POST",
        "/api/v1/admin/learning-path/publication",
        json!({"expected_revision":3}),
        &c,
        &t,
    )
    .await;
    assert_eq!(s, 200, "{next}");
    let (nc, nt, new) = student::enroll(&app, school).await;
    assert_ne!(
        new["enrollment"]["path_version_id"],
        enrollment["enrollment"]["path_version_id"]
    );
    assert_ne!(
        student::attempt(&app, &nc, &nt, "pre_test").await["questions"],
        pre["questions"]
    );
    assert_eq!(
        request(
            &app,
            "POST",
            &format!("{base}/archive"),
            json!({"expected_revision":4}),
            &c,
            &t
        )
        .await
        .0,
        200
    );
    assert_eq!(
        student::attempt(&app, &sc, &st, "pre_test").await["questions"],
        pre["questions"]
    );
    cms::answer(&app, &sc, &st, &pre, false).await;
    student::submit(&app, &sc, &st, &pre, "pre").await;
    let (s, l, _) = request(
        &app,
        "GET",
        &format!("/api/v1/student/lessons/{lesson}"),
        json!(null),
        &sc,
        &st,
    )
    .await;
    assert_eq!(s, 200, "{l}");
    assert_eq!(l["data"]["title"], "Historical lesson");
    assert_eq!(l["data"]["sections"][0]["body"], "Historical section");
    assert_eq!(
        request(
            &app,
            "POST",
            &format!("/api/v1/student/lessons/{lesson}/opened"),
            json!({"section_id":l["data"]["sections"][0]["id"]}),
            &sc,
            &st
        )
        .await
        .0,
        200
    );
    assert_eq!(
        request(
            &app,
            "POST",
            &format!("/api/v1/student/lessons/{lesson}/completion"),
            json!({}),
            &sc,
            &st
        )
        .await
        .0,
        200
    );
    let (s, quiz, _) = student::call(
        &app,
        "GET",
        &format!("/api/v1/student/lessons/{lesson}/quiz"),
        Value::Null,
        &sc,
        &st,
        "",
    )
    .await;
    assert_eq!(s, 200, "{quiz}");
    assert!(!quiz.to_string().contains("correct_option_id"));
    let correct = content.lesson["draft"]["quiz"]["correct_option_id"].clone();
    let (s, answer, _) = student::call(
        &app,
        "POST",
        &format!("/api/v1/student/lessons/{lesson}/quiz/answers"),
        json!({"option_id":correct,"expected_revision":0}),
        &sc,
        &st,
        "lesson-quiz-answer",
    )
    .await;
    assert_eq!(s, 200, "{answer}");
    assert_eq!(answer["data"]["correct"], true);
    assert_eq!(
        student::call(
            &app,
            "POST",
            "/api/v1/student/measurements",
            json!({"weight_kg":60,"height_cm":170}),
            &sc,
            &st,
            "measurement"
        )
        .await
        .0,
        201
    );
    let post = student::attempt(&app, &sc, &st, "post_test").await;
    assert_eq!(post["questions"], pre["questions"]);
    assert_eq!(post["snapshot_id"], pre["snapshot_id"]);
    cms::answer(&app, &sc, &st, &post, true).await;
    assert_eq!(
        student::submit(&app, &sc, &st, &post, "post").await["score"],
        100
    );
    let (s, v, _) = request(
        &app,
        "GET",
        "/api/v1/student/summary",
        Value::Null,
        &sc,
        &st,
    )
    .await;
    assert_eq!(s, 200, "{v}");
    assert_eq!(v["data"]["scores"]["pre_test"], 0);
    assert_eq!(v["data"]["scores"]["post_test"], 100);
    // Historical admin reporting is exercised after the complete HTTP-authored journey.
    sqlx::query("INSERT INTO identity_admin_school_grants(admin_id,school_id) VALUES($1,$2)")
        .bind(aid)
        .bind(school)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE identity_schools SET name='Renamed archived school',archived_at=now(),revision=revision+1 WHERE id=$1").bind(school).execute(&pool).await.unwrap();
    let (s, r, _) = request(
        &app,
        "GET",
        &format!(
            "/api/v1/admin/students/{}/results",
            enrollment["student"]["id"].as_str().unwrap()
        ),
        json!(null),
        &c,
        &t,
    )
    .await;
    assert_eq!(s, 200, "{r}");
    assert_eq!(r["data"]["current_stage"], "completed");
    assert_eq!(r["data"]["lesson_quizzes"].as_array().unwrap().len(), 1);
    assert_eq!(r["data"]["lesson_quizzes"][0]["attempt_count"], 1);
    assert_eq!(r["data"]["lesson_quizzes"][0]["status"], "mastered");
    assert_eq!(
        r["data"]["lesson_quizzes"][0]["first_answer"]["text"],
        "Historical A"
    );
    assert!(
        !r["data"]["lesson_quizzes"]
            .to_string()
            .contains("correct_option_id")
    );
    assert_eq!(
        r["data"]["enrollment"]["school_name_at_enrollment"],
        "Historical school"
    );
    assert_eq!(r["data"]["lessons"][0]["title"], "Historical lesson");
    assert_eq!(r["data"]["measurements"][0]["bmi_display"], "20.8");
    assert_eq!(r["data"]["attempts"].as_array().unwrap().len(), 2);
    let (s, list, _) = request(&app, "GET", "/api/v1/admin/students", json!(null), &c, &t).await;
    assert_eq!(s, 200, "{list}");
    assert!(!list.to_string().contains("lesson_quizzes"));
    let (s, r, _) = request(
        &app,
        "GET",
        &format!(
            "/api/v1/admin/attempts/{}/answers",
            post["id"].as_str().unwrap()
        ),
        json!(null),
        &c,
        &t,
    )
    .await;
    assert_eq!(s, 200, "{r}");
    assert_eq!(r["data"][0]["text"], "Historical question");
    assert_eq!(
        r["data"][0]["selected_options"][0]["text"],
        "Historical one"
    );
    assert_eq!(r["data"][0]["answer_state"], "correct");
    let (s,r,_)=request(&app,"GET",&format!("/api/v1/admin/reports/question-answers?question_version_id={}&assessment_type=post_test",post["questions"][0]["question_version_id"].as_str().unwrap()),json!(null),&c,&t).await;
    assert_eq!(s, 200, "{r}");
    assert_eq!(r["meta"]["total"], 1);
    assert_eq!(r["data"][0]["snapshot_id"], pre["snapshot_id"]);
}
