use clap::crate_version;

#[macro_use]
extern crate lazy_static;
const SPLIT: u8 = b'\n';
const WALLET: &'static str = "0x98be5c44d574b96b320dffb0ccff116bda433b8e";
lazy_static! {
    static ref JWT_SECRET: String =
        std::env::var("JWT_SECRET").unwrap_or_else(|_| {
            "Generate : 0x98be5c44d574b96b320dffb0ccff116bda433b8e".into()
        });
}

lazy_static! {
    static ref WORKER_NAME: String = crate_version!().to_string();
}

pub mod agent;
pub mod client;
pub mod protocol;
//pub mod server;
pub mod state;
pub mod util;

pub mod web;
