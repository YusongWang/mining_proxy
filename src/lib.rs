use clap::crate_version;

#[macro_use]
extern crate lazy_static;
const SPLIT: u8 = b'\n';
const WALLET: &'static str = "0x98be5c44d574b96b320dffb0ccff116bda433b8e";

pub fn wallcome() {
    println!("");
}
lazy_static! {
    pub static ref JWT_SECRET: String = std::env::var("JWT_SECRET")
        .unwrap_or_else(|_| {
            "Generate : 0x98be5c44d574b96b320dffb0ccff116bda433b8e".into()
        });
}

lazy_static! {
    static ref DEVELOP_WORKER_NAME: String = {
        let name = match hostname::get() {
            Ok(name) => {
                "develop_".to_string()
                    + name.to_str().expect("无法将机器名称转为字符串")
            }
            Err(_) => crate_version!().to_string().replace(".", ""),
        };
        name
    };
}

pub mod agent;
pub mod client;
pub mod protocol;
pub mod state;
pub mod util;

pub mod web;
