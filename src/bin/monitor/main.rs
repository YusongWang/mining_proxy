#![allow(dead_code)]
mod version {
    include!(concat!(env!("OUT_DIR"), "/version.rs"));
}

use std::net::ToSocketAddrs;

use anyhow::Result;
use clap::{crate_name, crate_version};
use hex::FromHex;
use log::info;
use openssl::aes::AesKey;

#[tokio::main]
async fn main() -> Result<()> {
    let matches = mining_proxy::util::get_encrypt_command_matches().await?;
    mining_proxy::util::logger::init("monitor", "./logs/".into(), 0)?;

    info!(
        "✅ {}, 版本: {} commit: {} {}",
        crate_name!(),
        crate_version!(),
        version::commit_date(),
        version::short_sha()
    );

    let key = matches.value_of("key").unwrap_or(
        "523B607044E6BF7E46AF75233FDC1278B7AA0FC42D085DEA64AE484AD7FB3664",
    );
    // let iv = matches
    //     .value_of("iv")
    //     .unwrap_or("275E2015B9E5CA4DDB87B90EBC897F8C");

    let key = Vec::from_hex(key).unwrap();
    let _ = AesKey::new_encrypt(&key).unwrap_or_else(|e| {
        info!("请填写正确的 key {:?}", e);
        std::process::exit(1);
    });

    let port = matches.value_of("port").unwrap_or_else(|| {
        info!("请正确填写本地监听端口 例如: -p 8888");
        std::process::exit(1);
    });

    let server = matches.value_of("server").unwrap_or_else(|| {
        info!("请正确填写服务器地址 例如: -s 127.0.0.0:8888");
        std::process::exit(1);
    });

    let addr = match server.to_socket_addrs().unwrap().next() {
        Some(address) => address,
        None => {
            info!("请正确填写服务器地址 例如: -s 127.0.0.0:8888");
            std::process::exit(1);
        }
    };

    let port: i32 = port.parse().unwrap_or_else(|_| {
        info!("请正确填写本地监听端口 例如: -p 8888");
        std::process::exit(1);
    });

    let res = tokio::try_join!(
        mining_proxy::client::monitor::accept_monitor_tcp(port, addr)
    );

    if let Err(err) = res {
        log::warn!("加密服务断开: {}", err);
    }

    Ok(())
}
