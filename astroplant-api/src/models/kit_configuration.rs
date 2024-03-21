use chrono::{DateTime, Utc};
use diesel::pg::PgConnection;
use diesel::prelude::*;
use diesel::{Identifiable, QueryResult, Queryable};
use serde_json::json;

use crate::schema::kit_configurations;

use super::{Kit, KitId};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Identifiable)]
#[diesel(table_name = kit_configurations)]
pub struct KitConfigurationId(#[diesel(column_name = id)] pub i32);

impl KitConfigurationId {
    pub fn delete(&self, conn: &mut PgConnection) -> QueryResult<bool> {
        diesel::delete(self).execute(conn).map(|r| r > 0)
    }
}

#[derive(Clone, Debug, PartialEq, Queryable, Identifiable, Associations)]
#[diesel(
    table_name = kit_configurations,
    belongs_to(Kit, foreign_key = kit_id),
    belongs_to(KitId, foreign_key = kit_id)
)]
pub struct KitConfiguration {
    pub id: i32,
    pub kit_id: i32,
    pub description: Option<String>,
    pub controller_symbol_location: String,
    pub controller_symbol: String,
    pub control_rules: serde_json::Value,
    pub active: bool,
    pub first_activated_at: Option<DateTime<Utc>>,
    pub never_used: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Queryable, Identifiable, AsChangeset)]
#[diesel(table_name = kit_configurations)]
pub struct UpdateKitConfiguration {
    pub id: i32,
    // None means don't update, Some(None) means set to null.
    pub description: Option<Option<String>>,
    pub controller_symbol_location: Option<String>,
    pub controller_symbol: Option<String>,
    pub control_rules: Option<serde_json::Value>,
    pub active: Option<bool>,
}

impl KitConfiguration {
    pub fn by_id(
        conn: &mut PgConnection,
        configuration_id: KitConfigurationId,
    ) -> QueryResult<Option<Self>> {
        kit_configurations::table
            .find(&configuration_id.0)
            .first(conn)
            .optional()
    }

    pub fn configurations_of_kit(conn: &mut PgConnection, kit: &Kit) -> QueryResult<Vec<Self>> {
        KitConfiguration::belonging_to(kit).load(conn)
    }

    pub fn configurations_of_kit_id(
        conn: &mut PgConnection,
        kit_id: KitId,
    ) -> QueryResult<Vec<Self>> {
        KitConfiguration::belonging_to(&kit_id).load(conn)
    }

    pub fn active_configuration_of_kit(
        conn: &mut PgConnection,
        kit: &Kit,
    ) -> QueryResult<Option<Self>> {
        Self::active_configuration_of_kit_id(conn, KitId(kit.id))
    }

    pub fn active_configuration_of_kit_id(
        conn: &mut PgConnection,
        kit_id: KitId,
    ) -> QueryResult<Option<Self>> {
        use kit_configurations::dsl;
        kit_configurations::table
            .filter(dsl::kit_id.eq(&kit_id.0))
            .filter(dsl::active.eq(true))
            .first(conn)
            .optional()
    }

    /**
     * Deactive all of the kit's configurations.
     * Returns the amount of deactivated configurations.
     */
    pub fn deactivate_all_of_kit(conn: &mut PgConnection, kit: &Kit) -> QueryResult<usize> {
        Self::deactivate_all_of_kit_id(conn, kit.get_id())
    }

    /**
     * Deactive all of the kit's configurations.
     * Returns the amount of deactivated configurations.
     */
    pub fn deactivate_all_of_kit_id(conn: &mut PgConnection, kit_id: KitId) -> QueryResult<usize> {
        use crate::schema::kit_configurations::dsl;

        diesel::update(dsl::kit_configurations.filter(dsl::kit_id.eq(&kit_id.0)))
            .set(dsl::active.eq(false))
            .execute(conn)
    }

    pub fn get_id(&self) -> KitConfigurationId {
        KitConfigurationId(self.id)
    }

    pub fn get_kit_id(&self) -> KitId {
        KitId(self.kit_id)
    }
}

impl UpdateKitConfiguration {
    pub fn update(&self, conn: &mut PgConnection) -> QueryResult<KitConfiguration> {
        self.save_changes(conn)
    }
}

#[derive(Clone, Debug, PartialEq, Insertable)]
#[diesel(table_name = kit_configurations)]
pub struct NewKitConfiguration {
    pub kit_id: i32,
    pub description: Option<String>,
    pub controller_symbol_location: String,
    pub controller_symbol: String,
    pub control_rules: serde_json::Value,
}

impl NewKitConfiguration {
    pub fn new(kit_id: KitId, description: Option<String>) -> Self {
        Self {
            kit_id: kit_id.0,
            description,
            controller_symbol_location: "astroplant_kit.controller".to_owned(),
            controller_symbol: "AstroplantControllerV1".to_owned(),
            control_rules: json!({}),
        }
    }

    pub fn create(&self, conn: &mut PgConnection) -> QueryResult<KitConfiguration> {
        use crate::schema::kit_configurations::dsl::*;

        diesel::insert_into(kit_configurations)
            .values(self)
            .on_conflict_do_nothing()
            .get_result::<KitConfiguration>(conn)
    }
}
