use jsonwebtoken::{
    decode, encode, DecodingKey, EncodingKey, Header, Validation,
};

use serde::{Deserialize, Serialize};

use crate::JWT_SECRET;

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
