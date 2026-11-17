//! Global admin reporting reads immutable snapshots and committed grading facts.
#[path = "fixtures/cms.rs"]
mod cms;
mod common;
#[path = "fixtures/http_student.rs"]
mod student;
use common::*;
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;
async fn setup(pool: &PgPool) -> (axum::Router, String, String, Uuid, cms::Content) {
    cms::initialize(pool).await;
    let (app, c, t, id) = cms::editor(pool).await;
    let content = cms::publish(&app, &c, &t).await;
    (app, c, t, id, content)
}
async fn grant(pool: &PgPool, admin: Uuid, school: Uuid) {
    sqlx::query("INSERT INTO identity_admin_school_grants(admin_id,school_id) VALUES($1,$2)")
        .bind(admin)
        .bind(school)
        .execute(pool)
        .await
        .unwrap();
}
#[sqlx::test(migrations = "./migrations")]
async fn global_reports_preserve_counts_pages_and_detail_selection(pool: PgPool) {
    let (app, c, t, editor, content) = setup(&pool).await;
    let a = cms::school(&pool, "A").await;
    let b = cms::school(&pool, "B").await;
    let (_, _, one) = student::enroll(&app, a).await;
    let (_, _, two) = student::enroll(&app, a).await;
    let (bc, bt, foreign) = student::enroll(&app, b).await;
    let (s, v, _) = request(
        &app,
        "GET",
        "/api/v1/admin/students?limit=1",
        json!(null),
        &c,
        &t,
    )
    .await;
    assert_eq!(s, 200, "empty scope list: {v}");
    assert_eq!(v["meta"]["total"], 3);
    assert_eq!(v["data"].as_array().unwrap().len(), 1);
    let reader = admin(&pool, false).await;
    grant(&pool, reader, a).await;
    let (rc, rt) = login(&app, reader).await;
    let (s, p1, _) = request(
        &app,
        "GET",
        "/api/v1/admin/students?limit=1&offset=0",
        json!(null),
        &rc,
        &rt,
    )
    .await;
    assert_eq!(s, 200, "reader with no content/catalog grant: {p1}");
    assert_eq!(p1["meta"]["total"], 3);
    assert_eq!(p1["data"].as_array().unwrap().len(), 1);
    let (_, p2, _) = request(
        &app,
        "GET",
        "/api/v1/admin/students?limit=1&offset=1",
        json!(null),
        &rc,
        &rt,
    )
    .await;
    assert_eq!(p2["meta"]["total"], 3);
    assert_ne!(p1["data"][0]["student_id"], p2["data"][0]["student_id"]);
    let (_, empty, _) = request(
        &app,
        "GET",
        "/api/v1/admin/students?offset=99",
        json!(null),
        &rc,
        &rt,
    )
    .await;
    assert_eq!(empty["meta"]["total"], 3);
    assert_eq!(empty["data"], json!([]));
    assert_eq!(
        request(
            &app,
            "GET",
            &format!("/api/v1/admin/students?school_id={b}"),
            json!(null),
            &rc,
            &rt
        )
        .await
        .0,
        200
    );
    let (_, pre_versions, _) = request(
        &app,
        "GET",
        &format!(
            "/api/v1/admin/question-banks/{}/questions/{}",
            content.questions[0]["bank_id"].as_str().unwrap(),
            content.questions[0]["id"].as_str().unwrap()
        ),
        json!(null),
        &c,
        &t,
    )
    .await;
    let version = pre_versions["data"]["versions"][0]["id"].as_str().unwrap();
    let report =
        format!("/api/v1/admin/reports/question-answers?question_version_id={version}&limit=1");
    let (s, r, _) = request(&app, "GET", &report, json!(null), &rc, &rt).await;
    assert_eq!(s, 200, "{r}");
    assert_eq!(r["meta"]["total"], 6);
    assert_eq!(r["data"].as_array().unwrap().len(), 1);
    let (_, r, _) = request(
        &app,
        "GET",
        &format!("{report}&offset=99"),
        json!(null),
        &rc,
        &rt,
    )
    .await;
    assert_eq!(r["meta"]["total"], 6);
    assert_eq!(r["data"], json!([]));
    let (_, r, _) = request(&app, "GET", &report, json!(null), &c, &t).await;
    assert_eq!(r["meta"]["total"], 6);
    assert_eq!(
        request(
            &app,
            "GET",
            &format!("{report}&question_id={}", Uuid::new_v4()),
            json!(null),
            &rc,
            &rt
        )
        .await
        .0,
        422
    );
    let foreign_id = foreign["student"]["id"].as_str().unwrap();
    let foreign_attempt = student::attempt(&app, &bc, &bt, "pre_test").await;
    for (path, expected) in [
        (format!("/api/v1/admin/students/{foreign_id}/results"), 200),
        (
            format!(
                "/api/v1/admin/students/{}/results?enrollment_id={}",
                one["student"]["id"].as_str().unwrap(),
                foreign["enrollment"]["id"].as_str().unwrap()
            ),
            404,
        ),
        (
            format!(
                "/api/v1/admin/attempts/{}/answers",
                foreign_attempt["id"].as_str().unwrap()
            ),
            200,
        ),
    ] {
        assert_eq!(
            request(&app, "GET", &path, json!(null), &rc, &rt).await.0,
            expected
        );
    }
    let id = one["student"]["id"].as_str().unwrap();
    let path = format!("/api/v1/admin/students/{id}/results");
    let (s, v, _) = request(&app, "GET", &path, json!(null), &rc, &rt).await;
    assert_eq!(s, 200, "{v}");
    assert_eq!(v["data"]["enrollment"]["id"], one["enrollment"]["id"]);
    assert_eq!(v["data"]["attempts"][0]["assessment_type"], "pre_test");
    assert_eq!(
        v["data"]["lessons"][0]["lesson_version_id"],
        content.lesson["versions"][0]["id"]
    );
    // Global admins must explicitly select among multiple enrollments across schools.
    let extra = Uuid::new_v4();
    sqlx::query("INSERT INTO learning_enrollments(id,student_id,school_id,school_name_at_enrollment,form_date,path_version_id) SELECT $1,student_id,$2,'B',form_date,path_version_id FROM learning_enrollments WHERE id=$3").bind(extra).bind(b).bind(one["enrollment"]["id"].as_str().unwrap().parse::<Uuid>().unwrap()).execute(&pool).await.unwrap();
    assert_eq!(
        request(&app, "GET", &path, json!(null), &rc, &rt).await.0,
        409
    );
    // Legacy grant insertion has no effect on the global policy.
    grant(&pool, reader, b).await;
    let (s, v, _) = request(&app, "GET", &path, json!(null), &rc, &rt).await;
    assert_eq!(s, 409, "{v}");
    assert_eq!(v["error"]["code"], "ENROLLMENT_SELECTION_REQUIRED");
    assert_eq!(
        request(
            &app,
            "GET",
            &format!(
                "{path}?enrollment_id={}",
                one["enrollment"]["id"].as_str().unwrap()
            ),
            json!(null),
            &rc,
            &rt
        )
        .await
        .0,
        200
    );
    assert_eq!(
        request(&app, "GET", "/api/v1/admin/students", json!(null), &bc, &bt)
            .await
            .0,
        401
    );
    // Another authenticated admin sees the same global enrollment ambiguity.
    assert_eq!(
        request(&app, "GET", &path, json!(null), &c, &t).await.0,
        409
    );
    let _ = (editor, two);
}
#[sqlx::test(migrations = "./migrations")]
async fn report_states_pending_clear_graded_historical_versions_and_required_selection(
    pool: PgPool,
) {
    let (app, c, t, aid, content) = setup(&pool).await;
    let school = cms::school(&pool, "Scope").await;
    grant(&pool, aid, school).await;
    let (sc, st, enrollment) = student::enroll(&app, school).await;
    let pre = student::attempt(&app, &sc, &st, "pre_test").await;
    let id = pre["id"].as_str().unwrap();
    let q = &pre["questions"][0];
    let version = q["question_version_id"].as_str().unwrap();
    let path = format!("/api/v1/admin/attempts/{id}/answers?limit=1");
    let (s, v, _) = request(&app, "GET", &path, json!(null), &c, &t).await;
    assert_eq!(s, 200, "attempt answers: {v}");
    assert_eq!(v["meta"]["total"], 2);
    assert_eq!(v["data"][0]["answer_state"], "not_started");
    assert_eq!(v["data"][0]["correctness"], Value::Null);
    let report = format!("/api/v1/admin/reports/question-answers?question_version_id={version}");
    let (s, v, _) = request(&app, "GET", &report, json!(null), &c, &t).await;
    assert_eq!(s, 200, "{v}");
    assert_eq!(v["meta"]["total"], 2);
    let post = v["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["assessment_type"] == "post_test")
        .unwrap();
    assert_eq!(post["attempt_id"], Value::Null);
    assert_eq!(post["answer_state"], "not_started");
    let save = format!(
        "/api/v1/student/attempts/{id}/answers/{}",
        q["id"].as_str().unwrap()
    );
    assert_eq!(
        request(
            &app,
            "PUT",
            &save,
            json!({"option_ids":[q["options"][0]["id"]],"expected_revision":0}),
            &sc,
            &st
        )
        .await
        .0,
        200
    );
    let (_, v, _) = request(&app, "GET", &path, json!(null), &c, &t).await;
    assert_eq!(v["data"][0]["answer_state"], "answered");
    // Missing fields must not masquerade as explicit nullable grading. Report option
    // schemas cannot gain private keys while the editor transport is refactored.
    let row = &v["data"][0];
    let expected = [
        "student_id",
        "student_name",
        "enrollment_id",
        "school_id",
        "school_name_at_enrollment",
        "bank_id",
        "bank_version_id",
        "snapshot_id",
        "snapshot_question_id",
        "question_id",
        "question_version_id",
        "position",
        "text",
        "type",
        "assessment_type",
        "attempt_id",
        "attempt_number",
        "attempt_status",
        "answer_revision",
        "saved_at",
        "submitted_at",
        "options",
        "selected_option_ids",
        "selected_options",
        "answer_state",
        "correctness",
        "earned_points",
        "max_points",
    ];
    assert_eq!(
        row.as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<std::collections::BTreeSet<_>>(),
        expected.into_iter().collect()
    );
    for option in row["options"]
        .as_array()
        .unwrap()
        .iter()
        .chain(row["selected_options"].as_array().unwrap())
    {
        assert_eq!(
            option
                .as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect::<std::collections::BTreeSet<_>>(),
            ["id", "text"].into_iter().collect()
        );
    }
    assert_eq!(
        row["selected_option_ids"][0],
        row["selected_options"][0]["id"]
    );
    assert_eq!(row["max_points"].to_string(), "1.0000");
    assert!(row["max_points"].is_number());
    assert_eq!(row["submitted_at"], Value::Null);
    assert_eq!(v["data"][0]["correctness"], Value::Null);
    assert_eq!(v["data"][0]["earned_points"], Value::Null);
    assert_eq!(
        v["data"][0]["selected_options"][0]["text"],
        "Historical one"
    );
    assert_eq!(
        request(
            &app,
            "PUT",
            &save,
            json!({"option_ids":[],"expected_revision":1}),
            &sc,
            &st
        )
        .await
        .0,
        200
    );
    let (_, v, _) = request(&app, "GET", &path, json!(null), &c, &t).await;
    assert_eq!(v["data"][0]["answer_state"], "unanswered");
    assert_eq!(v["data"][0]["answer_revision"], 2);
    assert_eq!(
        request(
            &app,
            "PUT",
            &save,
            json!({"option_ids":[q["options"][0]["id"]],"expected_revision":2}),
            &sc,
            &st
        )
        .await
        .0,
        200
    );
    let second = &pre["questions"][1];
    assert_eq!(
        request(
            &app,
            "PUT",
            &format!(
                "/api/v1/student/attempts/{id}/answers/{}",
                second["id"].as_str().unwrap()
            ),
            json!({"option_ids":[second["options"][1]["id"]],"expected_revision":0}),
            &sc,
            &st
        )
        .await
        .0,
        200
    );
    assert_eq!(
        student::submit(&app, &sc, &st, &pre, "pre").await["score"],
        Value::Null
    );
    let (_, v, _) = request(
        &app,
        "GET",
        &format!("/api/v1/admin/attempts/{id}/answers"),
        json!(null),
        &c,
        &t,
    )
    .await;
    assert_eq!(v["data"][0]["answer_state"], "correct");
    assert_eq!(v["data"][0]["earned_points"].to_string(), "1.0000");
    assert_eq!(v["data"][1]["answer_state"], "incorrect");
    assert_eq!(v["data"][1]["earned_points"].to_string(), "0.0000");
    assert_eq!(v["data"][0]["attempt_number"], 1);
    assert_eq!(v["data"][0]["correctness"], true);
    assert_eq!(v["data"][1]["correctness"], false);
    assert!(v["data"][0]["earned_points"].is_number());
    assert_eq!(v["data"][0]["snapshot_id"], pre["snapshot_id"]);
    assert_eq!(
        request(
            &app,
            "GET",
            &format!(
                "/api/v1/admin/reports/question-answers?question_id={}",
                q["question_id"].as_str().unwrap()
            ),
            json!(null),
            &c,
            &t
        )
        .await
        .0,
        422
    );
    let (s, v, _) = request(
        &app,
        "GET",
        "/api/v1/admin/reports/question-answers",
        json!(null),
        &c,
        &t,
    )
    .await;
    assert_eq!(s, 422);
    assert_eq!(v["error"]["code"], "QUESTION_VERSION_REQUIRED");
    assert!(v["error"]["message"].as_str().unwrap().contains("versi"));
    // Publish a new question version after grading; the report remains frozen on the selected old version.
    let root = &content.questions[0];
    let base = format!(
        "/api/v1/admin/question-banks/{}/questions/{}",
        root["bank_id"].as_str().unwrap(),
        root["id"].as_str().unwrap()
    );
    let (_, draft, _) = request(&app, "GET", &base, json!(null), &c, &t).await;
    let mut changed = draft["data"]["draft"].clone();
    changed["text"] = json!("Future text");
    changed["position"] = json!(1);
    changed["expected_revision"] = draft["data"]["revision"].clone();
    assert_eq!(request(&app, "PUT", &base, changed, &c, &t).await.0, 200);
    let (_, banks, _) = request(
        &app,
        "GET",
        "/api/v1/admin/question-banks",
        json!(null),
        &c,
        &t,
    )
    .await;
    let bank = banks["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["id"] == root["bank_id"])
        .unwrap();
    assert_eq!(
        request(
            &app,
            "POST",
            &format!(
                "/api/v1/admin/question-banks/{}/publication",
                bank["id"].as_str().unwrap()
            ),
            json!({"expected_revision":bank["revision"]}),
            &c,
            &t
        )
        .await
        .0,
        200
    );
    let (_, v, _) = request(
        &app,
        "GET",
        &format!("{report}&assessment_type=pre_test"),
        json!(null),
        &c,
        &t,
    )
    .await;
    assert_eq!(v["meta"]["total"], 1);
    assert_eq!(v["data"][0]["text"], "Historical question");
    assert_eq!(v["data"][0]["question_version_id"], version);
    let results = format!(
        "/api/v1/admin/students/{}/results",
        enrollment["student"]["id"].as_str().unwrap()
    );
    let (_, v, _) = request(&app, "GET", &results, json!(null), &c, &t).await;
    assert_eq!(v["data"]["attempts"][0]["score_percent"], 50);
    let result = &v["data"];
    assert_eq!(
        result
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<std::collections::BTreeSet<_>>(),
        [
            "student",
            "enrollment",
            "current_stage",
            "latest_submitted_at",
            "attempts",
            "lessons",
            "measurements"
        ]
        .into_iter()
        .collect()
    );
    let attempt = &result["attempts"][0];
    assert_eq!(
        attempt
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<std::collections::BTreeSet<_>>(),
        [
            "id",
            "snapshot_id",
            "assessment_type",
            "attempt_number",
            "status",
            "created_at",
            "submitted_at",
            "earned_points",
            "max_points",
            "score_percent"
        ]
        .into_iter()
        .collect()
    );
    assert_eq!(attempt["earned_points"].to_string(), "1.0000");
    assert_eq!(attempt["max_points"].to_string(), "2.0000");
    assert!(attempt["earned_points"].is_number());
}
#[sqlx::test(migrations = "./migrations")]
async fn report_filters_are_strict_and_date_submission_ranges_are_precise(pool: PgPool) {
    let (app, c, t, aid, _) = setup(&pool).await;
    let school = cms::school(&pool, "Dates").await;
    grant(&pool, aid, school).await;
    let (sc, st, _) = student::enroll(&app, school).await;
    let (s, v, _) = request(&app, "GET", "/api/v1/admin/students", json!(null), &c, &t).await;
    assert_eq!(s, 200, "{v}");
    for query in [
        "unknown=1",
        "limit=1&limit=2",
        "limit=0",
        "limit=101",
        "offset=-1",
        "stage=wrong",
        "assessment_type=pre",
        "school_id=nope",
        "form_date_from=2026-02-30",
        "form_date_from=2026-11-16&form_date_to=2026-11-15",
        "submitted_from=2026-11-15",
        "submitted_from=2026-11-15T00:00:00%2B07:00",
        "submitted_from=2026-11-16T00:00:00Z&submitted_to=2026-11-15T00:00:00Z",
    ] {
        let (s, v, _) = request(
            &app,
            "GET",
            &format!("/api/v1/admin/students?{query}"),
            json!(null),
            &c,
            &t,
        )
        .await;
        assert_eq!(s, 422, "accepted {query}: {v}");
    }
    for (query, total) in [
        ("form_date_from=2026-11-15&form_date_to=2026-11-15", 1),
        ("form_date_to=2026-11-14", 0),
        ("stage=pre_test", 1),
        ("stage=completed", 0),
        ("assessment_type=post_test", 0),
    ] {
        let (_, v, _) = request(
            &app,
            "GET",
            &format!("/api/v1/admin/students?{query}"),
            json!(null),
            &c,
            &t,
        )
        .await;
        assert_eq!(v["meta"]["total"], total, "{query}: {v}");
    }
    let pre = student::attempt(&app, &sc, &st, "pre_test").await;
    cms::answer(&app, &sc, &st, &pre, true).await;
    let submitted = student::submit(&app, &sc, &st, &pre, "submit").await;
    let stamp = submitted["submitted_at"].as_str().unwrap();
    for (query, total) in [
        (
            format!("submitted_from={stamp}&assessment_type=pre_test"),
            1,
        ),
        (format!("submitted_to={stamp}&assessment_type=pre_test"), 0),
        ("stage=lessons".into(), 1),
    ] {
        let (_, v, _) = request(
            &app,
            "GET",
            &format!("/api/v1/admin/students?{query}"),
            json!(null),
            &c,
            &t,
        )
        .await;
        assert_eq!(v["meta"]["total"], total, "{query}: {v}");
    }
}
