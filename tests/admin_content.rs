//! Real PostgreSQL editor contracts: draft roots are separate from immutable publications.
mod common;
use common::*;
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

async fn editor(pool: &PgPool) -> (axum::Router, String, String) {
    let id = admin(pool, false).await;
    sqlx::query("UPDATE identity_admins SET can_manage_content=true WHERE id=$1")
        .bind(id)
        .execute(pool)
        .await
        .unwrap();
    let app = app(pool.clone()).await;
    let (c, t) = login(&app, id).await;
    (app, c, t)
}
fn question(text: &str, typ: &str, position: i32) -> Value {
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    json!({"text":text,"type":typ,"options":[{"id":a,"text":"One"},{"id":b,"text":"Two"}],"correct_option_ids":if typ=="single"{vec![a]}else{vec![a,b]},"position":position,"required":true})
}
#[sqlx::test(migrations = "./migrations")]
async fn questions_bank_publication_versions_keys_and_revisions(pool: PgPool) {
    roots(&pool).await;
    let (app, c, t) = editor(&pool).await;
    let (s, v, _) = request(
        &app,
        "GET",
        "/api/v1/admin/question-banks",
        json!(null),
        &c,
        &t,
    )
    .await;
    assert_eq!(s, 200, "banks list: {v}");
    assert_eq!(v["meta"]["total"], 2);
    let bank = v["data"][0]["id"].as_str().unwrap();
    let base = format!("/api/v1/admin/question-banks/{bank}");
    assert_eq!(
        request(
            &app,
            "POST",
            &format!("{base}/publication"),
            json!({"expected_revision":1}),
            &c,
            &t
        )
        .await
        .0,
        422
    );
    let draft = question("Original", "single", 1);
    let (s, q, _) = request(
        &app,
        "POST",
        &format!("{base}/questions"),
        draft.clone(),
        &c,
        &t,
    )
    .await;
    assert_eq!(s, 201, "question create: {q}");
    let id = q["data"]["id"].as_str().unwrap();
    let qpath = format!("{base}/questions/{id}");
    let (_, banks, _) = request(
        &app,
        "GET",
        "/api/v1/admin/question-banks",
        json!(null),
        &c,
        &t,
    )
    .await;
    assert_eq!(banks["data"][0]["revision"], 2);
    let (s, b, _) = request(
        &app,
        "POST",
        &format!("{base}/publication"),
        json!({"expected_revision":2}),
        &c,
        &t,
    )
    .await;
    assert_eq!(s, 200, "bank publish: {b}");
    assert_eq!(b["data"]["revision"], 3);
    assert_eq!(
        request(
            &app,
            "POST",
            &format!("{base}/publication"),
            json!({"expected_revision":2}),
            &c,
            &t
        )
        .await
        .0,
        409
    );
    let (_, first, _) = request(&app, "GET", &qpath, json!(null), &c, &t).await;
    let v1 = first["data"]["versions"][0]["id"].clone();
    assert_eq!(first["data"]["revision"], 2);
    let mut changed = draft.clone();
    changed["text"] = json!("Future");
    changed["expected_revision"] = json!(2);
    assert_eq!(
        request(&app, "PUT", &qpath, changed.clone(), &c, &t)
            .await
            .0,
        200
    );
    assert_eq!(request(&app, "PUT", &qpath, changed, &c, &t).await.0, 409);
    let (s, b, _) = request(
        &app,
        "POST",
        &format!("{base}/publication"),
        json!({"expected_revision":4}),
        &c,
        &t,
    )
    .await;
    assert_eq!(s, 200, "republish: {b}");
    let (_, now, _) = request(&app, "GET", &qpath, json!(null), &c, &t).await;
    assert_eq!(now["data"]["versions"].as_array().unwrap().len(), 2);
    assert_eq!(now["data"]["draft"]["text"], "Future");
    let v2 = now["data"]["versions"][0]["id"].clone();
    let published_text: String =
        sqlx::query_scalar("SELECT text FROM assessment_question_versions WHERE id=$1")
            .bind(serde_json::from_value::<Uuid>(v2.clone()).unwrap())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(published_text, "Future");
    assert_ne!(v1, v2);
    let ids:Vec<(Uuid,Uuid,bool)>=sqlx::query_as("SELECT id,question_version_id,is_correct FROM assessment_options ORDER BY question_version_id,position").fetch_all(&pool).await.unwrap();
    assert_eq!(ids.len(), 4);
    assert_eq!(
        ids.iter()
            .map(|x| x.0)
            .collect::<std::collections::HashSet<_>>()
            .len(),
        4
    );
    assert!(!ids.iter().any(|x| json!(x.0) == draft["options"][0]["id"]));
    for v in [v1.clone(), v2] {
        let v: Uuid = serde_json::from_value(v).unwrap();
        assert_eq!(ids.iter().filter(|x| x.1 == v && x.2).count(), 1);
    }
    let text: String =
        sqlx::query_scalar("SELECT text FROM assessment_question_versions WHERE id=$1")
            .bind(serde_json::from_value::<Uuid>(v1).unwrap())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(text, "Original");
    let (s, v, _) = request(
        &app,
        "POST",
        &format!("{qpath}/archive"),
        json!({"expected_revision":4}),
        &c,
        &t,
    )
    .await;
    assert_eq!(s, 200, "{v}");
    let (_, v, _) = request(
        &app,
        "GET",
        &format!("{base}/questions?status=archived&limit=1"),
        json!(null),
        &c,
        &t,
    )
    .await;
    assert_eq!(v["meta"]["total"], 1);
    assert_eq!(
        request(
            &app,
            "POST",
            &format!("{base}/publication"),
            json!({"expected_revision":6}),
            &c,
            &t
        )
        .await
        .0,
        422
    );
}
#[sqlx::test(migrations = "./migrations")]
async fn question_validation_bank_ownership_and_transactional_publication(pool: PgPool) {
    roots(&pool).await;
    let (app, c, t) = editor(&pool).await;
    let (status, banks, _) = request(
        &app,
        "GET",
        "/api/v1/admin/question-banks",
        json!(null),
        &c,
        &t,
    )
    .await;
    assert_eq!(status, 200, "banks: {banks}");
    let bank = banks["data"][0]["id"].as_str().unwrap();
    let other = banks["data"][1]["id"].as_str().unwrap();
    let base = format!("/api/v1/admin/question-banks/{bank}");
    for invalid in [
        "empty",
        "duplicate",
        "foreign",
        "single",
        "blank",
        "required",
        "html",
    ] {
        let mut q = question("Question", "single", 1);
        match invalid {
            "empty" => q["options"] = json!([]),
            "duplicate" => q["options"][1]["id"] = q["options"][0]["id"].clone(),
            "foreign" => q["correct_option_ids"] = json!([Uuid::new_v4()]),
            "single" => {
                q["correct_option_ids"] = json!([q["options"][0]["id"], q["options"][1]["id"]])
            }
            "blank" => q["options"][0]["text"] = json!(" "),
            "required" => q["required"] = json!(false),
            _ => q["text"] = json!("<script>bad</script>"),
        }
        assert_eq!(
            request(&app, "POST", &format!("{base}/questions"), q, &c, &t)
                .await
                .0,
            422,
            "accepted {invalid}"
        );
    }
    let (_, q, _) = request(
        &app,
        "POST",
        &format!("{base}/questions"),
        question("Valid", "multiple", 1),
        &c,
        &t,
    )
    .await;
    let id = q["data"]["id"].as_str().unwrap();
    assert_eq!(
        request(
            &app,
            "GET",
            &format!("/api/v1/admin/question-banks/{other}/questions/{id}"),
            json!(null),
            &c,
            &t
        )
        .await
        .0,
        404
    );
    sqlx::raw_sql("CREATE FUNCTION fail_bank_audit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.action='bank.publish' THEN RAISE EXCEPTION 'synthetic'; END IF; RETURN NEW; END $$; CREATE TRIGGER fail_bank_audit BEFORE INSERT ON platform_audit_events FOR EACH ROW EXECUTE FUNCTION fail_bank_audit()").execute(&pool).await.unwrap();
    assert_eq!(
        request(
            &app,
            "POST",
            &format!("{base}/publication"),
            json!({"expected_revision":2}),
            &c,
            &t
        )
        .await
        .0,
        500
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM assessment_question_versions")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM assessment_bank_versions")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT revision FROM assessment_banks WHERE id=$1")
            .bind(bank.parse::<Uuid>().unwrap())
            .fetch_one(&pool)
            .await
            .unwrap(),
        2
    );
}
async fn roots(pool: &PgPool) {
    for kind in ["paired", "quiz"] {
        sqlx::query("INSERT INTO assessment_banks(id,kind,state) VALUES($1,$2,'draft')")
            .bind(Uuid::new_v4())
            .bind(kind)
            .execute(pool)
            .await
            .unwrap();
    }
    sqlx::query("INSERT INTO learning_path_drafts(id,lesson_ids,paired_bank_id,quiz_bank_id) SELECT $1,'{}',p.id,q.id FROM assessment_banks p,assessment_banks q WHERE p.kind='paired' AND q.kind='quiz'").bind(Uuid::new_v4()).execute(pool).await.unwrap();
}
fn lesson(title: &str, position: i32) -> Value {
    json!({"title":title,"description":"Description","sections":[{"title":"Section","body":"Plain text body","required":true}],"position":position})
}
#[sqlx::test(migrations = "./migrations")]
async fn explicit_dev_init_is_idempotent_preserves_drafts_and_refuses_production(pool: PgPool) {
    use sqlx::ConnectOptions;
    let url = pool.connect_options().to_url_lossy().to_string();
    let run = |environment: &str| {
        std::process::Command::new(env!("CARGO_BIN_EXE_imt-smart-teens-backend"))
            .arg("init-content-config")
            .env("APP_ENV", environment)
            .env("DATABASE_URL", &url)
            .env(
                "CSRF_SIGNING_KEY",
                "synthetic-test-key-not-for-deployment-000000000000",
            )
            .env("SESSION_COOKIE_SECURE", "true")
            .env("CORS_ALLOWED_ORIGINS", "https://example.invalid")
            .output()
            .unwrap()
    };
    assert!(!run("production").status.success());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM assessment_banks")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    assert!(
        run("development").status.success(),
        "development config initialization failed"
    );
    assert!(run("development").status.success());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM assessment_banks")
            .fetch_one(&pool)
            .await
            .unwrap(),
        2
    );
    let original: (Uuid, i64) = sqlx::query_as("SELECT id,revision FROM learning_path_drafts")
        .fetch_one(&pool)
        .await
        .unwrap();
    let (app, c, t) = editor(&pool).await;
    assert_eq!(
        request(
            &app,
            "POST",
            "/api/v1/admin/lessons",
            lesson("Preserved", 1),
            &c,
            &t
        )
        .await
        .0,
        201
    );
    assert!(run("development").status.success());
    let after: (Uuid, i64, i64) = sqlx::query_as(
        "SELECT id,revision,cardinality(lesson_ids)::bigint FROM learning_path_drafts",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!((after.0, after.1, after.2), (original.0, original.1 + 1, 1));
    for table in [
        "learning_path_versions",
        "learning_lesson_versions",
        "assessment_bank_versions",
        "identity_students",
    ] {
        assert_eq!(
            sqlx::query_scalar::<_, i64>(&format!("SELECT count(*) FROM {table}"))
                .fetch_one(&pool)
                .await
                .unwrap(),
            0
        );
    }
}
#[sqlx::test(migrations = "./migrations")]
async fn lesson_draft_publish_edit_order_archive_are_revisioned_and_historical(pool: PgPool) {
    roots(&pool).await;
    let (app, c, t) = editor(&pool).await;
    let (s, v, _) = request(
        &app,
        "POST",
        "/api/v1/admin/lessons",
        lesson("First", 1),
        &c,
        &t,
    )
    .await;
    assert_eq!(s, 201, "draft create: {v}");
    let id = v["data"]["id"].as_str().unwrap();
    assert_eq!(v["data"]["revision"], 1);
    let base = format!("/api/v1/admin/lessons/{id}");
    let (s, p, _) = request(
        &app,
        "POST",
        &format!("{base}/publication"),
        json!({"expected_revision":1}),
        &c,
        &t,
    )
    .await;
    assert_eq!(s, 200, "publish: {p}");
    assert_eq!(p["data"]["versions"][0]["version"], 1);
    let first_version = p["data"]["versions"][0]["id"].clone();
    let mut changed = lesson("Changed", 1);
    changed["expected_revision"] = json!(2);
    let (s, v, _) = request(&app, "PUT", &base, changed.clone(), &c, &t).await;
    assert_eq!(s, 200, "edit: {v}");
    assert_eq!(v["data"]["state"], "draft");
    assert_eq!(v["data"]["draft"]["title"], "Changed");
    assert_eq!(request(&app, "PUT", &base, changed, &c, &t).await.0, 409);
    let (s, v, _) = request(&app, "GET", &base, json!(null), &c, &t).await;
    assert_eq!(s, 200, "{v}");
    assert_eq!(v["data"]["versions"][0]["id"], first_version);
    let (s, v, _) = request(
        &app,
        "POST",
        "/api/v1/admin/lessons",
        lesson("Second", 2),
        &c,
        &t,
    )
    .await;
    assert_eq!(s, 201, "{v}");
    let second = v["data"]["id"].clone();
    let (s, path, _) = request(
        &app,
        "GET",
        "/api/v1/admin/learning-path",
        json!(null),
        &c,
        &t,
    )
    .await;
    assert_eq!(s, 200, "{path}");
    let rev = path["data"]["revision"].clone();
    let order = "/api/v1/admin/learning-path/order";
    assert_eq!(
        request(
            &app,
            "PUT",
            order,
            json!({"lesson_ids":[id,id],"expected_revision":rev}),
            &c,
            &t
        )
        .await
        .0,
        422
    );
    let (s, v, _) = request(
        &app,
        "PUT",
        order,
        json!({"lesson_ids":[second,id],"expected_revision":rev}),
        &c,
        &t,
    )
    .await;
    assert_eq!(s, 200, "{v}");
    let mut movement = lesson("Changed", 1);
    movement["expected_revision"] = json!(3);
    let (s, v, _) = request(&app, "PUT", &base, movement, &c, &t).await;
    assert_eq!(s, 409, "position must use explicit reorder: {v}");
    assert_eq!(v["error"]["code"], "USE_PATH_ORDER");
    assert!(
        v["error"]["message"].as_str().unwrap().contains("urutan"),
        "must direct the editor to explicit ordering"
    );
    let (s, v, _) = request(
        &app,
        "POST",
        &format!("{base}/archive"),
        json!({"expected_revision":3}),
        &c,
        &t,
    )
    .await;
    assert_eq!(s, 200, "{v}");
    assert_eq!(v["data"]["state"], "archived");
    let (_, v, _) = request(
        &app,
        "GET",
        "/api/v1/admin/lessons?status=archived&limit=1",
        json!(null),
        &c,
        &t,
    )
    .await;
    assert_eq!(v["meta"]["total"], 1);
    assert_eq!(v["data"][0]["id"], id);
    let frozen: String =
        sqlx::query_scalar("SELECT title FROM learning_lesson_versions WHERE id=$1")
            .bind(serde_json::from_value::<Uuid>(first_version).unwrap())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(frozen, "First");
}
#[sqlx::test(migrations = "./migrations")]
async fn content_configuration_missing_permissions_invalid_publication_and_audit_rollback(
    pool: PgPool,
) {
    let (app, c, t) = editor(&pool).await;
    let (s, v, _) = request(
        &app,
        "GET",
        "/api/v1/admin/learning-path",
        json!(null),
        &c,
        &t,
    )
    .await;
    assert_eq!(s, 409, "missing configuration must be explicit: {v}");
    roots(&pool).await;
    let mut empty = lesson("Draft", 1);
    empty["sections"] = json!([]);
    let (s, v, _) = request(&app, "POST", "/api/v1/admin/lessons", empty, &c, &t).await;
    assert_eq!(s, 201, "incomplete lesson may be saved as draft: {v}");
    let id = v["data"]["id"].as_str().unwrap();
    assert_eq!(
        request(
            &app,
            "POST",
            &format!("/api/v1/admin/lessons/{id}/publication"),
            json!({"expected_revision":1}),
            &c,
            &t
        )
        .await
        .0,
        422
    );
    let other = admin(&pool, true).await;
    let (oc, ot) = login(&app, other).await;
    assert_eq!(
        request(&app, "GET", "/api/v1/admin/lessons", json!(null), &oc, &ot)
            .await
            .0,
        403
    );
    sqlx::raw_sql("CREATE FUNCTION fail_cms_audit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'synthetic'; END $$; CREATE TRIGGER fail_cms_audit BEFORE INSERT ON platform_audit_events FOR EACH ROW EXECUTE FUNCTION fail_cms_audit()").execute(&pool).await.unwrap();
    let (s, _, _) = request(
        &app,
        "POST",
        "/api/v1/admin/lessons",
        lesson("Rollback", 2),
        &c,
        &t,
    )
    .await;
    assert_eq!(s, 500);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM learning_lessons")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT revision FROM learning_path_drafts")
            .fetch_one(&pool)
            .await
            .unwrap(),
        2
    );
}
