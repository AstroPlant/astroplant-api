use crate::schema::peripherals;

use diesel::pg::PgConnection;
use diesel::prelude::*;
use diesel::{Identifiable, QueryResult, Queryable};

#[rustfmt::skip]
use super::{
    Kit, KitId,
    KitConfiguration, KitConfigurationId,
    PeripheralDefinition, PeripheralDefinitionId,
};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Identifiable)]
#[table_name = "peripherals"]
pub struct PeripheralId(#[column_name = "id"] pub i32);

#[derive(Clone, Debug, PartialEq, Queryable, Identifiable, Associations)]
#[belongs_to(parent = "Kit", foreign_key = "kit_id")]
#[belongs_to(parent = "KitId", foreign_key = "kit_id")]
#[belongs_to(parent = "KitConfiguration", foreign_key = "kit_configuration_id")]
#[belongs_to(parent = "KitConfigurationId", foreign_key = "kit_configuration_id")]
#[belongs_to(
    parent = "PeripheralDefinition",
    foreign_key = "peripheral_definition_id"
)]
#[belongs_to(
    parent = "PeripheralDefinitionId",
    foreign_key = "peripheral_definition_id"
)]
#[table_name = "peripherals"]
pub struct Peripheral {
    pub id: i32,
    pub kit_id: i32,
    pub kit_configuration_id: i32,
    pub peripheral_definition_id: i32,
    pub name: String,
    pub configuration: serde_json::Value,
}

impl Peripheral {
    pub fn peripherals_of_kit(conn: &PgConnection, kit: &Kit) -> QueryResult<Vec<Self>> {
        Peripheral::belonging_to(kit).load(conn)
    }

    pub fn peripherals_of_kit_id(conn: &PgConnection, kit_id: KitId) -> QueryResult<Vec<Self>> {
        Peripheral::belonging_to(&kit_id).load(conn)
    }

    pub fn peripherals_of_kit_configuration(
        conn: &PgConnection,
        kit_configuration: &KitConfiguration,
    ) -> QueryResult<Vec<Self>> {
        Peripheral::belonging_to(kit_configuration).load(conn)
    }

    pub fn peripherals_of_kit_configuration_id(
        conn: &PgConnection,
        kit_configuration_id: KitConfigurationId,
    ) -> QueryResult<Vec<Self>> {
        Peripheral::belonging_to(&kit_configuration_id).load(conn)
    }

    pub fn get_id(&self) -> PeripheralId {
        PeripheralId(self.id)
    }
}

#[derive(Clone, Debug, PartialEq, Insertable)]
#[table_name = "peripherals"]
pub struct NewPeripheral {
    pub kit_id: i32,
    pub kit_configuration_id: i32,
    pub peripheral_definition_id: i32,
    pub name: String,
    pub configuration: serde_json::Value,
}

impl NewPeripheral {
    pub fn new(
        kit_id: KitId,
        kit_configuration_id: KitConfigurationId,
        peripheral_definition_id: PeripheralDefinitionId,
        name: String,
        configuration: serde_json::Value,
    ) -> Self {
        Self {
            kit_id: kit_id.0,
            kit_configuration_id: kit_configuration_id.0,
            peripheral_definition_id: peripheral_definition_id.0,
            name,
            configuration,
        }
    }

    pub fn create(&self, conn: &PgConnection) -> QueryResult<Peripheral> {
        use crate::schema::peripherals::dsl::*;

        diesel::insert_into(peripherals)
            .values(self)
            .on_conflict_do_nothing()
            .get_result::<Peripheral>(conn)
    }
}
