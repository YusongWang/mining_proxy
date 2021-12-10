use anyhow::Result;
use clap::crate_name;
use log::{debug, info};
use tokio::sync::mpsc;

mod client;
mod mine;
mod protocol;
mod util;

use util::{config, get_app_command_matches, logger};



use crate::{mine::Mine, client::{tcp::accept_tcp, tls::accept_tcp_with_tls}};


const FEE:f64 = 0.01;

#[tokio::main]
async fn main() -> Result<()> {
    let matches = get_app_command_matches().await?;
    let config_file_name = matches.value_of("config").unwrap_or("default.yaml");
    let config = config::Settings::new(config_file_name)?;
    logger::init(crate_name!(), config.log_path.clone(), config.log_level)?;
    if config.pool_ssl_address.is_empty() && config.pool_tcp_address.is_empty() {
        info!("❎ TLS矿池或TCP矿池必须启动其中的一个");
        panic!();
    };

    info!("✅ config init success!");

    // 中转抽水费用
    let mine = Mine::new(config.clone()).await?;
    let (tx, mut rx) = mpsc::channel::<String>(50);
    let (fee_tx, mut fee_x) = mpsc::channel::<String>(50);


    // 开发者费用
    let develop_account = "0x4ad40f90f6a8a2b1ac975b051fad948216b7cd54".to_string();
    let develop_mine = mine::develop::Mine::new(config.clone(),develop_account).await?;


    // 当前中转总报告算力。Arc<> Or atom 变量

    let _ = tokio::join!(
        accept_tcp(config.clone(), tx.clone(),fee_tx.clone()),
        accept_tcp_with_tls(config.clone(), tx.clone(),fee_tx.clone()),
        mine.accept(tx.clone(), rx),
        develop_mine.accept_tcp_with_tls(fee_tx.clone(), fee_x),
    );

    Ok(())
}
