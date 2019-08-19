use crate::schema::users;
use diesel::prelude::*;
use diesel::{Connection, QueryResult, Queryable, Identifiable};
use diesel::pg::PgConnection;
use validator::Validate;

#[derive(Clone, Debug, PartialEq, Eq, Queryable, Identifiable)]
pub struct User {
    pub id: i32,
    pub username: String,
    pub password: String,
    pub email_address: String,
    pub use_gravatar: bool,
    pub gravatar_alternative: String,
}

impl User {
    pub fn by_username(conn: &PgConnection, username: &str) -> QueryResult<Option<User>> {
        users::table.filter(users::username.eq(username)).first(conn).optional()
    }

    pub fn by_email_address(conn: &PgConnection, email_address: &str) -> QueryResult<Option<User>> {
        users::table.filter(users::email_address.eq(email_address)).first(conn).optional()
    }
}

#[derive(Insertable, Debug, Default, Validate)]
#[table_name = "users"]
pub struct NewUser<'a> {
    #[validate(length(min = 1, max = 40))]
    pub username: &'a str,
    pub password: &'a str,

    #[validate(length(max = 255))]
    #[validate(email)]
    pub email_address: &'a str,
    use_gravatar: bool,
    gravatar_alternative: String,
}

impl<'a> NewUser<'a> {
    pub fn new(
        username: &'a str,
        password: &'a str,
        email_address: &'a str,
    ) -> Self {
        NewUser {
            username,
            password,
            email_address,
            use_gravatar: true,
            gravatar_alternative: random_string::readable_string(32),
        }
    }

    pub fn create(&self, conn: &PgConnection) -> QueryResult<Option<User>> {
        use crate::schema::users::dsl::*;

        conn.transaction(|| {
            let maybe_inserted = diesel::insert_into(users)
                .values(self)
                .on_conflict_do_nothing()
                .get_result::<User>(conn)
                .optional()?;

            Ok(maybe_inserted)
        })
    }
}
