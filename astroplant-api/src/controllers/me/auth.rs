use axum::Extension;

use serde::{Deserialize, Serialize};

use astroplant_auth::{hash, token};

use crate::database::PgPool;
use crate::models;
use crate::problem::{self, Problem};
use crate::response::{Response, ResponseBuilder};

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AuthenticationDetails {
    username: String,
    password: String,
}

pub async fn authenticate_by_credentials(
    Extension(pg): Extension<PgPool>,
    authentication_details: crate::extract::Json<AuthenticationDetails>,
) -> Result<Response, Problem> {
    #[derive(Serialize, Debug)]
    #[serde(rename_all = "camelCase")]
    struct AuthenticationTokens {
        refresh_token: String,
        access_token: String,
    }

    let conn = pg.get().await?;

    let (user, password) = conn
        .interact_flatten_err(move |conn| {
            let user_by_username =
                models::User::by_username(conn, &authentication_details.username)?;
            Ok::<_, Problem>((user_by_username, authentication_details.0.password))
        })
        .await?;

    match user {
        Some(user) => {
            if hash::check_user_password(&password, &user.password_hash) {
                let token_signer: &token::TokenSigner = crate::TOKEN_SIGNER.get().unwrap();

                let authentication_state = token::AuthenticationState::new(user.id);
                let refresh_token = token_signer.create_refresh_token(authentication_state);
                let access_token = token_signer
                    .access_token_from_refresh_token(&refresh_token)
                    .unwrap();
                tracing::debug!("Authenticated user: {}.", user.username);

                let response = ResponseBuilder::ok().body(AuthenticationTokens {
                    refresh_token,
                    access_token,
                });

                return Ok(response);
            }
        }
        None => {
            // Probably unnecessary, but hash the provided password to help defeat timing
            // attacks.
            hash::hash_user_password(&password);
        }
    }

    let mut invalid_parameters = problem::InvalidParameters::new();
    invalid_parameters.add("username", problem::InvalidParameterReason::Other);
    invalid_parameters.add("password", problem::InvalidParameterReason::Other);

    Err(Problem::InvalidParameters { invalid_parameters })
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TaggedToken {
    refresh_token: String,
}

/// Get an access token through a refresh token.
///
/// # TODO
/// Check refresh token against the database for revocation.
pub async fn access_token_from_refresh_token(
    crate::extract::Json(TaggedToken { refresh_token }): crate::extract::Json<TaggedToken>,
) -> Result<Response, Problem> {
    use problem::{AccessTokenProblemCategory::*, InvalidParameterReason, InvalidParameters};

    let token_signer: &token::TokenSigner = crate::TOKEN_SIGNER.get().unwrap();

    match token_signer.access_token_from_refresh_token(&refresh_token) {
        Ok(access_token) => {
            tracing::trace!("Token refreshed.");
            Ok(ResponseBuilder::ok().body(access_token))
        }
        Err(token::Error::Expired) => {
            let mut invalid_parameters = InvalidParameters::new();
            invalid_parameters.add(
                "refreshToken",
                InvalidParameterReason::InvalidToken { category: Expired },
            );

            Err(Problem::InvalidParameters { invalid_parameters })
        }
        Err(_) => {
            let mut invalid_parameters = InvalidParameters::new();
            invalid_parameters.add(
                "refreshToken",
                InvalidParameterReason::InvalidToken {
                    category: Malformed,
                },
            );

            Err(Problem::InvalidParameters { invalid_parameters })
        }
    }
}
