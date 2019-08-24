use serde::{Deserialize, Serialize};
use warp::{filters::BoxedFilter, Filter, Rejection};

use crate::helpers;
use crate::models;
use crate::problem::{self, Problem};
use crate::response::{Response, ResponseBuilder};

/// Authenticate a user through provided credentials.
/// Returns both a refresh token and normal token.
pub fn authenticate_by_credentials(
    pg: BoxedFilter<(crate::PgPooled,)>,
) -> impl Filter<Extract = (Response,), Error = Rejection> + Clone {
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
        normal_token: String,
    }

    crate::helpers::deserialize()
        .and(pg)
        .and_then(
            |authentication_details: AuthenticationDetails, conn: crate::PgPooled| {
                helpers::threadpool_diesel_ok(move || {
                    let user_by_username =
                        models::User::by_username(&conn, &authentication_details.username)?;
                    Ok((user_by_username, authentication_details.password))
                })
            },
        )
        .and_then(|(user, password): (Option<models::User>, String)| {
            use astroplant_auth::{hash, token};

            match user {
                Some(user) => {
                    if hash::check_user_password(&password, &user.password_hash) {
                        let token_signer: &token::TokenSigner = crate::TOKEN_SIGNER.get().unwrap();

                        let authentication_state = token::AuthenticationState::new(user.id);
                        let refresh_token = token_signer.create_refresh_token(authentication_state);
                        let normal_token = token_signer
                            .normal_token_from_refresh_token(&refresh_token)
                            .unwrap();
                        debug!("Authenticated user: {}.", user.username);

                        let response = ResponseBuilder::ok().body(AuthenticationTokens {
                            refresh_token,
                            normal_token,
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

            Err(warp::reject::custom(Problem::InvalidParameters {
                invalid_parameters,
            }))
        })
}

/// Get a normal token through a refresh token.
///
/// # TODO
/// Check refresh token against the database for revocation.
pub fn normal_token_from_refresh_token(
) -> impl Filter<Extract = (Response,), Error = Rejection> + Clone {
    use astroplant_auth::token;
    use problem::{
        AuthenticationTokenProblemCategory::*, InvalidParameterReason, InvalidParameters,
    };

    #[derive(Deserialize, Debug)]
    #[serde(rename_all = "camelCase")]
    struct TaggedToken {
        refresh_token: String,
    }

    crate::helpers::deserialize().and_then(|TaggedToken { refresh_token }| {
        let token_signer: &token::TokenSigner = crate::TOKEN_SIGNER.get().unwrap();

        match token_signer.normal_token_from_refresh_token(&refresh_token) {
            Ok(normal_token) => {
                trace!("Token refreshed.");
                Ok(ResponseBuilder::ok().body(normal_token))
            }
            Err(token::Error::Expired) => {
                let mut invalid_parameters = InvalidParameters::new();
                invalid_parameters.add(
                    "refreshToken",
                    InvalidParameterReason::InvalidToken { category: Expired },
                );

                return Err(warp::reject::custom(Problem::InvalidParameters {
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

                return Err(warp::reject::custom(Problem::InvalidParameters {
                    invalid_parameters,
                }));
            }
        }
    })
}
