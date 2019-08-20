use crate::schema::kit_memberships;

use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel::{Connection, QueryResult, Queryable, Identifiable};
use diesel::pg::PgConnection;

use super::{User, UserId};
use super::Kit;

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
}

impl KitMembership {
    pub fn memberships_of_kit(conn: &PgConnection, kit: &Kit) -> QueryResult<Vec<Self>> {
        KitMembership::belonging_to(kit).load(conn)
    }

    pub fn memberships_of_user_id(conn: &PgConnection, user_id: UserId) -> QueryResult<Vec<Self>> {
        KitMembership::belonging_to(&user_id).load(conn)
    }

    pub fn memberships_of_user(conn: &PgConnection, user: &User) -> QueryResult<Vec<Self>> {
        KitMembership::belonging_to(user).load(conn)
    }
}
