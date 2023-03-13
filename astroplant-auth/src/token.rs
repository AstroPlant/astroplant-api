//! TODO: add ability to revoke refresh tokens.

use std::convert::TryFrom;
use std::time::Duration;

use jsonwebtoken::{DecodingKey, EncodingKey};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
enum TokenType {
    Refresh,
    Access,
}

#[derive(Debug)]
pub enum Error {
    Expired,
    Other,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct AuthenticationState {
    pub user_id: i32,
}

impl AuthenticationState {
    pub fn new(user_id: i32) -> Self {
        Self { user_id }
    }
}

#[derive(Serialize, Deserialize)]
pub struct Claims {
    exp: usize,
    token_type: TokenType,
    state: AuthenticationState,
}

#[derive(Serialize, Deserialize)]
pub struct ArbitraryTokenClaims<T> {
    exp: usize,
    data: T,
}

pub struct TokenSigner {
    decoding_key: DecodingKey,
    encoding_key: EncodingKey,
}

impl TokenSigner {
    pub fn new(key: Vec<u8>) -> TokenSigner {
        TokenSigner {
            decoding_key: DecodingKey::from_secret(&key),
            encoding_key: EncodingKey::from_secret(&key),
        }
    }

    fn create_token(
        &self,
        validity_time: usize,
        token_type: TokenType,
        state: AuthenticationState,
    ) -> String {
        let now: usize = chrono::Utc::now().timestamp() as usize;
        let header = jsonwebtoken::Header::default();

        let token = Claims {
            exp: now + validity_time,
            token_type,
            state,
        };

        jsonwebtoken::encode(&header, &token, &self.encoding_key).unwrap()
    }

    fn decode_token(&self, token: &str) -> Result<Claims, Error> {
        let validation = jsonwebtoken::Validation::default();

        jsonwebtoken::decode::<Claims>(token, &self.decoding_key, &validation)
            .map_err(|e| match e.kind() {
                jsonwebtoken::errors::ErrorKind::ExpiredSignature => Error::Expired,
                _ => Error::Other,
            })
            .map(|t| t.claims)
    }

    pub fn create_refresh_token(&self, state: AuthenticationState) -> String {
        const VALIDITY_TIME: usize = 60 * 60 * 24 * 365;

        self.create_token(VALIDITY_TIME, TokenType::Refresh, state)
    }

    pub fn access_token_from_refresh_token(&self, token: &str) -> Result<String, Error> {
        const VALIDITY_TIME: usize = 60 * 15;

        let claims = self.decode_token(token)?;
        match claims.token_type {
            TokenType::Refresh => {
                Ok(self.create_token(VALIDITY_TIME, TokenType::Access, claims.state))
            }
            _ => Err(Error::Other),
        }
    }

    pub fn decode_access_token(&self, token: &str) -> Result<AuthenticationState, Error> {
        let claims = self.decode_token(token)?;
        match claims.token_type {
            TokenType::Access => Ok(claims.state),
            _ => Err(Error::Other),
        }
    }

    /// Sign a token with arbitrary data.
    ///
    /// TODO: Currently you must make sure the token data is completely unambiguous (the type
    /// signature is not encoded in the token). Perhaps there should be a global enum of allowed
    /// token types.
    pub fn create_arbitrary_token(&self, token: impl Serialize, validity_time: Duration) -> String {
        let now: usize = chrono::Utc::now().timestamp() as usize;
        let exp = now
            .checked_add(
                usize::try_from(validity_time.as_secs()).expect("duration that fits in a usize"),
            )
            .expect("token expiry overflowed");

        let header = jsonwebtoken::Header::default();

        let wrapped_token = ArbitraryTokenClaims { exp, data: token };

        jsonwebtoken::encode(&header, &wrapped_token, &self.encoding_key).unwrap()
    }

    pub fn decode_arbitrary_token<T: DeserializeOwned>(&self, token: &str) -> Result<T, Error> {
        let validation = jsonwebtoken::Validation::default();

        let wrapped_token =
            jsonwebtoken::decode::<ArbitraryTokenClaims<T>>(token, &self.decoding_key, &validation)
                .map_err(|e| match e.kind() {
                    jsonwebtoken::errors::ErrorKind::ExpiredSignature => Error::Expired,
                    _ => Error::Other,
                })
                .map(|t| t.claims)?;
        Ok(wrapped_token.data)
    }
}

#[cfg(test)]
mod test {
    #[test]
    pub fn token_round_trip() {
        let token_signer = super::TokenSigner::new(b"my server secret".to_vec());

        for _ in 0..100 {
            let id = rand::random::<i32>();

            let state = super::AuthenticationState { user_id: id };

            let refresh_token = token_signer.create_refresh_token(state.clone());
            let access_token = token_signer
                .access_token_from_refresh_token(&refresh_token)
                .unwrap();

            assert_eq!(
                state,
                token_signer.decode_access_token(&access_token).unwrap()
            );
        }
    }
}
