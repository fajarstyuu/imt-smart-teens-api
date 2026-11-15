//! Real PostgreSQL student journeys; synthetic published versions are explicit test setup.
mod common;
use axum::{
    Router,
    body::Body,
    http::{Request, header},
};
use common::*;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

async fn call(
    app: &Router,
    method: &str,
    path: &str,
    body: Value,
    cookie: &str,
    token: &str,
    key: &str,
) -> (u16, Value, Vec<String>) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::ORIGIN, "http://localhost:5173")
                .header(header::COOKIE, cookie)
                .header("x-csrf-token", token)
                .header("idempotency-key", key)
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    let status = response.status().as_u16();
    let cookies = response
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .map(|v| v.to_str().unwrap().to_owned())
        .collect();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap()
    };
    (status, value, cookies)
}
/// Two lessons, each with one required section; one single and one multiple question per bank.
async fn content(pool: &PgPool) -> (Uuid, Vec<Uuid>) {
    let school = Uuid::new_v4();
    sqlx::query("INSERT INTO identity_schools(id,name,normalized_name) VALUES($1,'Synthetic school','synthetic school')").bind(school).execute(pool).await.unwrap();
    let mut banks = vec![];
    for kind in ["paired", "quiz"] {
        let bank = Uuid::new_v4();
        let version = Uuid::new_v4();
        banks.push(version);
        sqlx::query("INSERT INTO assessment_banks(id,kind,state) VALUES($1,$2,'published')")
            .bind(bank)
            .bind(kind)
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO assessment_bank_versions(id,bank_id,version) VALUES($1,$2,1)")
            .bind(version)
            .bind(bank)
            .execute(pool)
            .await
            .unwrap();
        for (index, typ) in ["single", "multiple"].iter().enumerate() {
            let q = Uuid::new_v4();
            let v = Uuid::new_v4();
            sqlx::query("INSERT INTO assessment_questions(id,bank_id,state,draft_payload,position) VALUES($1,$2,'published','{}',$3)").bind(q).bind(bank).bind(index as i32+1).execute(pool).await.unwrap();
            sqlx::query("INSERT INTO assessment_question_versions(id,question_id,version,text,type,required,scoring_rule,max_points) VALUES($1,$2,1,'Synthetic frozen question',$3,true,'exact_set_v1',1)").bind(v).bind(q).bind(typ).execute(pool).await.unwrap();
            for position in 1..=3 {
                sqlx::query("INSERT INTO assessment_options(id,question_version_id,position,text,is_correct) VALUES($1,$2,$3,$4,$5)").bind(Uuid::new_v4()).bind(v).bind(position).bind(format!("Synthetic option {position}")).bind(position==1 || (*typ=="multiple" && position==2)).execute(pool).await.unwrap();
            }
            sqlx::query("INSERT INTO assessment_bank_items(bank_version_id,question_version_id,position) VALUES($1,$2,$3)").bind(version).bind(v).bind(index as i32+1).execute(pool).await.unwrap();
        }
    }
    let path = Uuid::new_v4();
    sqlx::query("INSERT INTO learning_path_versions(id,version,paired_bank_version_id,quiz_bank_version_id) VALUES($1,1,$2,$3)").bind(path).bind(banks[0]).bind(banks[1]).execute(pool).await.unwrap();
    let mut lessons = vec![];
    for position in 1..=2 {
        let lesson = Uuid::new_v4();
        let version = Uuid::new_v4();
        lessons.push(lesson);
        sqlx::query(
            "INSERT INTO learning_lessons(id,state,draft_payload) VALUES($1,'published','{}')",
        )
        .bind(lesson)
        .execute(pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO learning_lesson_versions(id,lesson_id,version,title,description) VALUES($1,$2,1,'Synthetic lesson','Synthetic description')").bind(version).bind(lesson).execute(pool).await.unwrap();
        sqlx::query("INSERT INTO learning_sections(id,lesson_version_id,section_key,position,title,body,required) VALUES($1,$2,$3,1,'Synthetic section','Protected frozen body',true)").bind(Uuid::new_v4()).bind(version).bind(Uuid::new_v4()).execute(pool).await.unwrap();
        sqlx::query("INSERT INTO learning_path_items(path_version_id,lesson_version_id,position) VALUES($1,$2,$3)").bind(path).bind(version).bind(position).execute(pool).await.unwrap();
    }
    (school, lessons)
}
fn merge_cookie(old: &str, new: &[String]) -> String {
    let mut jar = std::collections::BTreeMap::new();
    for part in old
        .split(';')
        .map(str::trim)
        .chain(new.iter().map(|s| s.split(';').next().unwrap()))
    {
        if let Some((k, v)) = part.split_once('=') {
            jar.insert(k.to_owned(), v.to_owned());
        }
    }
    jar.into_iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("; ")
}
async fn enroll(app: &Router, school: Uuid) -> (String, String, Value) {
    let (cookie, token) = csrf(app).await;
    let (s, v, c) = call(
        app,
        "POST",
        "/api/v1/student/sessions",
        json!({"name":"Synthetic student","school_id":school,"form_date":"2026-11-15"}),
        &cookie,
        &token,
        "enroll",
    )
    .await;
    assert_eq!(s, 201, "{v}");
    (
        merge_cookie(&cookie, &c),
        v["data"]["csrf_token"].as_str().unwrap().into(),
        v["data"].clone(),
    )
}
async fn attempt(app: &Router, c: &str, t: &str, typ: &str) -> Value {
    let (s, v, _) = call(
        app,
        "POST",
        "/api/v1/student/attempts",
        json!({"assessment_type":typ}),
        c,
        t,
        typ,
    )
    .await;
    assert!([200, 201].contains(&s), "{s}: {v}");
    let id = v["data"]["attempt_id"].as_str().unwrap();
    let (s, v, _) = call(
        app,
        "GET",
        &format!("/api/v1/student/attempts/{id}"),
        json!(null),
        c,
        t,
        "",
    )
    .await;
    assert_eq!(s, 200, "{v}");
    v["data"].clone()
}
async fn answer_all(app: &Router, c: &str, t: &str, a: &Value, correct: bool) {
    for q in a["questions"].as_array().unwrap() {
        let ids = if correct {
            q["options"]
                .as_array()
                .unwrap()
                .iter()
                .take(if q["type"] == "multiple" { 2 } else { 1 })
                .map(|o| o["id"].clone())
                .collect::<Vec<_>>()
        } else {
            vec![q["options"][2]["id"].clone()]
        };
        let (s, v, _) = call(
            app,
            "PUT",
            &format!(
                "/api/v1/student/attempts/{}/answers/{}",
                a["id"].as_str().unwrap(),
                q["id"].as_str().unwrap()
            ),
            json!({"option_ids":ids,"expected_revision":0}),
            c,
            t,
            "",
        )
        .await;
        assert_eq!(s, 200, "{v}");
        assert_eq!(v["data"]["revision"], 1);
    }
}
async fn submit(app: &Router, c: &str, t: &str, a: &Value, key: &str) -> Value {
    let (s, v, _) = call(
        app,
        "POST",
        &format!(
            "/api/v1/student/attempts/{}/submission",
            a["id"].as_str().unwrap()
        ),
        json!({}),
        c,
        t,
        key,
    )
    .await;
    assert_eq!(s, 200, "{v}");
    v["data"].clone()
}
#[sqlx::test(migrations = "./migrations")]
async fn registration_replays_before_and_after_cookie_rotation(pool: PgPool) {
    let (school, _) = content(&pool).await;
    let app = app(pool.clone()).await;
    let (c, t) = csrf(&app).await;
    let body = json!({"name":"Ayu","school_id":school,"form_date":"2026-11-15"});
    let (s, first, cookies) = call(
        &app,
        "POST",
        "/api/v1/student/sessions",
        body.clone(),
        &c,
        &t,
        "same-browser-op",
    )
    .await;
    assert_eq!(s, 201, "{first}");
    let (s, lost, _) = call(
        &app,
        "POST",
        "/api/v1/student/sessions",
        body.clone(),
        &c,
        &t,
        "same-browser-op",
    )
    .await;
    assert_eq!(s, 201, "{lost}");
    assert_eq!(lost["data"]["enrollment"], first["data"]["enrollment"]);
    let rotated = merge_cookie(&c, &cookies);
    let rt = first["data"]["csrf_token"].as_str().unwrap();
    let (s, replay, _) = call(
        &app,
        "POST",
        "/api/v1/student/sessions",
        body.clone(),
        &rotated,
        rt,
        "same-browser-op",
    )
    .await;
    assert_eq!(s, 201, "{replay}");
    assert_eq!(replay["data"]["student"], first["data"]["student"]);
    let (s, v, _) = call(
        &app,
        "POST",
        "/api/v1/student/sessions",
        json!({"name":"Changed","school_id":school,"form_date":"2026-11-15"}),
        &rotated,
        rt,
        "same-browser-op",
    )
    .await;
    assert_eq!(s, 409, "{v}");
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM learning_enrollments")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    let cache: Vec<Value> = sqlx::query_scalar("SELECT response_body FROM platform_idempotency")
        .fetch_all(&pool)
        .await
        .unwrap();
    for v in cache {
        assert!(!v.to_string().contains("token"));
    }
}
#[sqlx::test(migrations = "./migrations")]
async fn full_persisted_student_sequence_and_foreign_ownership(pool: PgPool) {
    let (school, lessons) = content(&pool).await;
    let app = app(pool.clone()).await;
    let (c, t, _) = enroll(&app, school).await;
    let (s, v, _) = call(
        &app,
        "GET",
        &format!("/api/v1/student/lessons/{}", lessons[0]),
        json!(null),
        &c,
        &t,
        "",
    )
    .await;
    assert_eq!(s, 409, "{v}");
    assert!(!v.to_string().contains("Protected"));
    let (s, _, _) = call(
        &app,
        "POST",
        "/api/v1/student/attempts",
        json!({"assessment_type":"quiz"}),
        &c,
        &t,
        "locked",
    )
    .await;
    assert_eq!(s, 409);
    let pre = attempt(&app, &c, &t, "pre_test").await;
    assert!(!pre.to_string().contains("is_correct"));
    assert!(!pre.to_string().contains("scoring_rule"));
    assert!(pre["score"].is_null());
    let (other, ot, _) = enroll(&app, school).await;
    let (s, _, _) = call(
        &app,
        "GET",
        &format!("/api/v1/student/attempts/{}", pre["id"].as_str().unwrap()),
        json!(null),
        &other,
        &ot,
        "",
    )
    .await;
    assert_eq!(s, 404);
    let (s, _, _) = call(
        &app,
        "POST",
        &format!(
            "/api/v1/student/attempts/{}/submission",
            pre["id"].as_str().unwrap()
        ),
        json!({}),
        &c,
        &t,
        "missing",
    )
    .await;
    assert_eq!(s, 422);
    answer_all(&app, &c, &t, &pre, false).await;
    let result = submit(&app, &c, &t, &pre, "submit-pre").await;
    assert!(result["score"].is_null());
    for (i, lesson) in lessons.iter().enumerate() {
        if i == 0 {
            let (s, _, _) = call(
                &app,
                "GET",
                &format!("/api/v1/student/lessons/{}", lessons[1]),
                json!(null),
                &c,
                &t,
                "",
            )
            .await;
            assert_eq!(s, 409);
        }
        let path = format!("/api/v1/student/lessons/{lesson}");
        let (s, v, _) = call(&app, "GET", &path, json!(null), &c, &t, "").await;
        assert_eq!(s, 200, "{v}");
        let section = v["data"]["sections"][0]["id"].clone();
        let (s, _, _) = call(
            &app,
            "POST",
            &format!("{path}/completion"),
            json!({}),
            &c,
            &t,
            "",
        )
        .await;
        assert_eq!(s, 409);
        let (s, v, _) = call(
            &app,
            "POST",
            &format!("{path}/opened"),
            json!({"section_id":section}),
            &c,
            &t,
            "",
        )
        .await;
        assert_eq!(s, 200, "{v}");
        let (s, v, _) = call(
            &app,
            "POST",
            &format!("{path}/completion"),
            json!({}),
            &c,
            &t,
            "",
        )
        .await;
        assert_eq!(s, 200, "{v}");
    }
    let quiz = attempt(&app, &c, &t, "quiz").await;
    assert_ne!(quiz["snapshot_id"], pre["snapshot_id"]);
    answer_all(&app, &c, &t, &quiz, true).await;
    assert_eq!(
        submit(&app, &c, &t, &quiz, "quiz-submit").await["score"],
        100
    );
    let (s, _, _) = call(
        &app,
        "POST",
        "/api/v1/student/attempts",
        json!({"assessment_type":"post_test"}),
        &c,
        &t,
        "post-locked",
    )
    .await;
    assert_eq!(s, 409);
    for value in [0., -1., 0.00001, 100000000.] {
        let (s, v, _) = call(
            &app,
            "POST",
            "/api/v1/student/measurements",
            json!({"weight_kg":value,"height_cm":170}),
            &c,
            &t,
            "invalid",
        )
        .await;
        assert_eq!(s, 422, "{v}");
    }
    let (s, v, _) = call(
        &app,
        "POST",
        "/api/v1/student/measurements",
        json!({"weight_kg":60.5,"height_cm":170}),
        &c,
        &t,
        "measurement",
    )
    .await;
    assert_eq!(s, 201, "{v}");
    assert_eq!(v["data"]["bmi_display"], "20.9");
    let mid = v["data"]["id"].clone();
    let (s, v, _) = call(
        &app,
        "POST",
        "/api/v1/student/measurements",
        json!({"weight_kg":60.5,"height_cm":170}),
        &c,
        &t,
        "measurement",
    )
    .await;
    assert_eq!(s, 201, "{v}");
    assert_eq!(v["data"]["id"], mid);
    let equivalent: Value =
        serde_json::from_str(r#"{"weight_kg":60.50000,"height_cm":1.7e2}"#).unwrap();
    let (s, v, _) = call(
        &app,
        "POST",
        "/api/v1/student/measurements",
        equivalent,
        &c,
        &t,
        "measurement",
    )
    .await;
    assert_eq!(s, 201, "canonical numeric retry: {v}");
    assert_eq!(v["data"]["id"], mid);
    let post = attempt(&app, &c, &t, "post_test").await;
    assert_eq!(post["snapshot_id"], pre["snapshot_id"]);
    assert_eq!(post["questions"], pre["questions"]);
    answer_all(&app, &c, &t, &post, true).await;
    assert_eq!(
        submit(&app, &c, &t, &post, "post-submit").await["score"],
        100
    );
    let (s, v, _) = call(
        &app,
        "GET",
        "/api/v1/student/summary",
        json!(null),
        &c,
        &t,
        "",
    )
    .await;
    assert_eq!(s, 200, "{v}");
    assert_eq!(v["data"]["scores"]["pre_test"], 0);
    assert_eq!(v["data"]["scores"]["post_test"], 100);
    let (s, v, _) = call(
        &app,
        "GET",
        "/api/v1/student/measurements?limit=1&offset=0",
        json!(null),
        &c,
        &t,
        "",
    )
    .await;
    assert_eq!(s, 200, "{v}");
    assert_eq!(v["meta"]["total"], 1);
    let (s, _, _) = call(
        &app,
        "DELETE",
        "/api/v1/student/sessions/current",
        json!(null),
        &c,
        &t,
        "",
    )
    .await;
    assert_eq!(s, 204);
    let (s, _, _) = call(&app, "GET", "/api/v1/student/me", json!(null), &c, &t, "").await;
    assert_eq!(s, 401);
    let (s, _, _) = call(
        &app,
        "DELETE",
        "/api/v1/student/sessions/current",
        json!(null),
        &c,
        &t,
        "",
    )
    .await;
    assert_eq!(s, 204);
}

#[sqlx::test(migrations = "./migrations")]
async fn pinned_versions_reject_mutation_and_extension(pool: PgPool) {
    let (school, _) = content(&pool).await;
    let app = app(pool.clone()).await;
    let (c, t, _) = enroll(&app, school).await;
    let pre = attempt(&app, &c, &t, "pre_test").await;
    let version: Uuid =
        serde_json::from_value(pre["questions"][0]["question_version_id"].clone()).unwrap();
    assert!(
        sqlx::query("UPDATE assessment_question_versions SET text='Changed' WHERE id=$1")
            .bind(version)
            .execute(&pool)
            .await
            .is_err(),
        "pinned historical text was mutable"
    );
    assert!(sqlx::query("INSERT INTO assessment_options(id,question_version_id,position,text,is_correct) VALUES($1,$2,99,'late',false)").bind(Uuid::new_v4()).bind(version).execute(&pool).await.is_err(),"pinned choices could be extended");
    assert!(
        sqlx::query("UPDATE learning_sections SET body='Changed'")
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("DELETE FROM learning_path_items")
            .execute(&pool)
            .await
            .is_err()
    );
    // Draft roots may change freely; the pinned response must remain byte-for-byte safe content.
    sqlx::query(
        "UPDATE assessment_questions SET draft_payload='{\"new\":\"future\"}',revision=revision+1",
    )
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(
        attempt(&app, &c, &t, "pre_test").await["questions"],
        pre["questions"]
    );
}

#[test]
fn bmi_exact_boundaries_precision_and_rounding() {
    use imt_smart_teens_backend::modules::anthropometry::domain::{Bmi, PositiveInput};
    for invalid in [
        "0",
        "-1",
        "NaN",
        "Infinity",
        "0.00001",
        "100000000",
        "99999999.99999",
        "1e-2147483648",
    ] {
        assert!(
            PositiveInput::parse_text(invalid).is_err(),
            "accepted {invalid}"
        );
    }
    for valid in [
        "0.0001",
        "99999999.9999",
        "1e-4",
        "100e-2",
        "1.000000",
        "00001.00000",
    ] {
        assert!(PositiveInput::parse_text(valid).is_ok(), "rejected {valid}");
    }
    let w = PositiveInput::parse_text("60.5").unwrap();
    let h = PositiveInput::parse_text("170").unwrap();
    let bmi = Bmi::calculate(w, h).unwrap();
    assert_eq!(bmi.sql(), "20.934256055363");
    assert_eq!(bmi.display(), "20.9");
    let bmi = Bmi::calculate(
        PositiveInput::parse_text("20.95").unwrap(),
        PositiveInput::parse_text("100").unwrap(),
    )
    .unwrap();
    assert_eq!(bmi.display(), "21.0");
    assert_eq!(
        Bmi::calculate(
            PositiveInput::parse_text("99999999.9999").unwrap(),
            PositiveInput::parse_text("0.0001").unwrap()
        )
        .unwrap()
        .sql(),
        "99999999999900000000.000000000000"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn autosave_clear_stale_revision_and_simultaneous_submission(pool: PgPool) {
    let (school, _) = content(&pool).await;
    let app = app(pool.clone()).await;
    let (c, t, _) = enroll(&app, school).await;
    let a = attempt(&app, &c, &t, "pre_test").await;
    let id = a["id"].as_str().unwrap();
    let q = &a["questions"][0];
    let path = format!(
        "/api/v1/student/attempts/{id}/answers/{}",
        q["id"].as_str().unwrap()
    );
    let input = json!({"option_ids":[q["options"][0]["id"]],"expected_revision":0});
    let (one, two) = tokio::join!(
        call(&app, "PUT", &path, input.clone(), &c, &t, ""),
        call(&app, "PUT", &path, input, &c, &t, "")
    );
    let mut statuses = [one.0, two.0];
    statuses.sort();
    assert_eq!(statuses, [200, 409]);
    let conflict = if one.0 == 409 { one.1 } else { two.1 };
    assert_eq!(conflict["error"]["code"], "ANSWER_REVISION_CONFLICT");
    assert_eq!(conflict["error"]["details"]["latest_answer"]["revision"], 1);
    let (s, v, _) = call(
        &app,
        "PUT",
        &path,
        json!({"option_ids":[],"expected_revision":1}),
        &c,
        &t,
        "",
    )
    .await;
    assert_eq!(s, 200, "{v}");
    assert_eq!(v["data"]["revision"], 2);
    assert_eq!(v["data"]["option_ids"], json!([]));
    for ids in [
        json!([q["options"][0]["id"], q["options"][0]["id"]]),
        json!([Uuid::new_v4()]),
        json!([q["options"][0]["id"], q["options"][1]["id"]]),
    ] {
        assert_eq!(
            call(
                &app,
                "PUT",
                &path,
                json!({"option_ids":ids,"expected_revision":2}),
                &c,
                &t,
                ""
            )
            .await
            .0,
            422
        );
    }
    let submission = format!("/api/v1/student/attempts/{id}/submission");
    assert_eq!(
        call(&app, "POST", &submission, json!({}), &c, &t, "missing")
            .await
            .0,
        422
    );
    let points: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM assessment_answers WHERE earned_points IS NOT NULL",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(points, 0);
    assert_eq!(
        call(
            &app,
            "PUT",
            &path,
            json!({"option_ids":[q["options"][0]["id"]],"expected_revision":2}),
            &c,
            &t,
            ""
        )
        .await
        .0,
        200
    );
    let q = &a["questions"][1];
    assert_eq!(call(&app,"PUT",&format!("/api/v1/student/attempts/{id}/answers/{}",q["id"].as_str().unwrap()),json!({"option_ids":[q["options"][0]["id"],q["options"][1]["id"]],"expected_revision":0}),&c,&t,"").await.0,200);
    let (one, two) = tokio::join!(
        call(&app, "POST", &submission, json!({}), &c, &t, "submit1"),
        call(&app, "POST", &submission, json!({}), &c, &t, "submit2")
    );
    assert_eq!(one.0, 200, "{}", one.1);
    assert_eq!(two.0, 200, "{}", two.1);
    assert_eq!(one.1, two.1);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM platform_audit_events WHERE action='attempt.submit'"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
    assert_eq!(
        call(
            &app,
            "PUT",
            &path,
            json!({"option_ids":[],"expected_revision":3}),
            &c,
            &t,
            ""
        )
        .await
        .0,
        409
    );
}
#[sqlx::test(migrations = "./migrations")]
async fn save_and_submit_serialize_on_attempt_lock(pool: PgPool) {
    let (school, _) = content(&pool).await;
    let app = app(pool.clone()).await;
    let (c, t, _) = enroll(&app, school).await;
    let a = attempt(&app, &c, &t, "pre_test").await;
    answer_all(&app, &c, &t, &a, true).await;
    let id: Uuid = serde_json::from_value(a["id"].clone()).unwrap();
    let q = &a["questions"][0];
    let path = format!(
        "/api/v1/student/attempts/{id}/answers/{}",
        q["id"].as_str().unwrap()
    );
    let submission = format!("/api/v1/student/attempts/{id}/submission");
    let mut held = pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM assessment_attempts WHERE id=$1 FOR UPDATE")
        .bind(id)
        .execute(&mut *held)
        .await
        .unwrap();
    let request_app = app.clone();
    let cookie = c.clone();
    let token = t.clone();
    let input = json!({"option_ids":[q["options"][2]["id"]],"expected_revision":1});
    let save =
        tokio::spawn(
            async move { call(&request_app, "PUT", &path, input, &cookie, &token, "").await },
        );
    let request_app = app.clone();
    let cookie = c.clone();
    let token = t.clone();
    let submit = tokio::spawn(async move {
        call(
            &request_app,
            "POST",
            &submission,
            json!({}),
            &cookie,
            &token,
            "race",
        )
        .await
    });
    wait_for_blocked(&pool, "assessment_attempts").await;
    held.rollback().await.unwrap();
    let saved = save.await.unwrap();
    let submitted = submit.await.unwrap();
    assert_eq!(submitted.0, 200, "{}", submitted.1);
    assert!([200, 409].contains(&saved.0), "{}", saved.1);
    let score: i32 =
        sqlx::query_scalar("SELECT score_percent::integer FROM assessment_attempts WHERE id=$1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(score, if saved.0 == 200 { 50 } else { 100 });
}
async fn wait_for_blocked(pool: &PgPool, table: &str) {
    tokio::time::timeout(std::time::Duration::from_secs(3),async{
        loop {
            let count:i64=sqlx::query_scalar("SELECT count(*) FROM pg_stat_activity WHERE datname=current_database() AND pid<>pg_backend_pid() AND wait_event_type='Lock' AND query LIKE $1").bind(format!("%{table}%")).fetch_one(pool).await.unwrap();
            if count>0{break;}tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }).await.expect("request never reached the real PostgreSQL lock");
}
#[sqlx::test(migrations = "./migrations")]
async fn enrollment_is_atomic_when_content_or_audit_is_unavailable(pool: PgPool) {
    let school = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO identity_schools(id,name,normalized_name) VALUES($1,'Temporary','temporary')",
    )
    .bind(school)
    .execute(&pool)
    .await
    .unwrap();
    let app = app(pool.clone()).await;
    let (c, t) = csrf(&app).await;
    let body = json!({"name":"Synthetic","school_id":school,"form_date":"2026-11-15"});
    let (s, v, cookies) = call(
        &app,
        "POST",
        "/api/v1/student/sessions",
        body,
        &c,
        &t,
        "no-content",
    )
    .await;
    assert_eq!(s, 409, "{v}");
    assert!(cookies.is_empty());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM identity_students")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    let (school, _) = content(&pool).await;
    sqlx::raw_sql("CREATE FUNCTION fail_student_audit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'synthetic failure'; END; $$; CREATE TRIGGER fail_student_audit BEFORE INSERT ON platform_audit_events FOR EACH ROW EXECUTE FUNCTION fail_student_audit();").execute(&pool).await.unwrap();
    let (s, v, cookies) = call(
        &app,
        "POST",
        "/api/v1/student/sessions",
        json!({"name":"Synthetic","school_id":school,"form_date":"2026-11-15"}),
        &c,
        &t,
        "rollback",
    )
    .await;
    assert_eq!(s, 500, "{v}");
    assert!(cookies.is_empty());
    for table in [
        "identity_students",
        "learning_enrollments",
        "assessment_snapshots",
        "assessment_attempts",
        "identity_sessions",
        "platform_idempotency",
    ] {
        assert_eq!(
            sqlx::query_scalar::<_, i64>(&format!("SELECT count(*) FROM {table}"))
                .fetch_one(&pool)
                .await
                .unwrap(),
            0,
            "partial {table}"
        );
    }
}
#[sqlx::test(migrations = "./migrations")]
async fn archive_racing_enrollment_serializes_and_preserves_history(pool: PgPool) {
    let (school, _) = content(&pool).await;
    let app = app(pool.clone()).await;
    let (existing, et, original) = enroll(&app, school).await;
    let mut archive = pool.begin().await.unwrap();
    sqlx::query("UPDATE identity_schools SET archived_at=now(),name='Archived renamed',revision=revision+1 WHERE id=$1").bind(school).execute(&mut *archive).await.unwrap();
    let (c, t) = csrf(&app).await;
    let request_app = app.clone();
    let registration = tokio::spawn(async move {
        call(
            &request_app,
            "POST",
            "/api/v1/student/sessions",
            json!({"name":"Racing","school_id":school,"form_date":"2026-11-15"}),
            &c,
            &t,
            "archive-race",
        )
        .await
    });
    wait_for_blocked(&pool, "identity_schools").await;
    archive.commit().await.unwrap();
    let (s, v, cookies) = registration.await.unwrap();
    assert_eq!(s, 422, "{v}");
    assert_eq!(v["error"]["code"], "SCHOOL_NOT_AVAILABLE");
    assert!(cookies.is_empty());
    let (s, v, _) = call(
        &app,
        "GET",
        "/api/v1/student/me",
        json!(null),
        &existing,
        &et,
        "",
    )
    .await;
    assert_eq!(s, 200, "{v}");
    assert_eq!(v["data"]["enrollment"], original["enrollment"]);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM identity_students")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn enrollment_idempotency_hashes_validated_name(pool: PgPool) {
    let (school, _) = content(&pool).await;
    let app = app(pool.clone()).await;
    let (c, t) = csrf(&app).await;
    let (s, one, _) = call(
        &app,
        "POST",
        "/api/v1/student/sessions",
        json!({"name":"  Ayu   Putri  ","school_id":school,"form_date":"2026-11-15"}),
        &c,
        &t,
        "canonical",
    )
    .await;
    assert_eq!(s, 201, "{one}");
    let (s, two, _) = call(
        &app,
        "POST",
        "/api/v1/student/sessions",
        json!({"name":"Ayu Putri","school_id":school,"form_date":"2026-11-15"}),
        &c,
        &t,
        "canonical",
    )
    .await;
    assert_eq!(s, 201, "{two}");
    assert_eq!(one["data"]["student"], two["data"]["student"]);
}
#[sqlx::test(migrations = "./migrations")]
async fn student_registration_preserves_admin_session_and_rotates_student(pool: PgPool) {
    let (school, _) = content(&pool).await;
    let admin_id = admin(&pool, true).await;
    let app = app(pool.clone()).await;
    let (ac, at) = login(&app, admin_id).await;
    let (s, v, extra) = call(&app, "GET", "/api/v1/csrf", json!(null), &ac, &at, "").await;
    assert_eq!(s, 200);
    let jar = merge_cookie(&ac, &extra);
    let token = v["data"]["token"].as_str().unwrap();
    let (s, v, cookies) = call(
        &app,
        "POST",
        "/api/v1/student/sessions",
        json!({"name":"Student","school_id":school,"form_date":"2026-11-15"}),
        &jar,
        token,
        "first",
    )
    .await;
    assert_eq!(s, 201, "{v}");
    let jar = merge_cookie(&jar, &cookies);
    let token = v["data"]["csrf_token"].as_str().unwrap();
    let old_student_cookie = jar.clone();
    let (s, v, cookies) = call(
        &app,
        "POST",
        "/api/v1/student/sessions",
        json!({"name":"Second","school_id":school,"form_date":"2026-11-15"}),
        &jar,
        token,
        "second",
    )
    .await;
    assert_eq!(s, 201, "{v}");
    let jar = merge_cookie(&jar, &cookies);
    let token = v["data"]["csrf_token"].as_str().unwrap();
    assert_eq!(
        call(
            &app,
            "GET",
            "/api/v1/admin/me",
            json!(null),
            &jar,
            token,
            ""
        )
        .await
        .0,
        200
    );
    assert_eq!(
        call(
            &app,
            "GET",
            "/api/v1/student/me",
            json!(null),
            &old_student_cookie,
            token,
            ""
        )
        .await
        .0,
        401
    );
    let (s, v, _) = call(
        &app,
        "POST",
        "/api/v1/admin/schools",
        json!({"name":"Coexisting admin mutation"}),
        &jar,
        token,
        "",
    )
    .await;
    assert_eq!(s, 201, "{v}");
}
#[sqlx::test(migrations = "./migrations")]
async fn simultaneous_registration_replay_has_one_identity(pool: PgPool) {
    let (school, _) = content(&pool).await;
    let app = app(pool.clone()).await;
    let (c, t) = csrf(&app).await;
    let body = json!({"name":"Concurrent","school_id":school,"form_date":"2026-11-15"});
    let (one, two) = tokio::join!(
        call(
            &app,
            "POST",
            "/api/v1/student/sessions",
            body.clone(),
            &c,
            &t,
            "same"
        ),
        call(
            &app,
            "POST",
            "/api/v1/student/sessions",
            body,
            &c,
            &t,
            "same"
        )
    );
    assert_eq!(one.0, 201, "{}", one.1);
    assert_eq!(two.0, 201, "{}", two.1);
    assert_eq!(one.1["data"]["enrollment"], two.1["data"]["enrollment"]);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM identity_students")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
}
