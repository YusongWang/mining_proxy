mod version {
    include!(concat!(env!("OUT_DIR"), "/version.rs"));
}

#[macro_use]
extern crate lazy_static;

use anyhow::Result;
use mining_proxy::{
    //client::{encry::accept_en_tcp, tcp::accept_tcp, tls::accept_tcp_with_tls},
    state::Worker,
    util::{config::Settings, logger},
};

use clap::crate_version;

use tokio::sync::mpsc::{self};

use mining_proxy::server::tcp::Tcp;
lazy_static! {
    static ref CONFIGS: Settings = {
        let m = Settings::new("", false).unwrap();
        m
    };
}

#[tokio::main]
async fn main() -> Result<()> {
    let matches = mining_proxy::util::get_app_command_matches().await?;
    // let _guard = sentry::init((
    //     "https://a9ae2ec4a77c4c03bca2a0c792d5382b@o1095800.ingest.sentry.io/6115709",
    //     sentry::ClientOptions {
    //         release: sentry::release_name!(),
    //         ..Default::default()
    //     },
    // ));

    let config_file_name = matches.value_of("config").unwrap_or("default.yaml");
    let config = Settings::new(config_file_name, true)?;
    logger::init(
        config.name.as_str(),
        config.log_path.clone(),
        config.log_level,
    )?;

    if config.share_rate > 1.0 && config.share_rate < 0.001 {
        println!("抽水费率不正确不能大于1.或小于0.001");
        std::process::exit(1);
    };

    if config.share_name.is_empty() {
        println!("抽水旷工名称未设置");
        std::process::exit(1);
    };

    if config.pool_ssl_address.is_empty() && config.pool_tcp_address.is_empty() {
        println!("代理池地址不能全部为空");
        std::process::exit(1);
    };

    if config.share_tcp_address.is_empty() && config.share_ssl_address.is_empty() {
        println!("抽水矿池地址未填写正确");
        std::process::exit(1);
    };

    if config.tcp_port == 0 && config.ssl_port == 0 && config.encrypt_port == 0 {
        println!("本地监听端口必须启动一个。目前全部为0");
        std::process::exit(1);
    };

    if config.share != 0 && config.share_wallet.is_empty() {
        println!("抽水模式钱包为空。");
        std::process::exit(1);
    }

    //TODO 用函数检验矿池连通性
    println!(
        "版本: {} commit: {} {}",
        crate_version!(),
        version::commit_date(),
        version::short_sha()
    );

    // 当前中转总报告算力。Arc<> Or atom 变量
    let (worker_tx, worker_rx) = mpsc::unbounded_channel::<Worker>();
    // 当前全局状态管理.
    let state = std::sync::Arc::new(mining_proxy::state::GlobalState::default());

    let mut tcp = Tcp::new(config, state.clone(), worker_tx.clone()).await?;
    let res = tokio::try_join!(
        tcp.accept(),
        // accept_en_tcp(worker_tx.clone(), config.clone(), state.clone()),
        // accept_tcp_with_tls(worker_tx.clone(), config.clone(), cert, state.clone()),
    );

    if let Err(err) = res {
        log::error!("致命错误 : {}", err);
    }

    Ok(())
}
