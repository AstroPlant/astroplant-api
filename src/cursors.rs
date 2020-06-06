use crate::models;
use crate::problem::{Problem, BAD_REQUEST};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use uuid::Uuid;

#[derive(Deserialize, Serialize)]
pub struct AggregateMeasurements(pub DateTime<Utc>, pub Uuid);

impl FromStr for AggregateMeasurements {
    type Err = Problem;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        serde_json::from_str(s).map_err(|_| BAD_REQUEST)
    }
}

impl From<AggregateMeasurements> for String {
    fn from(cursor: AggregateMeasurements) -> Self {
        serde_json::to_string(&cursor).unwrap()
    }
}

impl AggregateMeasurements {
    pub const PER_PAGE: usize = 50;

    pub fn next_from_page(page: &[models::AggregateMeasurement]) -> Option<Self> {
        if page.len() >= Self::PER_PAGE {
            let measurement = page.last().unwrap();
            Some(Self(measurement.datetime_start, measurement.id))
        } else {
            None
        }
    }
}
