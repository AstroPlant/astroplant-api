use futures::future::FutureExt;
use serde::{Deserialize, Serialize};
use warp::{Filter, Rejection};

use crate::database::PgPool;
use crate::problem::{self, AppResult, Problem};
use crate::response::{Response, ResponseBuilder};
use crate::{helpers, models};

/// Authenticate a user through provided credentials.
/// Returns both a refresh token and authentication token.
pub fn authenticate_by_credentials(
    pg: PgPool,
) -> impl Filter<Extract = (AppResult<Response>,), Error = Rejection> + Clone {
    #[derive(Deserialize, Debug)]
    #[serde(rename_all = "camelCase")]
    struct AuthenticationDetails {
        username: String,
        password: String,
    }

    #[derive(Serialize, Debug)]
    #[serde(rename_all = "camelCase")]
    struct AuthenticationTokens {
        refresh_token: String,
        access_token: String,
    }

    async fn implementation(
        pg: PgPool,
        authentication_details: AuthenticationDetails,
    ) -> AppResult<Response> {
        use astroplant_auth::{hash, token};

        let conn = pg.get().await?;

        let (user, password) = helpers::threadpool(move || {
            let user_by_username =
                models::User::by_username(&conn, &authentication_details.username)?;
            Ok::<_, Problem>((user_by_username, authentication_details.password))
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
                    debug!("Authenticated user: {}.", user.username);

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

    crate::helpers::deserialize().and_then(move |authentication_details: AuthenticationDetails| {
        implementation(pg.clone(), authentication_details).never_error()
    })
}

/// Get an access token through a refresh token.
///
/// # TODO
/// Check refresh token against the database for revocation.
pub fn access_token_from_refresh_token(
) -> impl Filter<Extract = (AppResult<Response>,), Error = Rejection> + Clone {
    use astroplant_auth::token;
    use problem::{AccessTokenProblemCategory::*, InvalidParameterReason, InvalidParameters};

    #[derive(Deserialize, Debug)]
    #[serde(rename_all = "camelCase")]
    struct TaggedToken {
        refresh_token: String,
    }

    crate::helpers::deserialize().and_then(|TaggedToken { refresh_token }| async move {
        let token_signer: &token::TokenSigner = crate::TOKEN_SIGNER.get().unwrap();

        Ok(
            match token_signer.access_token_from_refresh_token(&refresh_token) {
                Ok(access_token) => {
                    trace!("Token refreshed.");
                    Ok(ResponseBuilder::ok().body(access_token))
                }
                Err(token::Error::Expired) => {
                    let mut invalid_parameters = InvalidParameters::new();
                    invalid_parameters.add(
                        "refreshToken",
                        InvalidParameterReason::InvalidToken { category: Expired },
                    );

                    return Err(Rejection::from(Problem::InvalidParameters {
                        invalid_parameters,
                    }));
                }
                Err(_) => {
                    let mut invalid_parameters = InvalidParameters::new();
                    invalid_parameters.add(
                        "refreshToken",
                        InvalidParameterReason::InvalidToken {
                            category: Malformed,
                        },
                    );

                    return Err(Rejection::from(Problem::InvalidParameters {
                        invalid_parameters,
                    }));
                }
            },
        )
    })
}
