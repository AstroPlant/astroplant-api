use crate::problem::{AuthenticationTokenProblemCategory::*, Problem};

use crate::models::UserId;

use astroplant_auth::token;
use warp::{Filter, Rejection};

/// A filter to authenticate a user through a normal token in the Accept header.
pub fn authenticate_by_token() -> impl Filter<Extract = (UserId,), Error = Rejection> + Copy {
    warp::header("Authorization")
        .or_else(|_| {
            Err(warp::reject::custom(Problem::AuthorizationHeader {
                category: Missing,
            }))
        })
        .and_then(|authorization: String| {
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

            let authentication_token = parts[1];
            let authentication_state: token::AuthenticationState =
                match token_signer.decode_authentication_token(&authentication_token) {
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

            trace!("User authenticated with state {:?}", authentication_state);
            Ok(UserId(authentication_state.user_id))
        })
}
