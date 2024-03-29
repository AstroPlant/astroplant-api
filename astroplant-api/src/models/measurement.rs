use crate::cursors;
use crate::schema::aggregate_measurements;

use chrono::{DateTime, Utc};
use diesel::pg::PgConnection;
use diesel::prelude::*;
use diesel::{Identifiable, QueryResult, Queryable};
use uuid::Uuid;
use validator::Validate;

#[rustfmt::skip]
use super::{
    Kit, KitId,
    KitConfiguration, KitConfigurationId,
    Peripheral, PeripheralId,
    QuantityType, QuantityTypeId,
};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Identifiable)]
#[diesel(table_name = aggregate_measurements)]
pub struct AggregateMeasurementId(#[diesel(column_name = id)] pub Uuid);

#[derive(Clone, Debug, PartialEq, Queryable, Identifiable, Associations, Validate)]
#[diesel(
    belongs_to(Kit, foreign_key = kit_id),
    belongs_to(KitId, foreign_key = kit_id),
    belongs_to(KitConfiguration, foreign_key = kit_configuration_id),
    belongs_to(KitConfigurationId, foreign_key = kit_configuration_id),
    belongs_to(Peripheral, foreign_key = peripheral_id),
    belongs_to(PeripheralId, foreign_key = peripheral_id),
    belongs_to(QuantityType, foreign_key = quantity_type_id),
    belongs_to(QuantityTypeId, foreign_key = quantity_type_id),
)]
pub struct AggregateMeasurement {
    pub id: Uuid,
    pub peripheral_id: i32,
    pub kit_id: i32,
    pub kit_configuration_id: i32,
    pub quantity_type_id: i32,
    pub datetime_start: DateTime<Utc>,
    pub datetime_end: DateTime<Utc>,
    pub values: serde_json::Value,
}

impl AggregateMeasurement {
    pub fn by_id(
        conn: &mut PgConnection,
        aggregate_measurement_id: AggregateMeasurementId,
    ) -> QueryResult<Option<Self>> {
        aggregate_measurements::table
            .find(&aggregate_measurement_id.0)
            .first(conn)
            .optional()
    }

    pub fn page(
        conn: &mut PgConnection,
        kit_id: KitId,
        configuration_id: Option<i32>,
        peripheral_id: Option<i32>,
        quantity_type_id: Option<i32>,
        cursor: Option<cursors::AggregateMeasurements>,
    ) -> QueryResult<Vec<Self>> {
        let mut query = aggregate_measurements::table
            .filter(aggregate_measurements::columns::kit_id.eq(kit_id.0))
            .into_boxed();

        if let Some(configuration_id) = configuration_id {
            query = query
                .filter(aggregate_measurements::columns::kit_configuration_id.eq(configuration_id));
        }
        if let Some(peripheral_id) = peripheral_id {
            query = query.filter(aggregate_measurements::columns::peripheral_id.eq(peripheral_id));
        }
        if let Some(quantity_type_id) = quantity_type_id {
            query = query
                .filter(aggregate_measurements::columns::quantity_type_id.eq(quantity_type_id));
        }

        if let Some(cursors::AggregateMeasurements(datetime, id)) = cursor {
            query = query.filter(
                aggregate_measurements::columns::datetime_start
                    .lt(datetime)
                    .or(aggregate_measurements::columns::datetime_start
                        .eq(datetime)
                        .and(aggregate_measurements::columns::id.lt(id))),
            )
        }
        query
            .order((
                aggregate_measurements::dsl::datetime_start.desc(),
                aggregate_measurements::dsl::id.desc(),
            ))
            .limit(cursors::AggregateMeasurements::PER_PAGE as i64)
            .load(conn)
    }

    pub fn get_id(&self) -> AggregateMeasurementId {
        AggregateMeasurementId(self.id)
    }
}
