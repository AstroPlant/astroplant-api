use crate::schema::{kit_memberships, kits};

use chrono::{DateTime, Utc};
use diesel::pg::PgConnection;
use diesel::prelude::*;
use diesel::{Identifiable, QueryResult, Queryable};

use super::{Kit, KitId};
use super::{User, UserId};

#[derive(
    Clone, Debug, PartialEq, Eq, Queryable, Identifiable, Associations, AsChangeset, Selectable,
)]
#[diesel(
    table_name = kit_memberships,
    belongs_to(User),
    belongs_to(UserId, foreign_key = user_id),
    belongs_to(Kit),
)]
pub struct KitMembership {
    pub id: i32,
    pub user_id: i32,
    pub kit_id: i32,
    pub datetime_linked: DateTime<Utc>,
    pub access_super: bool,
    pub access_configure: bool,
}

pub type All = diesel::dsl::Select<
    kit_memberships::table,
    diesel::dsl::AsSelect<KitMembership, diesel::pg::Pg>,
>;

pub type WithUserId = diesel::dsl::Eq<kit_memberships::user_id, i32>;
pub type ByUserId = diesel::dsl::Filter<All, WithUserId>;

impl KitMembership {
    pub fn all() -> All {
        kit_memberships::table.select(KitMembership::as_select())
    }

    pub fn with_user_id(user_id: UserId) -> WithUserId {
        kit_memberships::user_id.eq(user_id.0)
    }

    pub fn by_user_id(user_id: UserId) -> ByUserId {
        Self::all().filter(Self::with_user_id(user_id))
    }

    pub fn memberships_of_user_id(
        conn: &mut PgConnection,
        user_id: UserId,
    ) -> QueryResult<Vec<Self>> {
        KitMembership::belonging_to(&user_id).load(conn)
    }

    pub fn memberships_with_kit_of_user_id(
        conn: &mut PgConnection,
        user_id: UserId,
    ) -> QueryResult<Vec<(Kit, Self)>> {
        // TODO: benchmark; this join might be less efficient than performing two queries with
        // belonging_to.
        kits::table
            .inner_join(kit_memberships::table)
            .filter(kit_memberships::dsl::user_id.eq(user_id.0))
            .get_results(conn)
    }

    pub fn by_user_id_and_kit_id(
        conn: &mut PgConnection,
        user_id: UserId,
        kit_id: KitId,
    ) -> QueryResult<Option<Self>> {
        use kit_memberships::dsl;
        kit_memberships::table
            .filter(dsl::user_id.eq(&user_id.0).and(dsl::kit_id.eq(&kit_id.0)))
            .first(conn)
            .optional()
    }

    pub fn by_user_and_kit(
        conn: &mut PgConnection,
        user: &User,
        kit: &Kit,
    ) -> QueryResult<Option<Self>> {
        Self::by_user_id_and_kit_id(conn, UserId(user.id), KitId(kit.id))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Insertable)]
#[diesel(table_name = kit_memberships)]
pub struct NewKitMembership {
    pub user_id: i32,
    pub kit_id: i32,
    pub datetime_linked: DateTime<Utc>,
    pub access_super: bool,
    pub access_configure: bool,
}

impl NewKitMembership {
    pub fn new(user_id: UserId, kit_id: KitId, access_super: bool, access_configure: bool) -> Self {
        Self {
            user_id: user_id.0,
            kit_id: kit_id.0,
            datetime_linked: Utc::now(),
            access_super,
            access_configure,
        }
    }

    pub fn create(&self, conn: &mut PgConnection) -> QueryResult<KitMembership> {
        use crate::schema::kit_memberships::dsl::*;

        diesel::insert_into(kit_memberships)
            .values(self)
            .on_conflict_do_nothing()
            .get_result::<KitMembership>(conn)
    }
}
