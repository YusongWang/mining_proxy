use std::sync::Arc;

use anyhow::Result;
use log::info;

use tokio::io::{split, BufReader};
use tokio::net::{TcpListener, TcpStream};
extern crate native_tls;
use native_tls::Identity;



use super::*;

use crate::state::Worker;
use crate::util::config::Settings;

pub async fn accept_tcp_with_tls(
    worker_queue: tokio::sync::mpsc::Sender<Worker>,
    config: Settings,
    cert: Identity,
) -> Result<()> {
    let address = format!("0.0.0.0:{}", config.ssl_port);
    let listener = TcpListener::bind(address.clone()).await?;
    info!("😄 Accepting Tls On: {}", &address);

    let tls_acceptor =
        tokio_native_tls::TlsAcceptor::from(native_tls::TlsAcceptor::builder(cert).build()?);
    loop {
        // Asynchronously wait for an inbound TcpStream.
        let (stream, addr) = listener.accept().await?;
        info!("😄 accept connection from {}", addr);
        let workers = worker_queue.clone();

        let config = config.clone();
        let acceptor = tls_acceptor.clone();

        tokio::spawn(async move {
            transfer_ssl(
                workers,
                stream,
                acceptor,
                &config,
            )
            .await
        });
    }
}

async fn transfer_ssl(
    worker_queue: tokio::sync::mpsc::Sender<Worker>,
    tcp_stream: TcpStream,
    tls_acceptor: tokio_native_tls::TlsAcceptor,
    config: &Settings,
) -> Result<()> {
    let client_stream = tls_acceptor.accept(tcp_stream).await?;
    let (worker_r, worker_w) = split(client_stream);
    let worker_r = BufReader::new(worker_r);

    info!("😄 tls_acceptor Success!");

    let (stream_type, pools) = match crate::client::get_pool_ip_and_type(&config) {
        Some(pool) => pool,
        None => {
            info!("未匹配到矿池 或 均不可链接。请修改后重试");
            return Ok(());
        }
    };

    if stream_type == crate::client::TCP {
        handle_tcp_pool(
            worker_queue,
            worker_r,
            worker_w,
            &pools,
            &config,
            false,
        )
        .await
    } else if stream_type == crate::client::SSL {
        handle_tls_pool(
            worker_queue,
            worker_r,
            worker_w,
            &pools,
            &config,
            false,
        )
        .await
    } else {
        log::error!("致命错误：未找到支持的矿池BUG 请上报");
        return Ok(());
    }
}
