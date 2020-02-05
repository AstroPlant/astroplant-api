use serde::Serialize;

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RawMeasurement {
    pub kit_serial: String,
    pub datetime: u64,
    pub peripheral: i32,
    pub quantity_type: i32,
    pub value: f64,
}

