use std::ops::Deref;

use async_trait::async_trait;
use axum::{
    extract::rejection::JsonRejection,
    extract::{FromRequest, RequestParts, TypedHeader},
    headers::{authorization::Bearer, Authorization},
    BoxError,
};
use serde::de::DeserializeOwned;

use crate::problem::{self, Problem};

#[derive(Debug, Clone, Copy, Default)]
pub struct Json<T>(pub T);

#[derive(Debug, Clone, Copy, Default)]
pub struct Query<T>(pub T);

pub type UserId = crate::models::UserId;

#[async_trait]
impl<B, T> FromRequest<B> for Json<T>
where
    // these trait bounds are copied from `impl FromRequest for axum::Json`
    T: DeserializeOwned,
    B: axum::body::HttpBody + Send,
    B::Data: Send,
    B::Error: Into<BoxError>,
{
    type Rejection = Problem;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        match axum::Json::<T>::from_request(req).await {
            Ok(value) => Ok(Self(value.0)),
            Err(rejection) => {
                let problem = match rejection {
                    JsonRejection::JsonDataError(_) | JsonRejection::JsonSyntaxError(_) => {
                        problem::BAD_REQUEST
                    }
                    _err => problem::INTERNAL_SERVER_ERROR,
                };
                Err(problem)
            }
        }
    }
}

#[async_trait]
impl<B, T> FromRequest<B> for Query<T>
where
    // these trait bounds are copied from `impl FromRequest for axum::extract::Query`
    T: DeserializeOwned,
    B: Send,
{
    type Rejection = Problem;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        match axum::extract::Query::<T>::from_request(req).await {
            Ok(value) => Ok(Self(value.0)),
            Err(_rejection) => Err(problem::BAD_REQUEST),
        }
    }
}

#[async_trait]
impl<B> FromRequest<B> for UserId
where
    B: Send,
{
    type Rejection = problem::AccessTokenProblemCategory;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        let TypedHeader(Authorization(bearer)) =
            TypedHeader::<Authorization<Bearer>>::from_request(req)
                .await
                .map_err(|rejection| match rejection.reason() {
                    axum::extract::rejection::TypedHeaderRejectionReason::Missing => {
                        problem::AccessTokenProblemCategory::Missing
                    }
                    _ => problem::AccessTokenProblemCategory::Malformed,
                })?;

        let token_signer = crate::TOKEN_SIGNER.get().unwrap();
        let access_token = bearer.token();
        let authentication_state: astroplant_auth::token::AuthenticationState =
            match token_signer.decode_access_token(access_token) {
                Ok(authentication_state) => authentication_state,
                Err(astroplant_auth::token::Error::Expired) => {
                    return Err(problem::AccessTokenProblemCategory::Expired)
                }
                Err(_) => return Err(problem::AccessTokenProblemCategory::Malformed),
            };

        tracing::trace!("User authenticated with state {:?}", authentication_state);
        Ok(crate::models::UserId(authentication_state.user_id))
    }
}

impl<T> Deref for Json<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> Deref for Query<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Deref for UserId {
    type Target = i32;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
