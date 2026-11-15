//! Explicit synthetic published content. Never called by production bootstrap.
use sqlx::PgPool;
use uuid::Uuid;
/// Two lessons, each with one required section; one single and one multiple question per bank.
pub async fn content(pool: &PgPool) -> (Uuid, Vec<Uuid>) {
    weighted_content(pool, [["1", "1"], ["1", "1"]]).await
}
/// Construct weights before publication seals the immutable question versions.
pub async fn weighted_content(pool: &PgPool, weights: [[&str; 2]; 2]) -> (Uuid, Vec<Uuid>) {
    let mut tx = pool.begin().await.unwrap();
    let school = Uuid::new_v4();
    sqlx::query("INSERT INTO identity_schools(id,name,normalized_name) VALUES($1,'Synthetic school','synthetic school')").bind(school).execute(&mut *tx).await.unwrap();
    let mut banks = vec![];
    for (kind, weights) in ["paired", "quiz"].into_iter().zip(weights) {
        let bank = Uuid::new_v4();
        let version = Uuid::new_v4();
        banks.push(version);
        sqlx::query("INSERT INTO assessment_banks(id,kind,state) VALUES($1,$2,'published')")
            .bind(bank)
            .bind(kind)
            .execute(&mut *tx)
            .await
            .unwrap();
        sqlx::query("INSERT INTO assessment_bank_versions(id,bank_id,version) VALUES($1,$2,1)")
            .bind(version)
            .bind(bank)
            .execute(&mut *tx)
            .await
            .unwrap();
        for (index, typ) in ["single", "multiple"].iter().enumerate() {
            let q = Uuid::new_v4();
            let v = Uuid::new_v4();
            sqlx::query("INSERT INTO assessment_questions(id,bank_id,state,draft_payload,position) VALUES($1,$2,'published','{}',$3)").bind(q).bind(bank).bind(index as i32+1).execute(&mut *tx).await.unwrap();
            sqlx::query("INSERT INTO assessment_question_versions(id,question_id,version,text,type,required,scoring_rule,max_points) VALUES($1,$2,1,'Synthetic frozen question',$3,true,'exact_set_v1',$4::text::numeric)").bind(v).bind(q).bind(typ).bind(weights[index]).execute(&mut *tx).await.unwrap();
            for position in 1..=3 {
                sqlx::query("INSERT INTO assessment_options(id,question_version_id,position,text,is_correct) VALUES($1,$2,$3,$4,$5)").bind(Uuid::new_v4()).bind(v).bind(position).bind(format!("Synthetic option {position}")).bind(position==1 || (*typ=="multiple" && position==2)).execute(&mut *tx).await.unwrap();
            }
            sqlx::query("INSERT INTO assessment_bank_items(bank_version_id,question_version_id,position) VALUES($1,$2,$3)").bind(version).bind(v).bind(index as i32+1).execute(&mut *tx).await.unwrap();
        }
    }
    let path = Uuid::new_v4();
    sqlx::query("INSERT INTO learning_path_versions(id,version,paired_bank_version_id,quiz_bank_version_id) VALUES($1,1,$2,$3)").bind(path).bind(banks[0]).bind(banks[1]).execute(&mut *tx).await.unwrap();
    let mut lessons = vec![];
    for position in 1..=2 {
        let lesson = Uuid::new_v4();
        let version = Uuid::new_v4();
        lessons.push(lesson);
        sqlx::query(
            "INSERT INTO learning_lessons(id,state,draft_payload) VALUES($1,'published','{}')",
        )
        .bind(lesson)
        .execute(&mut *tx)
        .await
        .unwrap();
        sqlx::query("INSERT INTO learning_lesson_versions(id,lesson_id,version,title,description) VALUES($1,$2,1,'Synthetic lesson','Synthetic description')").bind(version).bind(lesson).execute(&mut *tx).await.unwrap();
        sqlx::query("INSERT INTO learning_sections(id,lesson_version_id,section_key,position,title,body,required) VALUES($1,$2,$3,1,'Synthetic section','Protected frozen body',true)").bind(Uuid::new_v4()).bind(version).bind(Uuid::new_v4()).execute(&mut *tx).await.unwrap();
        sqlx::query("INSERT INTO learning_path_items(path_version_id,lesson_version_id,position) VALUES($1,$2,$3)").bind(path).bind(version).bind(position).execute(&mut *tx).await.unwrap();
    }
    tx.commit().await.unwrap();
    (school, lessons)
}

