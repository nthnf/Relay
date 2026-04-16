use std::time::Duration;

use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng},
};
use jwt_simple::prelude::*;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use thiserror::Error;
use uuid::Uuid;

pub const ACCESS_TOKEN_VALIDITY: Duration = Duration::from_secs(15 * 60);

#[derive(Debug, Error)]
pub enum PasswordAuthError {
    #[error("password hash error")]
    PasswordHash,
}

#[derive(Debug, Error)]
pub enum TokenAuthError {
    #[error("jwt error: {0}")]
    Jwt(#[from] jwt_simple::Error),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccessClaims {
    pub user_id: Uuid,
    pub session_id: Uuid,
}

#[derive(Clone)]
pub struct AuthKeys {
    access_signing_key: HS256Key,
}

impl AuthKeys {
    pub fn from_shared_secret(secret: &[u8]) -> Self {
        Self {
            access_signing_key: HS256Key::from_bytes(secret),
        }
    }

    pub fn sign_access_token(&self, claims: AccessClaims) -> Result<String, TokenAuthError> {
        let claims = Claims::with_custom_claims(claims, ACCESS_TOKEN_VALIDITY.into());
        Ok(self.access_signing_key.authenticate(claims)?)
    }

    pub fn verify_access_token(&self, token: &str) -> Result<AccessClaims, TokenAuthError> {
        let claims = self
            .access_signing_key
            .verify_token::<AccessClaims>(token, None)?;
        Ok(claims.custom)
    }
}

pub fn hash_password(secret: &str) -> Result<String, PasswordAuthError> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(secret.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|_| PasswordAuthError::PasswordHash)
}

pub fn verify_password(secret: &str, hash: &str) -> Result<bool, PasswordAuthError> {
    let parsed = PasswordHash::new(hash).map_err(|_| PasswordAuthError::PasswordHash)?;
    Ok(Argon2::default()
        .verify_password(secret.as_bytes(), &parsed)
        .is_ok())
}

pub fn hash_token(token: &str) -> String {
    Sha256::digest(token.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub fn verify_token(token: &str, hash: &str) -> bool {
    hash_token(token).as_bytes().ct_eq(hash.as_bytes()).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_hash_round_trips_plaintext_passwords() {
        let password = "correct horse battery staple";
        let hash = hash_password(password).expect("password hashing should succeed");

        assert!(verify_password(password, &hash).expect("password verification should succeed"));
        assert!(
            !verify_password("wrong password", &hash)
                .expect("password verification should succeed")
        );
    }

    #[test]
    fn opaque_token_hashing_is_deterministic() {
        let token = "opaque-token-value";

        let first = hash_token(token);
        let second = hash_token(token);

        assert_eq!(first, second);
        assert!(verify_token(token, &first));
        assert!(!verify_token("different-token", &first));
    }

    #[test]
    fn generic_token_helper_verifies_matching_and_non_matching_tokens() {
        let token = "email-verification-token";
        let hash = hash_token(token);

        assert!(verify_token(token, &hash));
        assert!(!verify_token("different-token", &hash));
    }

    #[test]
    fn access_tokens_round_trip_stable_session_identity_claims() {
        let auth = AuthKeys::from_shared_secret(b"test-secret-key");
        let claims = AccessClaims {
            user_id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
        };

        let token = auth
            .sign_access_token(claims.clone())
            .expect("access token signing should succeed");

        assert_eq!(
            auth.verify_access_token(&token)
                .expect("access token verification should succeed"),
            claims
        );
    }

    #[test]
    fn access_token_validity_matches_registration_contract() {
        assert_eq!(ACCESS_TOKEN_VALIDITY.as_secs(), 15 * 60);
    }
}
