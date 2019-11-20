use crate::schema::kits;

use bigdecimal::BigDecimal;
use diesel::pg::PgConnection;
use diesel::prelude::*;
use diesel::{Identifiable, QueryResult, Queryable};
use validator::Validate;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Identifiable)]
#[table_name = "kits"]
pub struct KitId(#[column_name = "id"] pub i32);

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

    pub fn by_serial(conn: &PgConnection, serial: String) -> QueryResult<Option<Kit>> {
        kits::table
            .filter(kits::columns::serial.eq(serial))
            .first(conn)
            .optional()
    }

    pub fn all(conn: &PgConnection) -> QueryResult<Vec<Kit>> {
        kits::table.load(conn)
    }

    pub fn cursor_page(
        conn: &PgConnection,
        after: Option<i32>,
        limit: i64,
    ) -> QueryResult<Vec<Kit>> {
        let q = kits::table.order(kits::columns::id.asc()).limit(limit);
        if let Some(after) = after {
            q.filter(kits::columns::id.gt(after)).load(conn)
        } else {
            q.load(conn)
        }
    }

    pub fn get_id(&self) -> KitId {
        KitId(self.id)
    }
}

#[derive(Clone, Debug, PartialEq, Queryable, Identifiable, AsChangeset)]
#[table_name = "kits"]
pub struct UpdateKit {
    pub id: i32,
    pub password_hash: Option<String>,
    // None means don't update, Some(None) means set to null.
    pub name: Option<Option<String>>,
    pub description: Option<Option<String>>,
    pub latitude: Option<Option<BigDecimal>>,
    pub longitude: Option<Option<BigDecimal>>,
    pub privacy_public_dashboard: Option<bool>,
    pub privacy_show_on_map: Option<bool>,
}

impl UpdateKit {
    pub fn unchanged_for_id(id: i32) -> Self {
        UpdateKit {
            id,
            password_hash: None,
            name: None,
            description: None,
            latitude: None,
            longitude: None,
            privacy_public_dashboard: None,
            privacy_show_on_map: None,
        }
    }

    pub fn reset_password(mut self) -> (Self, String) {
        let password = random_string::password();
        self.password_hash = Some(astroplant_auth::hash::hash_kit_password(&password));
        (self, password)
    }

    pub fn update(&self, conn: &PgConnection) -> QueryResult<Kit> {
        self.save_changes(conn)
    }
}

#[derive(Insertable, Debug, Default, Validate)]
#[table_name = "kits"]
pub struct NewKit {
    #[validate(length(equal = 14))]
    pub serial: String,
    pub password_hash: String,
    #[validate(length(min = 1, max = 40))]
    pub name: Option<String>,
    #[validate(length(min = 1, max = 5000))]
    pub description: Option<String>,
    pub latitude: Option<BigDecimal>,
    pub longitude: Option<BigDecimal>,
    pub privacy_public_dashboard: bool,
    pub privacy_show_on_map: bool,
}

impl NewKit {
    /// Creates a new kit and returns the generated password.
    pub fn new_with_generated_password(
        name: Option<String>,
        description: Option<String>,
        latitude: Option<BigDecimal>,
        longitude: Option<BigDecimal>,
        privacy_public_dashboard: bool,
        privacy_show_on_map: bool,
    ) -> (Self, String) {
        let password = random_string::password();
        let password_hash = astroplant_auth::hash::hash_kit_password(&password);

        let new_kit = NewKit {
            serial: random_string::unambiguous_lowercase_string(14),
            password_hash,
            name,
            description,
            latitude,
            longitude,
            privacy_public_dashboard,
            privacy_show_on_map,
        };

        (new_kit, password)
    }

    pub fn create(&self, conn: &PgConnection) -> QueryResult<Kit> {
        use crate::schema::kits::dsl::*;
        diesel::insert_into(kits)
            .values(self)
            .on_conflict_do_nothing()
            .get_result::<Kit>(conn)
    }
}
