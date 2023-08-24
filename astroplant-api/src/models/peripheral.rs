use crate::schema::{peripheral_definitions, peripherals};

use diesel::dsl::sql;
use diesel::pg::PgConnection;
use diesel::prelude::*;
use diesel::sql_types::Integer;
use diesel::{Identifiable, QueryResult, Queryable};
use validator::Validate;

#[rustfmt::skip]
use super::{
    Kit, KitId,
    KitConfiguration, KitConfigurationId,
    PeripheralDefinition, PeripheralDefinitionId,
};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Identifiable)]
#[table_name = "peripherals"]
pub struct PeripheralId(#[column_name = "id"] pub i32);

#[derive(Clone, Debug, PartialEq, Queryable, Identifiable, Associations, AsChangeset, Validate)]
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
    #[validate(length(min = 1, max = 40))]
    pub name: String,
    pub configuration: serde_json::Value,
}

#[derive(Clone, Debug, PartialEq, Queryable, Identifiable, AsChangeset, Validate)]
#[table_name = "peripherals"]
pub struct UpdatePeripheral {
    pub id: i32,
    // None means don't update.
    #[validate(length(min = 1, max = 40))]
    pub name: Option<String>,
    pub configuration: Option<serde_json::Value>,
}

impl Peripheral {
    pub fn by_id(conn: &mut PgConnection, peripheral_id: PeripheralId) -> QueryResult<Option<Self>> {
        peripherals::table
            .find(&peripheral_id.0)
            .first(conn)
            .optional()
    }

    pub fn delete(&self, conn: &mut PgConnection) -> QueryResult<bool> {
        diesel::delete(self).execute(conn).map(|r| r > 0)
    }

    pub fn peripherals_of_kit(conn: &mut PgConnection, kit: &Kit) -> QueryResult<Vec<Self>> {
        Peripheral::belonging_to(kit).load(conn)
    }

    pub fn peripherals_of_kit_id(conn: &mut PgConnection, kit_id: KitId) -> QueryResult<Vec<Self>> {
        Peripheral::belonging_to(&kit_id).load(conn)
    }

    pub fn peripherals_of_kit_configuration(
        conn: &mut PgConnection,
        kit_configuration: &KitConfiguration,
    ) -> QueryResult<Vec<Self>> {
        Peripheral::belonging_to(kit_configuration).load(conn)
    }

    pub fn peripherals_of_kit_configuration_id(
        conn: &mut PgConnection,
        kit_configuration_id: KitConfigurationId,
    ) -> QueryResult<Vec<Self>> {
        Peripheral::belonging_to(&kit_configuration_id).load(conn)
    }

    pub fn peripherals_with_definitions_of_kit_configuration(
        conn: &mut PgConnection,
        kit_configuration: &KitConfiguration,
    ) -> QueryResult<Vec<(Self, PeripheralDefinition)>> {
        Peripheral::peripherals_with_definitions_of_kit_configuration_id(
            conn,
            kit_configuration.get_id(),
        )
    }

    pub fn peripherals_with_definitions_of_kit_configuration_id(
        conn: &mut PgConnection,
        kit_configuration_id: KitConfigurationId,
    ) -> QueryResult<Vec<(Self, PeripheralDefinition)>> {
        Peripheral::belonging_to(&kit_configuration_id)
            .inner_join(peripheral_definitions::table)
            .load(conn)
    }

    pub fn clone_all_to_new_configuration(
        conn: &mut PgConnection,
        from_kit_configuration: KitConfigurationId,
        to_kit: KitId,
        to_kit_configuration: KitConfigurationId,
    ) -> QueryResult<()> {
        let peripherals = peripherals::table
            .filter(peripherals::kit_configuration_id.eq_all(from_kit_configuration.0))
            .select((
                sql::<Integer>(&to_kit.0.to_string()),
                sql::<Integer>(&to_kit_configuration.0.to_string()),
                peripherals::peripheral_definition_id,
                peripherals::name,
                peripherals::configuration,
            ));
        diesel::insert_into(peripherals::table)
            .values(peripherals)
            .into_columns((
                peripherals::kit_id,
                peripherals::kit_configuration_id,
                peripherals::peripheral_definition_id,
                peripherals::name,
                peripherals::configuration,
            ))
            .execute(conn)?;
        Ok(())
    }

    pub fn get_id(&self) -> PeripheralId {
        PeripheralId(self.id)
    }

    pub fn get_kit_id(&self) -> KitId {
        KitId(self.kit_id)
    }

    pub fn get_kit_configuration_id(&self) -> KitConfigurationId {
        KitConfigurationId(self.kit_configuration_id)
    }

    pub fn get_peripheral_definition_id(&self) -> PeripheralDefinitionId {
        PeripheralDefinitionId(self.peripheral_definition_id)
    }
}

impl UpdatePeripheral {
    pub fn update(&self, conn: &mut PgConnection) -> QueryResult<Peripheral> {
        self.save_changes(conn)
    }
}

#[derive(Clone, Debug, PartialEq, Insertable, Validate)]
#[table_name = "peripherals"]
pub struct NewPeripheral {
    pub kit_id: i32,
    pub kit_configuration_id: i32,
    pub peripheral_definition_id: i32,
    #[validate(length(min = 1, max = 40))]
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

    pub fn create(&self, conn: &mut PgConnection) -> QueryResult<Peripheral> {
        use crate::schema::peripherals::dsl::*;

        diesel::insert_into(peripherals)
            .values(self)
            .on_conflict_do_nothing()
            .get_result::<Peripheral>(conn)
    }
}
