use anyhow::Result;
use std::sync::Arc;
use tracing::info;

use tokio::{
    io::{split, BufReader},
    net::{TcpListener, TcpStream},
    sync::RwLockReadGuard,
};

use crate::{proxy::Proxy, state::Worker, util::config::Settings};

use super::*;
pub async fn accept_tcp(proxy: Arc<Proxy>) -> Result<()> {
    let config: Settings;
    {
        let rconfig = RwLockReadGuard::map(proxy.config.read().await, |s| s);
        config = rconfig.clone();
    }

    if config.tcp_port == 0 {
        return Ok(());
    }

    let address = format!("0.0.0.0:{}", config.tcp_port);
    let listener = match TcpListener::bind(address.clone()).await {
        Ok(listener) => listener,
        Err(_) => {
            tracing::info!("本地端口被占用 {}", address);
            std::process::exit(1);
        }
    };

    tracing::info!("本地TCP端口{} 启动成功!!!", &address);

    loop {
        let (stream, addr) = listener.accept().await?;

        let p = Arc::clone(&proxy);
        tokio::spawn(async move {
            // 矿工状态管理
            let mut worker: Worker = Worker::default();
            let worker_tx = p.worker_tx.clone();

            match transfer(p, &mut worker, stream).await {
                Ok(_) => {
                    if worker.is_online() {
                        worker.offline();
                        info!("IP: {} 安全下线", addr);
                        worker_tx.send(worker).unwrap();
                    } else {
                        info!("IP: {} 下线", addr);
                    }
                }
                Err(e) => {
                    if worker.is_online() {
                        worker.offline();
                        worker_tx.send(worker).unwrap();
                        info!("IP: {} 下线原因 {}", addr, e);
                    } else {
                        debug!("IP: {} 恶意链接断开: {}", addr, e);
                    }
                }
            }
        });
    }
}

async fn transfer(
    proxy: Arc<Proxy>, worker: &mut Worker, tcp_stream: TcpStream,
) -> Result<()> {
    let (worker_r, worker_w) = split(tcp_stream);
    let worker_r = BufReader::new(worker_r);

    let mut pool_address: Vec<String> = Vec::new();
    {
        let config = RwLockReadGuard::map(proxy.config.read().await, |s| s);
        pool_address = config.pool_address.to_vec();
    }

    let (stream_type, pools) =
        match crate::client::get_pool_ip_and_type_from_vec(&pool_address) {
            Ok(pool) => pool,
            Err(_) => {
                bail!("未匹配到矿池 或 均不可链接。请修改后重试");
            }
        };

    handle_tcp_random(
        worker,
        worker_r,
        worker_w,
        &pools,
        proxy,
        stream_type,
        false,
    )
    .await
    //handle_tcp_random(worker, worker_r, worker_w, &pools, proxy, false).await

    // if config.share == 0 {
    //     handle_tcp_pool(
    //         worker,
    //         worker_queue,
    //         worker_r,
    //         worker_w,
    //         &pools,
    //         &config,
    //         false,
    //     )
    //     .await
    // } else if config.share == 1 {
    //     // if config.share_alg == 99 {

    //     // } else {
    //     //     handle_tcp_pool_timer(
    //     //         worker,
    //     //         worker_queue,
    //     //         worker_r,
    //     //         worker_w,
    //     //         &pools,
    //     //         &config,
    //     //         false,
    //     //     )
    //     //     .await
    //     // }
    // } else {
    //     handle_tcp_pool_all(
    //         worker,
    //         worker_queue,
    //         worker_r,
    //         worker_w,
    //         &config,
    //         false,
    //     )
    //     .await
    // }
}
