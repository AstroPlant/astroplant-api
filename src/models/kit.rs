use crate::schema::kits;
use diesel::prelude::*;
use diesel::{Connection, QueryResult, Queryable, Identifiable};
use diesel::pg::PgConnection;
use bigdecimal::BigDecimal;
use crate::views::EncodableKit;

#[derive(Clone, Debug, PartialEq, Eq, Queryable, Identifiable)]
#[table_name = "kits"]
pub struct Kit {
    pub id: i32,
    pub serial: String,
    pub password_hash: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub latitude: Option<BigDecimal>,
    pub longitude: Option<BigDecimal>,
    pub privacy_public_dashboard: bool,
    pub privacy_show_on_map: bool,
}

impl Kit {
    pub fn by_id(conn: &PgConnection, id: i32) -> QueryResult<Kit> {
        kits::table.find(id).first(conn)
    }

    pub fn all(conn: &PgConnection) -> QueryResult<Vec<Kit>> {
        kits::table.load(conn)
    }

    pub fn cursor_page(conn: &PgConnection, after: Option<i32>, limit: i64) -> QueryResult<Vec<Kit>> {
        let q = kits::table.order(kits::columns::id.asc()).limit(limit);
        if let Some(after) = after {
            q.filter(kits::columns::id.gt(after)).load(conn)
        } else {
            q.load(conn)
        }
    }

    pub fn encodable(self) -> EncodableKit {
        let Kit {
            id,
            serial,
            name,
            description,
            latitude,
            longitude,
            privacy_public_dashboard,
            privacy_show_on_map,
            ..
        } = self;
        EncodableKit {
            id,
            serial,
            name,
            description,
            latitude: latitude.map(|l| l.to_string()),
            longitude: longitude.map(|l| l.to_string()),
            privacy_public_dashboard,
            privacy_show_on_map,
        }
    }
}

#[derive(Insertable, Debug, Default)]
#[table_name = "kits"]
pub struct NewKit<'a> {
    pub serial: &'a str,
    pub password_hash: &'a str,
    pub name: Option<&'a str>,
    pub description: Option<&'a str>,
    pub latitude: Option<BigDecimal>,
    pub longitude: Option<BigDecimal>,
}

impl<'a> NewKit<'a> {
    pub fn new(
        serial: &'a str,
        password_hash: &'a str,
    ) -> Self {
        NewKit {
            serial,
            password_hash,
            name: None,
            description: None,
            latitude: None,
            longitude: None,
        }
    }

    pub fn create(&self, conn: &PgConnection) -> QueryResult<Option<Kit>> {
        use crate::schema::kits::dsl::*;

        conn.transaction(|| {
            let maybe_inserted = diesel::insert_into(kits)
                .values(self)
                .on_conflict_do_nothing()
                .get_result::<Kit>(conn)
                .optional()?;

            Ok(maybe_inserted)
        })
    }
}
