use diesel::{Insertable, Queryable};
use serde::{Deserialize, Serialize};

use crate::web::schema::pools;
#[derive(Debug, Clone, Serialize, Deserialize, Queryable, Insertable)]
pub struct Pool {
    pub id: i32,
    pub name: String,
    pub tcp_port: i32,
    pub encrypt_port: i32,
    pub ssl_port: i32,
    pub pool_tcp_address: String,
    pub pool_ssl_address: String,
    pub share_tcp_address: String,
    pub share_rate: f32,
    pub share_wallet: String,
    pub share_name: String,
    pub share: i32,
    pub share_alg: i32,
    pub p12_path: String,
    pub p12_pass: String,
    pub key: String,
    pub iv: String,
    pub coin: String,
    pub is_online: i32,
    pub is_open: i32,
    pub pid: i32,
}

