//! Measurement insert/history coordinated with quiz and paired post-test prerequisites.
use crate::{
    modules::{
        anthropometry::domain::*,
        learning::{
            application::{Operation, StudentService},
            domain::StudentScope,
        },
    },
    shared::error::AppResult,
};
use serde_json::{Number, Value, json};
impl StudentService {
    /// Validate exact explicit-unit input and record a server-computed result before unlocking post-test.
    pub async fn measure(
        &self,
        scope: StudentScope,
        weight: Number,
        height: Number,
        op: Operation,
    ) -> AppResult<Value> {
        let weight = PositiveInput::parse(&weight)?;
        let height = PositiveInput::parse(&height)?;
        let bmi = Bmi::calculate(weight, height)?;
        // Exact equivalent decimal spellings are the same validated operation.
        let op = Operation::new(
            op.scope,
            op.key,
            &json!({"weight_kg":weight.sql(),"height_cm":height.sql()}),
            op.request_id,
        )?;
        let mut tx = self.uow.begin().await?;
        if let Some((_, out)) = op.replay(&mut *tx, "measurement.create").await? {
            tx.commit().await?;
            return Ok(out);
        }
        tx.journey(scope).await?.require_measurement()?;
        let m = tx
            .insert_measurement(
                scope,
                &weight.sql(),
                &height.sql(),
                &bmi.sql(),
                &bmi.display(),
            )
            .await?;
        let j = tx.journey(scope).await?;
        let mut out = json!(m);
        out["progress"] = j.progress();
        op.remember(&mut *tx, "measurement.create", 201, out.clone())
            .await?;
        tx.audit("measurement.create", m.id, &op.request_id).await?;
        tx.commit().await?;
        Ok(out)
    }
    /// Return bounded own history in descending creation/UUID order.
    pub async fn measurements(
        &self,
        scope: StudentScope,
        limit: i64,
        offset: i64,
    ) -> AppResult<(Vec<Measurement>, i64)> {
        let mut tx = self.uow.begin().await?;
        tx.journey(scope).await?;
        let out = tx.measurement_history(scope, limit, offset).await?;
        tx.commit().await?;
        Ok(out)
    }
}
