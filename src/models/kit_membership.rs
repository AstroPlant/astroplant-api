use crate::schema::{kit_memberships, kits};

use chrono::{DateTime, Utc};
use diesel::pg::PgConnection;
use diesel::prelude::*;
use diesel::{Identifiable, QueryResult, Queryable};

use super::{Kit, KitId};
use super::{User, UserId};

#[derive(Clone, Debug, PartialEq, Eq, Queryable, Identifiable, Associations)]
#[belongs_to(parent = "User")]
#[belongs_to(parent = "UserId", foreign_key = "user_id")]
#[belongs_to(parent = "Kit")]
#[table_name = "kit_memberships"]
pub struct KitMembership {
    pub id: i32,
    pub user_id: i32,
    pub kit_id: i32,
    pub datetime_linked: DateTime<Utc>,
    pub access_super: bool,
    pub access_configure: bool,
}

impl KitMembership {
    pub fn memberships_of_kit(conn: &PgConnection, kit: &Kit) -> QueryResult<Vec<Self>> {
        KitMembership::belonging_to(kit).load(conn)
    }

    pub fn memberships_of_user_id(conn: &PgConnection, user_id: UserId) -> QueryResult<Vec<Self>> {
        KitMembership::belonging_to(&user_id).load(conn)
    }

    pub fn memberships_with_kit_of_user_id(
        conn: &PgConnection,
        user_id: UserId,
    ) -> QueryResult<Vec<(Kit, Self)>> {
        // TODO: benchmark; this join might be less efficient than performing two queries with
        // belonging_to.
        kits::table
            .inner_join(kit_memberships::table)
            .filter(kit_memberships::dsl::user_id.eq(user_id.0))
            .get_results(conn)
    }

    pub fn memberships_of_user(conn: &PgConnection, user: &User) -> QueryResult<Vec<Self>> {
        KitMembership::belonging_to(user).load(conn)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Insertable)]
#[table_name = "kit_memberships"]
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

    pub fn create(&self, conn: &PgConnection) -> QueryResult<KitMembership> {
        use crate::schema::kit_memberships::dsl::*;

        diesel::insert_into(kit_memberships)
            .values(self)
            .on_conflict_do_nothing()
            .get_result::<KitMembership>(conn)
    }
}
