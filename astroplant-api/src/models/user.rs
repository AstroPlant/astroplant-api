use crate::schema::users;

use chrono::{DateTime, Utc};
use diesel::pg::PgConnection;
use diesel::prelude::*;
use diesel::{Connection, Identifiable, QueryResult, Queryable};
use validator::{Validate, ValidationError};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Identifiable)]
#[diesel(table_name = users)]
pub struct UserId(#[diesel(column_name = id)] pub i32);

#[derive(Clone, Debug, PartialEq, Eq, Queryable, Identifiable)]
pub struct User {
    pub id: i32,
    pub username: String,
    pub display_name: String,
    pub password_hash: String,
    pub email_address: String,
    pub use_email_address_for_gravatar: bool,
    pub gravatar_alternative: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl User {
    pub fn by_id(conn: &mut PgConnection, id: UserId) -> QueryResult<Option<User>> {
        users::table.find(id.0).first(conn).optional()
    }

    pub fn by_username(conn: &mut PgConnection, username: &str) -> QueryResult<Option<User>> {
        users::table
            .filter(users::username.ilike(username))
            .first(conn)
            .optional()
    }

    pub fn by_email_address(
        conn: &mut PgConnection,
        email_address: &str,
    ) -> QueryResult<Option<User>> {
        users::table
            .filter(users::email_address.ilike(email_address))
            .first(conn)
            .optional()
    }

    pub fn get_id(&self) -> UserId {
        UserId(self.id)
    }
}

#[derive(Clone, Debug, PartialEq, Queryable, Identifiable, AsChangeset, Validate)]
#[diesel(table_name = users)]
pub struct UpdateUser {
    pub id: i32,
    // None means don't update, Some(None) means set to null.
    #[validate(length(min = 1, max = 40))]
    pub display_name: Option<String>,
    #[validate(length(max = 255))]
    #[validate(email)]
    pub email_address: Option<String>,
    pub password_hash: Option<String>,
    pub use_email_address_for_gravatar: Option<bool>,
}

impl UpdateUser {
    pub fn unchanged_for_id(id: i32) -> Self {
        UpdateUser {
            id,
            password_hash: None,
            display_name: None,
            email_address: None,
            use_email_address_for_gravatar: None,
        }
    }

    pub fn update(&self, conn: &mut PgConnection) -> QueryResult<User> {
        self.save_changes(conn)
    }
}

#[derive(Insertable, Debug, Default, Validate)]
#[diesel(table_name = users)]
pub struct NewUser {
    #[validate(length(min = 1), custom = "validate_username")]
    #[validate(length(min = 1, max = 40))]
    pub username: String,
    #[validate(length(min = 1, max = 40))]
    pub display_name: String,
    pub password_hash: String,

    #[validate(length(max = 255))]
    #[validate(email)]
    pub email_address: String,
    use_email_address_for_gravatar: bool,
    gravatar_alternative: String,
}

impl NewUser {
    pub fn new(username: String, password_hash: String, email_address: String) -> Self {
        NewUser {
            username: username.to_lowercase(),
            display_name: username,
            password_hash,

            // TODO: in principle, only the host-part of the email address should be lowercased.
            email_address: email_address.to_lowercase(),

            use_email_address_for_gravatar: true,
            gravatar_alternative: random_string::unambiguous_string(32),
        }
    }

    pub fn create(&self, conn: &mut PgConnection) -> QueryResult<Option<User>> {
        use crate::schema::users::dsl::*;

        conn.transaction(|conn| {
            let maybe_inserted = diesel::insert_into(users)
                .values(self)
                .on_conflict_do_nothing()
                .get_result::<User>(conn)
                .optional()?;

            Ok(maybe_inserted)
        })
    }
}

fn validate_username(username: &str) -> Result<(), ValidationError> {
    if !username.chars().all(|c| c.is_alphanumeric() || c == '-')
        || username.starts_with('-')
        || username.ends_with('-')
    {
        Err(ValidationError::new("invalid_username"))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::NewUser;
    use validator::{Validate, ValidationErrors};

    #[test]
    fn reject_empty_username() {
        let user = NewUser::new(
            "".to_owned(),
            "".to_owned(),
            "example@example.com".to_owned(),
        );
        assert!(ValidationErrors::has_error(&user.validate(), "username"));
    }

    #[test]
    fn reject_long_username() {
        let user = NewUser::new(
            vec!['a'; 41].into_iter().collect(),
            "".to_owned(),
            "example@example.com".to_owned(),
        );
        assert!(ValidationErrors::has_error(&user.validate(), "username"));
    }

    #[test]
    fn reject_username_beginning_or_ending_with_hyphen() {
        let user = NewUser::new(
            "-example".to_owned(),
            "".to_owned(),
            "example@example.com".to_owned(),
        );
        assert!(ValidationErrors::has_error(&user.validate(), "username"));

        let user = NewUser::new(
            "example-".to_owned(),
            "".to_owned(),
            "example@example.com".to_owned(),
        );
        assert!(ValidationErrors::has_error(&user.validate(), "username"));
    }

    #[test]
    fn reject_invalid_email_address() {
        let user = NewUser::new(
            "example".to_owned(),
            "".to_owned(),
            "example.com".to_owned(),
        );
        assert!(ValidationErrors::has_error(
            &user.validate(),
            "email_address"
        ));
    }

    #[test]
    fn accept_valid_username_and_email_address() {
        let user = NewUser::new(
            "123example-example".to_owned(),
            "".to_owned(),
            "example@example.com".to_owned(),
        );
        assert!(user.validate().is_ok());
    }
}
