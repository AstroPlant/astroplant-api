use crate::schema::{kit_last_seen, kits};

use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use diesel::pg::PgConnection;
use diesel::prelude::*;
use diesel::{Identifiable, QueryResult, Queryable, Selectable};
use validator::Validate;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Identifiable)]
#[diesel(table_name = kits)]
pub struct KitId(#[diesel(column_name = id)] pub i32);

#[derive(Clone, Debug, PartialEq, Eq, Queryable, Identifiable, Selectable)]
#[diesel(table_name = kits)]
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
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Queryable, Identifiable, Associations)]
#[diesel(
    table_name = kit_last_seen,
    primary_key(kit_id),
    belongs_to(KitId, foreign_key = kit_id),
    belongs_to(Kit, foreign_key = kit_id),
)]
pub struct KitLastSeen {
    pub kit_id: i32,
    pub datetime_last_seen: DateTime<Utc>,
}

pub type All = diesel::dsl::Select<kits::table, diesel::dsl::AsSelect<Kit, diesel::pg::Pg>>;
pub type ById = diesel::dsl::Find<All, i32>;
pub type BySerial<'a> = diesel::dsl::Filter<All, diesel::dsl::Eq<kits::serial, &'a str>>;

pub type ShowOnMap = diesel::dsl::Eq<kits::privacy_show_on_map, bool>;
pub type PublicDashboard = diesel::dsl::Eq<kits::privacy_public_dashboard, bool>;
pub type Public = diesel::dsl::And<ShowOnMap, PublicDashboard>;

impl Kit {
    pub fn all() -> All {
        kits::table.select(Kit::as_select())
    }

    pub fn by_id(id: KitId) -> ById {
        Self::all().find(id.0)
    }

    pub fn by_serial(serial: &str) -> BySerial<'_> {
        Self::all().filter(kits::columns::serial.eq(serial))
    }

    /// Kits that are findable on the map
    pub fn show_on_map() -> ShowOnMap {
        kits::privacy_show_on_map.eq(true)
    }

    /// Kits that are publicly viewable by their serial
    pub fn public_dashboard() -> PublicDashboard {
        kits::privacy_public_dashboard.eq(true)
    }

    /// Kits that are findable on the map and publicly viewable by their serial
    pub fn public() -> Public {
        Self::show_on_map().and(Self::public_dashboard())
    }

    pub fn cursor_page(
        conn: &mut PgConnection,
        after: Option<i32>,
        limit: i64,
    ) -> QueryResult<Vec<Kit>> {
        let q = kits::table
            .filter(kits::columns::privacy_show_on_map.eq(true))
            .order(kits::columns::id.asc())
            .limit(limit);
        if let Some(after) = after {
            q.filter(kits::columns::id.gt(after)).load(conn)
        } else {
            q.load(conn)
        }
    }

    pub fn get_id(&self) -> KitId {
        KitId(self.id)
    }

    pub fn last_seen(&self, conn: &mut PgConnection) -> QueryResult<Option<DateTime<Utc>>> {
        kit_last_seen::table
            .select(kit_last_seen::datetime_last_seen)
            .find(self.id)
            .first(conn)
            .optional()
    }
}

#[derive(Clone, Debug, PartialEq, Queryable, Identifiable, AsChangeset)]
#[diesel(table_name = kits)]
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

    pub fn update(&self, conn: &mut PgConnection) -> QueryResult<Kit> {
        self.save_changes(conn)
    }
}

#[derive(Insertable, Debug, Default, Validate)]
#[diesel(table_name = kits)]
pub struct NewKit {
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

        // FIXME: the serial should be checked on the database for uniqueness.
        // Roughly 55 bits of entropy.
        let serial = format!(
            "k-{}-{}-{}",
            random_string::unambiguous_lowercase_string(4),
            random_string::unambiguous_lowercase_string(4),
            random_string::unambiguous_lowercase_string(4)
        );

        let new_kit = NewKit {
            serial,
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

    pub fn create(&self, conn: &mut PgConnection) -> QueryResult<Kit> {
        use crate::schema::kits::dsl::*;
        diesel::insert_into(kits)
            .values(self)
            .on_conflict_do_nothing()
            .get_result::<Kit>(conn)
    }
}
