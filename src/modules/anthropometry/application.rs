//! Measurement insert/history coordinated with quiz and paired post-test prerequisites.
use crate::{
    modules::{
        anthropometry::domain::*,
        learning::{
            application::{
                MeasurementCreation, MeasurementHistory, Operation, OperationResult,
                StudentService, fingerprint_fields,
            },
            domain::StudentScope,
        },
    },
    shared::error::{AppError, AppResult},
};
impl StudentService {
    /// Validate exact explicit-unit input and record a server-computed result before unlocking post-test.
    pub async fn measure(
        &self,
        scope: StudentScope,
        weight: PositiveInput,
        height: PositiveInput,
        op: Operation,
    ) -> AppResult<MeasurementCreation> {
        let bmi = Bmi::calculate(weight, height)?;
        // Exact equivalent decimal spellings are the same validated operation.
        let op = Operation::new(
            op.scope,
            op.key,
            &fingerprint_fields(&[
                weight.decimal_text().as_bytes(),
                height.decimal_text().as_bytes(),
            ]),
            op.request_id,
        )?;
        let mut tx = self.uow.begin().await?;
        if let Some(out) = op.replay(&mut *tx, "measurement.create").await? {
            tx.commit().await?;
            let OperationResult::Measurement(out) = out else {
                return Err(AppError::Internal);
            };
            return Ok(out);
        }
        tx.journey(scope).await?.require_measurement()?;
        let m = tx
            .insert_measurement(
                scope,
                &weight.decimal_text(),
                &height.decimal_text(),
                &bmi.decimal_text(),
                &bmi.display(),
            )
            .await?;
        let j = tx.journey(scope).await?;
        let measurement_id = m.id;
        let out = MeasurementCreation {
            measurement: m,
            progress: j.progress(),
        };
        op.remember(
            &mut *tx,
            "measurement.create",
            OperationResult::Measurement(out.clone()),
        )
        .await?;
        tx.audit("measurement.create", measurement_id, &op.request_id)
            .await?;
        tx.commit().await?;
        Ok(out)
    }
    /// Return bounded own history in descending creation/UUID order.
    pub async fn measurements(
        &self,
        scope: StudentScope,
        limit: i64,
        offset: i64,
    ) -> AppResult<MeasurementHistory> {
        let mut tx = self.uow.begin().await?;
        tx.journey(scope).await?;
        let (measurements, total) = tx.measurement_history(scope, limit, offset).await?;
        let out = MeasurementHistory {
            measurements,
            total,
            limit,
            offset,
        };
        tx.commit().await?;
        Ok(out)
    }
}
