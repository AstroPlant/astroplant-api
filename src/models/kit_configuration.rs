use crate::schema::kit_configurations;

use diesel::pg::PgConnection;
use diesel::prelude::*;
use diesel::{Identifiable, QueryResult, Queryable};

use super::{Kit, KitId};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Identifiable)]
#[table_name = "kit_configurations"]
pub struct KitConfigurationId(#[column_name = "id"] pub i32);

#[derive(Clone, Debug, PartialEq, Eq, Queryable, Identifiable, Associations)]
#[belongs_to(parent = "Kit", foreign_key = "kit_id")]
#[belongs_to(parent = "KitId", foreign_key = "kit_id")]
#[table_name = "kit_configurations"]
pub struct KitConfiguration {
    pub id: i32,
    pub kit_id: i32,
    pub description: Option<String>,
    pub active: bool,
    pub never_used: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Queryable, Identifiable, AsChangeset)]
#[table_name = "kit_configurations"]
pub struct UpdateKitConfiguration {
    pub id: i32,
    // None means don't update, Some(None) means set to null.
    pub description: Option<Option<String>>,
    pub active: Option<bool>,
    pub never_used: Option<bool>,
}

impl KitConfiguration {
    pub fn by_id(
        conn: &PgConnection,
        configuration_id: KitConfigurationId,
    ) -> QueryResult<Option<Self>> {
        kit_configurations::table
            .find(&configuration_id.0)
            .first(conn)
            .optional()
    }

    pub fn configurations_of_kit(conn: &PgConnection, kit: &Kit) -> QueryResult<Vec<Self>> {
        KitConfiguration::belonging_to(kit).load(conn)
    }

    pub fn configurations_of_kit_id(conn: &PgConnection, kit_id: KitId) -> QueryResult<Vec<Self>> {
        KitConfiguration::belonging_to(&kit_id).load(conn)
    }

    pub fn active_configuration_of_kit(
        conn: &PgConnection,
        kit: &Kit,
    ) -> QueryResult<Option<Self>> {
        Self::active_configuration_of_kit_id(conn, KitId(kit.id))
    }

    pub fn active_configuration_of_kit_id(
        conn: &PgConnection,
        kit_id: KitId,
    ) -> QueryResult<Option<Self>> {
        use kit_configurations::dsl;
        kit_configurations::table
            .filter(dsl::kit_id.eq(&kit_id.0))
            .first(conn)
            .optional()
    }

    /**
     * Deactive all of the kit's configurations.
     * Returns the amount of deactivated configurations.
     */
    pub fn deactivate_all_of_kit(conn: &PgConnection, kit: &Kit) -> QueryResult<usize> {
        Self::deactivate_all_of_kit_id(conn, kit.get_id())
    }

    /**
     * Deactive all of the kit's configurations.
     * Returns the amount of deactivated configurations.
     */
    pub fn deactivate_all_of_kit_id(conn: &PgConnection, kit_id: KitId) -> QueryResult<usize> {
        use crate::schema::kit_configurations::dsl;

        diesel::update(dsl::kit_configurations.filter(dsl::kit_id.eq(&kit_id.0)))
            .set(dsl::active.eq(false))
            .execute(conn)
    }

    pub fn get_id(&self) -> KitConfigurationId {
        KitConfigurationId(self.id)
    }
}

impl UpdateKitConfiguration {
    pub fn update(&self, conn: &PgConnection) -> QueryResult<KitConfiguration> {
        use crate::schema::kit_configurations::dsl;
        diesel::update(dsl::kit_configurations)
            .set(self)
            .get_result(conn)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Insertable)]
#[table_name = "kit_configurations"]
pub struct NewKitConfiguration {
    pub kit_id: i32,
    pub description: Option<String>,
}

impl NewKitConfiguration {
    pub fn new(kit_id: KitId, description: Option<String>) -> Self {
        Self {
            kit_id: kit_id.0,
            description: description,
        }
    }

    pub fn create(&self, conn: &PgConnection) -> QueryResult<KitConfiguration> {
        use crate::schema::kit_configurations::dsl::*;

        diesel::insert_into(kit_configurations)
            .values(self)
            .on_conflict_do_nothing()
            .get_result::<KitConfiguration>(conn)
    }
}
