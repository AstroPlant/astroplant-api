use chrono::{DateTime, Utc};
use serde::Serialize;

pub struct RawMeasurement {
    pub kit_serial: String,
    pub datetime: DateTime<Utc>,
    pub peripheral: i32,
    pub quantity_type: i32,
    pub value: f64,
}

// Temporary message struct with a timestamp datetime (millis).
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RawMeasurementWithTimestamp {
    pub kit_serial: String,
    pub datetime: i64,
    pub peripheral: i32,
    pub quantity_type: i32,
    pub value: f64,
}

