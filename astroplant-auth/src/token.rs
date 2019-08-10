//! TODO: add ability to revoke refresh tokens.

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
enum TokenType {
    Refresh,
    Normal,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct AuthenticationState {
    pub user_id: i32,
}

#[derive(Serialize, Deserialize)]
pub struct Claims {
    exp: usize,
    token_type: TokenType,
    state: AuthenticationState,
}

pub struct TokenSigner {
    key: Vec<u8>,
}

impl TokenSigner {
    pub fn new(key: Vec<u8>) -> TokenSigner {
        TokenSigner { key }
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

        jsonwebtoken::encode(&header, &token, &self.key).unwrap()
    }

    fn decode_token(&self, token: &str) -> Option<Claims> {
        let validation = jsonwebtoken::Validation::default();

        jsonwebtoken::decode::<Claims>(token, &self.key, &validation)
            .ok()
            .map(|t| t.claims)
    }

    pub fn create_refresh_token(&self, state: AuthenticationState) -> String {
        const VALIDITY_TIME: usize = 60 * 60 * 24 * 365;

        self.create_token(VALIDITY_TIME, TokenType::Refresh, state)
    }

    pub fn normal_token_from_refresh_token(&self, token: &str) -> Option<String> {
        const VALIDITY_TIME: usize = 60 * 15;

        let claims = self.decode_token(token)?;
        match claims.token_type {
            TokenType::Refresh => Some(self.create_token(VALIDITY_TIME, TokenType::Normal, claims.state)),
            _ => None,
        }
    }

    pub fn decode_normal_token(&self, token: &str) -> Option<AuthenticationState> {
        let claims = self.decode_token(token)?;
        match claims.token_type {
            TokenType::Normal => Some(claims.state),
            _ => None,
        }
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
            let normal_token = token_signer.normal_token_from_refresh_token(&refresh_token).unwrap();

            assert_eq!(
                state,
                token_signer
                    .decode_normal_token(&normal_token)
                    .unwrap()
            );
        }
    }
}
