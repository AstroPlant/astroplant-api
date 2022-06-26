use crate::schema::peripheral_definition_expected_quantity_types;

use diesel::pg::PgConnection;
use diesel::prelude::*;
use diesel::{Identifiable, QueryResult, Queryable};

use super::{PeripheralDefinition, PeripheralDefinitionId};
use super::{QuantityType, QuantityTypeId};

#[derive(Clone, Debug, PartialEq, Eq, Queryable, Identifiable, Associations)]
#[belongs_to(parent = "QuantityType")]
#[belongs_to(parent = "PeripheralDefinition")]
#[belongs_to(parent = "QuantityTypeId", foreign_key = "quantity_type_id")]
#[belongs_to(
    parent = "PeripheralDefinitionId",
    foreign_key = "peripheral_definition_id"
)]
#[table_name = "peripheral_definition_expected_quantity_types"]
pub struct PeripheralDefinitionExpectedQuantityType {
    pub id: i32,
    pub quantity_type_id: i32,
    pub peripheral_definition_id: i32,
}

impl PeripheralDefinitionExpectedQuantityType {
    pub fn of_peripheral_definitions(
        conn: &PgConnection,
        peripheral_definitions: &[PeripheralDefinition],
    ) -> QueryResult<Vec<Vec<Self>>> {
        PeripheralDefinitionExpectedQuantityType::belonging_to(peripheral_definitions)
            .load(conn)
            .map(|res| res.grouped_by(peripheral_definitions))
    }
}
