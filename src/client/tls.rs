use anyhow::Result;
use log::info;

use tokio::io::{split, BufReader};
use tokio::net::{TcpListener, TcpStream};
extern crate native_tls;
use native_tls::Identity;
use tokio::sync::mpsc::UnboundedSender;

use super::*;

use crate::state::{State, Worker};
use crate::util::config::Settings;

pub async fn accept_tcp_with_tls(
    worker_queue: UnboundedSender<Worker>,
    config: Settings,
    cert: Identity,
    state: State,
) -> Result<()> {
    if config.ssl_port == 0 {
        return Ok(());
    }

    let address = format!("0.0.0.0:{}", config.ssl_port);
    let listener = match TcpListener::bind(address.clone()).await {
        Ok(listener) => listener,
        Err(_) => {
            println!("本地端口被占用 {}", address);
            std::process::exit(1);
        }
    };

    println!("本地SSL端口{} 启动成功!!!", &address);

    let tls_acceptor =
        tokio_native_tls::TlsAcceptor::from(native_tls::TlsAcceptor::builder(cert).build()?);
    loop {
        // Asynchronously wait for an inbound TcpStream.
        let (stream, addr) = listener.accept().await?;
        //info!("😄 accept connection from {}", addr);
        let workers = worker_queue.clone();

        let config = config.clone();
        let acceptor = tls_acceptor.clone();
        let state = state.clone();

        state
            .online
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        tokio::spawn(async move {
            // 矿工状态管理
            let mut worker: Worker = Worker::default();
            match transfer_ssl(
                &mut worker,
                workers.clone(),
                stream,
                acceptor,
                &config,
                state.clone(),
            )
            .await
            {
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

async fn transfer_ssl(
    worker: &mut Worker,
    worker_queue: UnboundedSender<Worker>,
    tcp_stream: TcpStream,
    tls_acceptor: tokio_native_tls::TlsAcceptor,
    config: &Settings,
    state: State,
) -> Result<()> {
    let client_stream = tls_acceptor.accept(tcp_stream).await?;
    let (worker_r, worker_w) = split(client_stream);
    let worker_r = BufReader::new(worker_r);

    let (stream_type, pools) = match crate::client::get_pool_ip_and_type(&config) {
        Some(pool) => pool,
        None => {
            bail!("未匹配到矿池 或 均不可链接。请修改后重试");
        }
    };

    //if stream_type == crate::client::TCP {
        if config.share != 0 {
            if config.share_alg == 99 {
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
            } else {
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
            }
        } else {
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
        }
//     } else if stream_type == crate::client::SSL {
//         handle_tls_pool(
//             worker,
//             worker_queue,
//             worker_r,
//             worker_w,
//             &pools,
//             &config,
//             state,
//             false,
//         )
//         .await
//     } else {
//         log::error!("致命错误：未找到支持的矿池BUG 请上报");
//         bail!("致命错误：未找到支持的矿池BUG 请上报");
//     }
}
