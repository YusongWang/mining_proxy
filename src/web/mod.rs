use std::env;

use diesel::{
    r2d2::{self, ConnectionManager, PooledConnection},
    SqliteConnection,
};

use self::{models::Pool, schema::pools::{self, pid}};
use diesel::prelude::*;


pub mod actions;
pub mod greet;
pub mod models;
pub mod schema;
pub mod webmain;

pub type DbPool = r2d2::Pool<ConnectionManager<SqliteConnection>>;

pub fn run(pool_data: Pool,conn: PooledConnection<ConnectionManager<SqliteConnection>>) {
    match env::current_exe() {
        Ok(exe_path) => {
            let p = std::process::Command::new(exe_path)
                .env("PROXY_NAME", pool_data.name)
                .env("PROXY_LOG_LEVEL", 1.to_string())
                .env("PROXY_LOG_PATH", "./logs/")
                .env("PROXY_TCP_PORT", pool_data.tcp_port.to_string())
                .env("PROXY_SSL_PORT", pool_data.ssl_port.to_string())
                .env("PROXY_POOL_TCP_ADDRESS", pool_data.pool_tcp_address)
                .env("PROXY_POOL_SSL_ADDRESS", pool_data.pool_ssl_address)
                .env("PROXY_SHARE_TCP_ADDRESS", pool_data.share_tcp_address)
                .env("PROXY_SHARE_RATE", pool_data.share_rate.to_string())
                .env("PROXY_SHARE_NAME", pool_data.share_name.to_string())
                .env("PROXY_SHARE", pool_data.share.to_string())
                .env("PROXY_P12_PATH", pool_data.p12_path.to_string())
                .env("PROXY_P12_PASS", pool_data.p12_pass.to_string())
                .env("PROXY_KEY", pool_data.key.to_string())
                .env("PROXY_IV", pool_data.iv.to_string())
                .spawn()
                .unwrap();
            let id = p.id();
            use crate::web::pools::dsl::pools;
            let mut a = vec![];
            a.push(p);
            
            diesel::update(pools.find(pool_data.id))
                .set(pid.eq(id as i32))
                .execute(&conn)
                .expect(&format!("Unable to find post {}", id));
        }
        Err(e) => println!("failed to get current exe path: {}", e),
    };
}
