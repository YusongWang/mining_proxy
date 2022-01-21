use anyhow::Result;
use log::info;

use tokio::io::{split, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::UnboundedSender;

use crate::state::{State, Worker};
use crate::util::config::Settings;

use super::*;
pub async fn accept_tcp(
    worker_queue: UnboundedSender<Worker>,
    config: Settings,
    state: State,
) -> Result<()> {
    if config.tcp_port == 0 {
        return Ok(());
    }

    let address = format!("0.0.0.0:{}", config.tcp_port);
    let listener = match TcpListener::bind(address.clone()).await {
        Ok(listener) => listener,
        Err(_) => {
            println!("本地端口被占用 {}", address);
            std::process::exit(1);
        }
    };

    println!("本地TCP端口{} 启动成功!!!", &address);

    loop {
        let (stream, addr) = listener.accept().await?;

        let config = config.clone();
        let workers = worker_queue.clone();
        let state = state.clone();
        state
            .online
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        tokio::spawn(async move {
            // 矿工状态管理
            let mut worker: Worker = Worker::default();
            match transfer(&mut worker, workers.clone(), stream, &config, state.clone()).await {
                Ok(_) => {
                    state
                        .online
                        .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
                    if worker.is_online() {
                        worker.offline();
                        workers.send(worker);
                    } else {
                        info!("IP: {} 断开", addr);
                    }
                }
                Err(e) => {
                    if worker.is_online() {
                        worker.offline();
                        workers.send(worker);
                        info!("IP: {} 断开原因 {}", addr, e);
                    } else {
                        info!("IP: {} 恶意链接断开: {}", addr, e);
                    }

                    state
                        .online
                        .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
                }
            }
        });
    }
}

async fn transfer(
    worker: &mut Worker,
    worker_queue: UnboundedSender<Worker>,
    tcp_stream: TcpStream,
    config: &Settings,
    state: State,
) -> Result<()> {
    let (worker_r, worker_w) = split(tcp_stream);
    let worker_r = BufReader::new(worker_r);
    let (stream_type, pools) = match crate::client::get_pool_ip_and_type(&config) {
        Some(pool) => pool,
        None => {
            bail!("未匹配到矿池 或 均不可链接。请修改后重试");
        }
    };

    if config.share == 0 {
        handle_tcp_pool(
            worker,
            worker_queue,
            worker_r,
            worker_w,
            &pools,
            &config,
            state,
            false,
        )
        .await
    } else if config.share == 1 {
        handle_tcp_pool_timer(
            worker,
            worker_queue,
            worker_r,
            worker_w,
            &pools,
            &config,
            state,
            false,
        )
        .await
    } else {
        handle_tcp_pool_all(
            worker,
            worker_queue,
            worker_r,
            worker_w,
            &config,
            state,
            false,
        )
        .await
    }
}
