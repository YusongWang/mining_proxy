use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct CreateRequest {
    pub name: String,
    pub tcp_port: u32,
    pub ssl_port: u32,
    pub encrypt_port: u32,
    pub share: u32,
    pub pool_address: String,
    pub share_address: String,
    pub share_rate: f32,
    pub share_wallet: String,
    pub key: String,
    pub iv: String,
}

#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct TokenDataResponse {
    pub token: String,
}

#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct InfoResponse {
    pub roles: Vec<String>,
    pub introduction: String,
    pub avatar: String,
    pub name: String,
}

#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct Response<T> {
    pub code: i32,
    pub message: String,
    pub data: T,
}

#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct LoginRequest {
    pub password: String,
}

#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct LoginResponse {
    pub code: i32,
    pub data: TokenDataResponse,
}