/// A later CMS publication changes future enrollments only, including option IDs and lesson bodies.
pub async fn publish_future(pool: &PgPool) {
    let mut tx = pool.begin().await.unwrap();
    let (old_path,old_paired,quiz):(Uuid,Uuid,Uuid)=sqlx::query_as("SELECT id,paired_bank_version_id,quiz_bank_version_id FROM learning_path_versions ORDER BY version DESC LIMIT 1").fetch_one(&mut *tx).await.unwrap();
    let bank: Uuid = sqlx::query_scalar("SELECT bank_id FROM assessment_bank_versions WHERE id=$1")
        .bind(old_paired)
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    let bank_version = Uuid::new_v4();
    sqlx::query("INSERT INTO assessment_bank_versions(id,bank_id,version) VALUES($1,$2,2)")
        .bind(bank_version)
        .bind(bank)
        .execute(&mut *tx)
        .await
        .unwrap();
    let rows:Vec<(Uuid,Uuid,String,i32)>=sqlx::query_as("SELECT q.id,q.question_id,q.type,i.position FROM assessment_bank_items i JOIN assessment_question_versions q ON q.id=i.question_version_id WHERE i.bank_version_id=$1 ORDER BY i.position").bind(old_paired).fetch_all(&mut *tx).await.unwrap();
    for (old, question, typ, position) in rows {
        let version = Uuid::new_v4();
        sqlx::query("INSERT INTO assessment_question_versions(id,question_id,version,text,type,required,scoring_rule,max_points) VALUES($1,$2,2,'Future synthetic question',$3,true,'exact_set_v1',1)").bind(version).bind(question).bind(typ).execute(&mut *tx).await.unwrap();
        let options:Vec<(i32,bool)>=sqlx::query_as("SELECT position,is_correct FROM assessment_options WHERE question_version_id=$1 ORDER BY position").bind(old).fetch_all(&mut *tx).await.unwrap();
        for (p, correct) in options {
            sqlx::query("INSERT INTO assessment_options(id,question_version_id,position,text,is_correct) VALUES($1,$2,$3,'Future synthetic option',$4)").bind(Uuid::new_v4()).bind(version).bind(p).bind(correct).execute(&mut *tx).await.unwrap();
        }
        sqlx::query("INSERT INTO assessment_bank_items(bank_version_id,question_version_id,position) VALUES($1,$2,$3)").bind(bank_version).bind(version).bind(position).execute(&mut *tx).await.unwrap();
    }
    let path = Uuid::new_v4();
    sqlx::query("INSERT INTO learning_path_versions(id,version,paired_bank_version_id,quiz_bank_version_id) VALUES($1,2,$2,$3)").bind(path).bind(bank_version).bind(quiz).execute(&mut *tx).await.unwrap();
    let rows:Vec<(Uuid,Uuid,i32)>=sqlx::query_as("SELECT i.lesson_version_id,v.lesson_id,i.position FROM learning_path_items i JOIN learning_lesson_versions v ON v.id=i.lesson_version_id WHERE i.path_version_id=$1 ORDER BY i.position").bind(old_path).fetch_all(&mut *tx).await.unwrap();
    for (old, lesson, position) in rows {
        let version = Uuid::new_v4();
        sqlx::query("INSERT INTO learning_lesson_versions(id,lesson_id,version,title,description) VALUES($1,$2,2,'Future synthetic lesson','Future description')").bind(version).bind(lesson).execute(&mut *tx).await.unwrap();
        sqlx::query("INSERT INTO learning_sections(id,lesson_version_id,section_key,position,title,body,required) SELECT $1,$2,section_key,position,'Future section','Future body',required FROM learning_sections WHERE lesson_version_id=$3").bind(Uuid::new_v4()).bind(version).bind(old).execute(&mut *tx).await.unwrap();
        sqlx::query("INSERT INTO learning_path_items(path_version_id,lesson_version_id,position) VALUES($1,$2,$3)").bind(path).bind(version).bind(position).execute(&mut *tx).await.unwrap();
    }
    tx.commit().await.unwrap();
}
