//! Real HTTP characterization of the editor wire schema while transport gains static DTOs.
#[path = "fixtures/cms.rs"]
mod cms;
mod common;
use common::*;
use serde_json::{Value, json};
use sqlx::PgPool;
use std::collections::BTreeSet;

fn keys(value: &Value, expected: &[&str]) {
    assert_eq!(
        value
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>(),
        expected.iter().copied().collect()
    );
}
fn history(value: &Value, number: i64) {
    keys(value, &["id", "version", "published_at"]);
    assert!(value["id"].as_str().unwrap().parse::<uuid::Uuid>().is_ok());
    assert_eq!(value["version"].as_i64(), Some(number));
    assert!(chrono::DateTime::parse_from_rfc3339(value["published_at"].as_str().unwrap()).is_ok());
}
#[sqlx::test(migrations = "./migrations")]
async fn editors_keep_nested_drafts_histories_publication_and_nullable_manifest_schema(
    pool: PgPool,
) {
    cms::initialize(&pool).await;
    let (app, c, t, _) = cms::editor(&pool).await;
    let (s, p, _) = request(
        &app,
        "GET",
        "/api/v1/admin/learning-path",
        json!(null),
        &c,
        &t,
    )
    .await;
    assert_eq!(s, 200);
    keys(&p, &["data"]);
    assert!(p["data"].as_object().unwrap().contains_key("published"));
    assert_eq!(p["data"]["published"], Value::Null);
    let content = cms::publish(&app, &c, &t).await;
    let l = &content.lesson;
    keys(
        l,
        &["id", "state", "revision", "position", "draft", "versions"],
    );
    keys(&l["draft"], &["title", "description", "sections", "quiz"]);
    assert_eq!(l["draft"]["quiz"]["options"].as_array().unwrap().len(), 2);
    assert!(l["draft"]["quiz"]["correct_option_id"].is_string());
    keys(
        &l["draft"]["sections"][0],
        &["id", "title", "body", "required"],
    );
    assert_eq!(l["draft"]["sections"][0]["required"], true);
    history(&l["versions"][0], 1);
    for b in &content.banks {
        keys(b, &["id", "kind", "state", "revision", "versions"]);
        history(&b["versions"][0], 1);
    }
    let q = &content.questions[0];
    let (s, q, _) = request(
        &app,
        "GET",
        &format!(
            "/api/v1/admin/question-banks/{}/questions/{}",
            q["bank_id"].as_str().unwrap(),
            q["id"].as_str().unwrap()
        ),
        json!(null),
        &c,
        &t,
    )
    .await;
    assert_eq!(s, 200);
    keys(&q, &["data"]);
    keys(
        &q["data"],
        &[
            "id", "bank_id", "state", "revision", "position", "draft", "versions",
        ],
    );
    keys(
        &q["data"]["draft"],
        &["text", "type", "options", "correct_option_ids", "required"],
    );
    keys(&q["data"]["draft"]["options"][0], &["id", "text"]);
    assert_eq!(q["data"]["draft"]["type"], "single");
    assert_eq!(
        q["data"]["draft"]["correct_option_ids"][0],
        q["data"]["draft"]["options"][0]["id"]
    );
    history(&q["data"]["versions"][0], 1);
    let p = &content.path;
    keys(
        p,
        &[
            "id",
            "revision",
            "lesson_ids",
            "paired_bank_id",
            "quiz_bank_id",
            "versions",
            "published",
        ],
    );
    history(&p["versions"][0], 1);
    let manifest = &p["published"];
    keys(
        manifest,
        &[
            "id",
            "version",
            "paired_bank_version_id",
            "quiz_bank_version_id",
            "lessons",
        ],
    );
    keys(
        &manifest["lessons"][0],
        &["lesson_id", "lesson_version_id", "position"],
    );
    assert_eq!(manifest["id"], p["versions"][0]["id"]);
    assert_eq!(
        manifest["lessons"][0]["lesson_version_id"],
        l["versions"][0]["id"]
    );
    assert_eq!(manifest["lessons"][0]["position"].as_i64(), Some(1));
    let (s, newer, _) = request(
        &app,
        "POST",
        &format!(
            "/api/v1/admin/lessons/{}/publication",
            l["id"].as_str().unwrap()
        ),
        json!({"expected_revision":l["revision"]}),
        &c,
        &t,
    )
    .await;
    assert_eq!(s, 200);
    assert_eq!(newer["data"]["versions"].as_array().unwrap().len(), 2);
    history(&newer["data"]["versions"][0], 2);
    assert_eq!(newer["data"]["versions"][1], l["versions"][0]);
    let (_, page, _) = request(
        &app,
        "GET",
        "/api/v1/admin/lessons?limit=1",
        json!(null),
        &c,
        &t,
    )
    .await;
    keys(&page, &["data", "meta"]);
    assert_eq!(page["meta"], json!({"limit":1,"offset":0,"total":1}));
    assert_eq!(page["data"][0], newer["data"]);
    let (s, archived, _) = request(
        &app,
        "POST",
        &format!(
            "/api/v1/admin/lessons/{}/archive",
            l["id"].as_str().unwrap()
        ),
        json!({"expected_revision":newer["data"]["revision"]}),
        &c,
        &t,
    )
    .await;
    assert_eq!(s, 200);
    assert!(
        archived["data"]
            .as_object()
            .unwrap()
            .contains_key("position")
    );
    assert_eq!(archived["data"]["position"], Value::Null);
}
