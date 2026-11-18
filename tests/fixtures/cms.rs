//! Synthetic content is authored through real admin HTTP endpoints, never imported as publications.
#![allow(dead_code)]
use crate::common::*;
use axum::Router;
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;
pub async fn editor(pool: &PgPool) -> (Router, String, String, Uuid) {
    let id = admin(pool, false).await;
    sqlx::query("UPDATE identity_admins SET can_manage_content=true WHERE id=$1")
        .bind(id)
        .execute(pool)
        .await
        .unwrap();
    let app = app(pool.clone()).await;
    let (c, t) = login(&app, id).await;
    (app, c, t, id)
}
pub async fn initialize(pool: &PgPool) {
    use imt_smart_teens_backend::modules::learning::admin::{
        application::LessonCms, infrastructure::PgLessonCms,
    };
    LessonCms::new(std::sync::Arc::new(PgLessonCms::new(pool.clone())))
        .initialize_development("development")
        .await
        .unwrap();
}
pub struct Content {
    pub lesson: Value,
    pub banks: Vec<Value>,
    pub questions: Vec<Value>,
    pub path: Value,
}
pub async fn publish(app: &Router, c: &str, t: &str) -> Content {
    let (s,l,_)=request(app,"POST","/api/v1/admin/lessons",json!({"title":"Historical lesson","description":"Description","sections":[{"title":"Section","body":"Historical section","required":true}],"quiz":{"text":"Historical quiz","options":[{"id":"00000000-0000-0000-0000-000000000001","text":"Historical A"},{"id":"00000000-0000-0000-0000-000000000002","text":"Historical B"}],"correct_option_id":"00000000-0000-0000-0000-000000000001"},"position":1}),c,t).await;
    assert_eq!(s, 201, "{l}");
    let (s, l, _) = request(
        app,
        "POST",
        &format!(
            "/api/v1/admin/lessons/{}/publication",
            l["data"]["id"].as_str().unwrap()
        ),
        json!({"expected_revision":1}),
        c,
        t,
    )
    .await;
    assert_eq!(s, 200, "{l}");
    let (_, bs, _) = request(
        app,
        "GET",
        "/api/v1/admin/question-banks",
        json!(null),
        c,
        t,
    )
    .await;
    let mut banks = vec![];
    let mut questions = vec![];
    for b in bs["data"].as_array().unwrap() {
        let id = b["id"].as_str().unwrap();
        for (i, typ) in ["single", "multiple"].iter().enumerate() {
            let a = Uuid::new_v4();
            let z = Uuid::new_v4();
            let(s,q,_)=request(app,"POST",&format!("/api/v1/admin/question-banks/{id}/questions"),json!({"text":"Historical question","type":typ,"options":[{"id":a,"text":"Historical one"},{"id":z,"text":"Historical two"}],"correct_option_ids":if *typ=="single"{vec![a]}else{vec![a,z]},"position":i+1,"required":true}),c,t).await;
            assert_eq!(s, 201, "{q}");
            questions.push(q["data"].clone());
        }
        let (s, b, _) = request(
            app,
            "POST",
            &format!("/api/v1/admin/question-banks/{id}/publication"),
            json!({"expected_revision":3}),
            c,
            t,
        )
        .await;
        assert_eq!(s, 200, "{b}");
        banks.push(b["data"].clone());
    }
    let (_, p, _) = request(app, "GET", "/api/v1/admin/learning-path", json!(null), c, t).await;
    let (s, p, _) = request(
        app,
        "POST",
        "/api/v1/admin/learning-path/publication",
        json!({"expected_revision":p["data"]["revision"]}),
        c,
        t,
    )
    .await;
    assert_eq!(s, 200, "path publication: {p}");
    Content {
        lesson: l["data"].clone(),
        banks,
        questions,
        path: p["data"].clone(),
    }
}
pub async fn school(pool: &PgPool, label: &str) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO identity_schools(id,name,normalized_name) VALUES($1,$2,$2)")
        .bind(id)
        .bind(label)
        .execute(pool)
        .await
        .unwrap();
    id
}
pub async fn answer(app: &Router, c: &str, t: &str, a: &Value, correct: bool) {
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
            vec![q["options"][1]["id"].clone()]
        };
        let (s, v, _) = request(
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
        )
        .await;
        assert_eq!(s, 200, "{v}");
    }
}
