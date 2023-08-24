use crate::schema::peripheral_definition_expected_quantity_types;

use diesel::pg::PgConnection;
use diesel::prelude::*;
use diesel::{Identifiable, QueryResult, Queryable};

use super::{PeripheralDefinition, PeripheralDefinitionId};
use super::{QuantityType, QuantityTypeId};

#[derive(Clone, Debug, PartialEq, Eq, Queryable, Identifiable, Associations)]
#[diesel(
    table_name = peripheral_definition_expected_quantity_types,
    belongs_to(QuantityType),
    belongs_to(PeripheralDefinition),
    belongs_to(QuantityTypeId, foreign_key = quantity_type_id),
    belongs_to(PeripheralDefinitionId, foreign_key = peripheral_definition_id),
)]
pub struct PeripheralDefinitionExpectedQuantityType {
    pub id: i32,
    pub quantity_type_id: i32,
    pub peripheral_definition_id: i32,
}

impl PeripheralDefinitionExpectedQuantityType {
    pub fn of_peripheral_definitions(
        conn: &mut PgConnection,
        peripheral_definitions: &[PeripheralDefinition],
    ) -> QueryResult<Vec<Vec<Self>>> {
        PeripheralDefinitionExpectedQuantityType::belonging_to(peripheral_definitions)
            .load(conn)
            .map(|res| res.grouped_by(peripheral_definitions))
    }
}
