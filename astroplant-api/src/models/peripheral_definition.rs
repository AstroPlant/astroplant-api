use crate::schema::peripheral_definitions;

use diesel::pg::PgConnection;
use diesel::prelude::*;
use diesel::{Identifiable, QueryResult, Queryable};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Identifiable, Queryable)]
#[diesel(table_name = peripheral_definitions)]
pub struct PeripheralDefinitionId(#[diesel(column_name = id)] pub i32);

#[derive(Clone, Debug, PartialEq, Queryable, Identifiable)]
#[diesel(table_name = peripheral_definitions)]
pub struct PeripheralDefinition {
    pub id: i32,
    pub name: String,
    pub description: Option<String>,
    pub brand: Option<String>,
    pub model: Option<String>,
    pub symbol_location: String,
    pub symbol: String,
    pub configuration_schema: serde_json::Value,
    pub command_schema: Option<serde_json::Value>,
}

#[derive(Clone, Debug, PartialEq, Insertable, AsChangeset)]
#[diesel(table_name = peripheral_definitions)]
pub struct NewPeripheralDefinition {
    pub name: String,
    pub description: Option<String>,
    pub brand: Option<String>,
    pub model: Option<String>,
    pub symbol_location: String,
    pub symbol: String,
    pub configuration_schema: serde_json::Value,
    pub command_schema: Option<serde_json::Value>,
}

impl PeripheralDefinition {
    pub fn by_id(conn: &mut PgConnection, id: i32) -> QueryResult<Self> {
        peripheral_definitions::table.find(id).first(conn)
    }

    pub fn by_ids(conn: &mut PgConnection, ids: Vec<i32>) -> QueryResult<Vec<Self>> {
        use peripheral_definitions::dsl;
        peripheral_definitions::table
            .filter(dsl::id.eq_any(ids))
            .load(conn)
    }

    pub fn all(conn: &mut PgConnection) -> QueryResult<Vec<Self>> {
        peripheral_definitions::table.load(conn)
    }

    pub fn cursor_page(
        conn: &mut PgConnection,
        after: Option<i32>,
        limit: i64,
    ) -> QueryResult<Vec<Self>> {
        let q = peripheral_definitions::table
            .order(peripheral_definitions::columns::id.asc())
            .limit(limit);
        if let Some(after) = after {
            q.filter(peripheral_definitions::columns::id.gt(after))
                .load(conn)
        } else {
            q.load(conn)
        }
    }

    pub fn get_id(&self) -> PeripheralDefinitionId {
        PeripheralDefinitionId(self.id)
    }
}
