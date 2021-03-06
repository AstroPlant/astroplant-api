use crate::problem::{AccessTokenProblemCategory::*, Problem};

use crate::models::UserId;

use astroplant_auth::token;
use warp::{Filter, Rejection};

/// A filter to authenticate a user through a normal token in the Authorization header.
/// If there is no Authorization header, returns None.
///
/// Rejects the request if the Authorization header is malformed.
pub fn option_by_token() -> impl Filter<Extract = (Option<UserId>,), Error = Rejection> + Copy {
    warp::header("Authorization")
        .map(|a| Some(a))
        .or_else(|_| futures::future::ok::<_, Rejection>((None,)))
        .and_then(|authorization: Option<String>| {
            async {
                if let Some(authorization) = authorization {
                    let parts: Vec<_> = authorization.split(" ").collect();
                    if parts.len() != 2 {
                        return Err(warp::reject::custom(Problem::AuthorizationHeader {
                            category: Malformed,
                        }));
                    }

                    if parts[0] != "Bearer" {
                        return Err(warp::reject::custom(Problem::AuthorizationHeader {
                            category: Malformed,
                        }));
                    }

                    let token_signer: &token::TokenSigner = crate::TOKEN_SIGNER.get().unwrap();

                    let access_token = parts[1];
                    let authentication_state: token::AuthenticationState =
                        match token_signer.decode_access_token(&access_token) {
                            Ok(authentication_state) => authentication_state,
                            Err(token::Error::Expired) => {
                                return Err(warp::reject::custom(Problem::AuthorizationHeader {
                                    category: Expired,
                                }))
                            }
                            Err(_) => {
                                return Err(warp::reject::custom(Problem::AuthorizationHeader {
                                    category: Malformed,
                                }))
                            }
                        };

                    tracing::trace!("User authenticated with state {:?}", authentication_state);
                    Ok(Some(UserId(authentication_state.user_id)))
                } else {
                    Ok(None)
                }
            }
        })
}

/// A filter to authenticate a user through a normal token in the Accept header.
/// Rejects the request if the Authorization header is missing or malformed.
pub fn by_token() -> impl Filter<Extract = (UserId,), Error = Rejection> + Copy {
    option_by_token().and_then(|user: Option<UserId>| {
        futures::future::ready(
            user.ok_or(warp::reject::custom(Problem::AuthorizationHeader {
                category: Missing,
            })),
        )
    })
}
