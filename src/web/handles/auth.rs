use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};

lazy_static! {
    static ref JWT_SECRET: String = std::env::var("JWT_SECRET").expect("JWT_SECRET must be set");
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Claims {
    pub username: String,
}

pub fn generate_jwt(claims: &Claims) -> anyhow::Result<String> {
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(JWT_SECRET.as_bytes()),
    )
    .map_err(|e| anyhow::anyhow!(e))
}