use crate::schema::quantity_types;

use diesel::pg::PgConnection;
use diesel::prelude::*;
use diesel::{Identifiable, QueryResult, Queryable};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Identifiable)]
#[table_name = "quantity_types"]
pub struct QuantityTypeId(#[column_name = "id"] pub i32);

#[derive(Clone, Debug, PartialEq, Queryable, Identifiable)]
#[table_name = "quantity_types"]
pub struct QuantityType {
    pub id: i32,
    pub physical_quantity: String,
    pub physical_unit: String,
    pub physical_unit_symbol: Option<String>,
}

impl QuantityType {
    pub fn by_id(conn: &mut PgConnection, id: i32) -> QueryResult<Self> {
        quantity_types::table.find(id).first(conn)
    }

    pub fn by_ids(conn: &mut PgConnection, ids: Vec<i32>) -> QueryResult<Vec<Self>> {
        use quantity_types::dsl;
        quantity_types::table
            .filter(dsl::id.eq_any(ids))
            .load(conn)
    }

    pub fn all(conn: &mut PgConnection) -> QueryResult<Vec<Self>> {
        quantity_types::table.load(conn)
    }

    pub fn cursor_page(
        conn: &mut PgConnection,
        after: Option<i32>,
        limit: i64,
    ) -> QueryResult<Vec<Self>> {
        let q = quantity_types::table
            .order(quantity_types::columns::id.asc())
            .limit(limit);
        if let Some(after) = after {
            q.filter(quantity_types::columns::id.gt(after)).load(conn)
        } else {
            q.load(conn)
        }
    }

    pub fn get_id(&self) -> QuantityTypeId {
        QuantityTypeId(self.id)
    }
}
