//! Exact fixed-point BMI policy. Input limits are representation limits, not health ranges.
use crate::shared::error::{AppError, AppResult};
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Number;
use std::str::FromStr;
use uuid::Uuid;

/// Positive NUMERIC(12,4) value, represented in ten-thousandths without rounding.
#[derive(Clone, Copy, Debug)]
pub struct PositiveInput(i128);
impl PositiveInput {
    pub fn parse(value: &Number) -> AppResult<Self> {
        Self::parse_text(&value.to_string())
    }
    pub fn parse_text(value: &str) -> AppResult<Self> {
        let invalid = || AppError::Invalid("INVALID_MEASUREMENT");
        let (mantissa, exponent) = match value.split_once(['e', 'E']) {
            Some((m, e)) => (m, e.parse::<i32>().map_err(|_| invalid())?),
            None => (value, 0),
        };
        if mantissa.starts_with('-') || !(-100..=100).contains(&exponent) {
            return Err(invalid());
        }
        let (whole, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
        let digits = format!("{whole}{fraction}");
        if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return Err(invalid());
        }
        let trimmed = digits.trim_start_matches('0').trim_end_matches('0');
        if trimmed.is_empty() {
            return Err(invalid());
        }
        let trailing = digits.len() - digits.trim_end_matches('0').len();
        let shift = 4i64 + i64::from(exponent) - fraction.len() as i64 + trailing as i64;
        if !(0..=12).contains(&shift) || trimmed.len() > 12 {
            return Err(invalid());
        }
        let scaled = trimmed
            .parse::<i128>()
            .map_err(|_| invalid())?
            .checked_mul(10i128.pow(shift as u32))
            .ok_or_else(invalid)?;
        if !(1..=999_999_999_999).contains(&scaled) {
            return Err(invalid());
        }
        Ok(Self(scaled))
    }
    pub fn sql(self) -> String {
        fixed(self.0, 4)
    }
    pub fn number(self) -> AppResult<Number> {
        Number::from_str(&self.sql()).map_err(|_| AppError::Internal)
    }
}
/// Stored scale12 result; display rounding never feeds back into calculations.
#[derive(Clone, Debug)]
pub struct Bmi {
    scaled: i128,
}
impl Bmi {
    pub fn calculate(weight: PositiveInput, height: PositiveInput) -> AppResult<Self> {
        let denominator = height
            .0
            .checked_mul(height.0)
            .ok_or(AppError::Invalid("INVALID_MEASUREMENT"))?;
        let numerator = weight
            .0
            .checked_mul(100_000_000_000_000_000_000)
            .ok_or(AppError::Invalid("INVALID_MEASUREMENT"))?;
        let scaled = (numerator + denominator / 2) / denominator;
        if scaled <= 0 {
            return Err(AppError::Invalid("INVALID_MEASUREMENT"));
        }
        Ok(Self { scaled })
    }
    pub fn sql(&self) -> String {
        fixed(self.scaled, 12)
    }
    pub fn number(&self) -> AppResult<Number> {
        Number::from_str(&self.sql()).map_err(|_| AppError::Internal)
    }
    pub fn display(&self) -> String {
        fixed((self.scaled + 50_000_000_000) / 100_000_000_000, 1)
    }
}
fn fixed(value: i128, scale: u32) -> String {
    let base = 10i128.pow(scale);
    format!(
        "{}.{:0width$}",
        value / base,
        value % base,
        width = scale as usize
    )
}
#[derive(Clone, Serialize)]
pub struct Measurement {
    pub id: Uuid,
    pub weight_kg: Number,
    pub height_cm: Number,
    pub bmi: Number,
    pub bmi_display: String,
    pub calculation_rule_version: String,
    pub created_at: DateTime<Utc>,
}
