//! Numeric strings avoid lossy binary floating-point driver conversion.
use super::PgStudentTransaction;
use crate::{
    modules::{
        anthropometry::domain::{Bmi, Measurement, PositiveInput},
        learning::{application::ports::MeasurementRepository, domain::StudentScope},
    },
    platform::database::failure,
    shared::error::{AppError, AppResult},
};
use async_trait::async_trait;
use sqlx::{Row, postgres::PgRow};
use std::str::FromStr;
use uuid::Uuid;
fn measurement(r: PgRow) -> AppResult<Measurement> {
    let weight = PositiveInput::parse_text(&r.try_get::<String, _>("weight").map_err(failure)?)?;
    let height = PositiveInput::parse_text(&r.try_get::<String, _>("height").map_err(failure)?)?;
    // Display is deterministic from the stored policy and inputs; stored BMI remains authoritative.
    let display = Bmi::calculate(weight, height)?.display();
    Ok(Measurement {
        id: r.try_get("id").map_err(failure)?,
        weight_kg: weight.number()?,
        height_cm: height.number()?,
        bmi: serde_json::Number::from_str(&r.try_get::<String, _>("result").map_err(failure)?)
            .map_err(|_| AppError::Internal)?,
        bmi_display: display,
        calculation_rule_version: r.try_get("calculation_rule_version").map_err(failure)?,
        created_at: r.try_get("created_at").map_err(failure)?,
    })
}
#[async_trait]
impl MeasurementRepository for PgStudentTransaction {
    async fn insert_measurement(
        &mut self,
        s: StudentScope,
        weight: &str,
        height: &str,
        bmi: &str,
        _display: &str,
    ) -> AppResult<Measurement> {
        let r=sqlx::query("INSERT INTO anthropometry_measurements(id,enrollment_id,weight_kg,height_cm,bmi,calculation_rule_version) VALUES($1,$2,$3::text::numeric,$4::text::numeric,$5::text::numeric,'bmi_v1') RETURNING *,weight_kg::text AS weight,height_cm::text AS height,bmi::text AS result").bind(Uuid::new_v4()).bind(s.enrollment_id).bind(weight).bind(height).bind(bmi).fetch_one(&mut *self.tx).await.map_err(failure)?;
        measurement(r)
    }
    async fn measurement_history(
        &mut self,
        s: StudentScope,
        limit: i64,
        offset: i64,
    ) -> AppResult<(Vec<Measurement>, i64)> {
        let total = sqlx::query_scalar(
            "SELECT count(*) FROM anthropometry_measurements WHERE enrollment_id=$1",
        )
        .bind(s.enrollment_id)
        .fetch_one(&mut *self.tx)
        .await
        .map_err(failure)?;
        let rows=sqlx::query("SELECT *,weight_kg::text AS weight,height_cm::text AS height,bmi::text AS result FROM anthropometry_measurements WHERE enrollment_id=$1 ORDER BY created_at DESC,id DESC LIMIT $2 OFFSET $3").bind(s.enrollment_id).bind(limit).bind(offset).fetch_all(&mut *self.tx).await.map_err(failure)?;
        Ok((
            rows.into_iter()
                .map(measurement)
                .collect::<AppResult<Vec<_>>>()?,
            total,
        ))
    }
}
