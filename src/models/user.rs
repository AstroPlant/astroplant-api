use crate::schema::users;
use diesel::prelude::*;
use diesel::{Connection, QueryResult, Queryable, Identifiable};
use diesel::pg::PgConnection;

#[derive(Clone, Debug, PartialEq, Eq, Queryable, Identifiable)]
pub struct User {
    pub id: i32,
    pub username: String,
    pub password: String,
    pub email_address: String,
    pub use_gravatar: bool,
    pub gravatar_alternative: String,
}

#[derive(Insertable, Debug, Default)]
#[table_name = "users"]
pub struct NewUser<'a> {
    pub username: &'a str,
    pub password: &'a str,
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
            gravatar_alternative: "todo".to_owned(),
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
