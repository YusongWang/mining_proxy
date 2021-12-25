use std::sync::Arc;

use anyhow::Result;
use log::info;

use tokio::io::{split, BufReader};
use tokio::net::{TcpListener, TcpStream};
extern crate native_tls;
use native_tls::Identity;

use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::{broadcast, RwLock};

use crate::client::handle_stream;
use crate::jobs::JobQueue;
use crate::state::State;
use crate::util::config::Settings;

pub async fn accept_tcp_with_tls(
    state: Arc<RwLock<State>>,
    mine_jobs_queue: Arc<JobQueue>,
    develop_jobs_queue: Arc<JobQueue>,
    config: Settings,
    job_send: broadcast::Sender<String>,
    proxy_fee_sender: broadcast::Sender<(u64, String)>,
    develop_fee_sender: broadcast::Sender<(u64, String)>,
    state_send: UnboundedSender<(u64, String)>,
    dev_state_send: UnboundedSender<(u64, String)>,
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

        let config = config.clone();
        let acceptor = tls_acceptor.clone();
        let mine_jobs_queue = mine_jobs_queue.clone();
        let develop_jobs_queue = develop_jobs_queue.clone();
        let proxy_fee_sender = proxy_fee_sender.clone();
        let develop_fee_sender = develop_fee_sender.clone();

        tokio::spawn(async move {
            transfer_ssl(
                stream,
                acceptor,
                &config,
                mine_jobs_queue,
                develop_jobs_queue,
                proxy_fee_sender,
                develop_fee_sender,
            )
            .await
        });
    }
}

async fn transfer_ssl(
    tcp_stream: TcpStream,
    tls_acceptor: tokio_native_tls::TlsAcceptor,
    config: &Settings,
    mine_jobs_queue: Arc<JobQueue>,
    develop_jobs_queue: Arc<JobQueue>,
    proxy_fee_sender: broadcast::Sender<(u64, String)>,
    develop_fee_sender: broadcast::Sender<(u64, String)>,
) -> Result<()> {
    let client_stream = tls_acceptor.accept(tcp_stream).await?;
    let (worker_r, worker_w) = split(client_stream);
    let worker_r = BufReader::new(worker_r);

    info!("😄 tls_acceptor Success!");

    let (stream_type, pools) = match crate::util::get_pool_ip_and_type(&config) {
        Some(pool) => pool,
        None => {
            info!("未匹配到矿池 或 均不可链接。请修改后重试");
            return Ok(());
        }
    };

    let (outbound, _) = match crate::util::get_pool_stream(&pools) {
        Some((stream, addr)) => (stream, addr),
        None => {
            info!("所有TCP矿池均不可链接。请修改后重试");
            return Ok(());
        }
    };

    let stream = TcpStream::from_std(outbound)?;

    let (pool_r, pool_w) = split(stream);
    let pool_r = BufReader::new(pool_r);

    match handle_stream::handle_stream(
        worker_r,
        worker_w,
        pool_r,
        pool_w,
        &config,
        mine_jobs_queue,
        develop_jobs_queue,
        proxy_fee_sender,
        develop_fee_sender,
    )
    .await
    {
        Ok(_) => info!("正常退出"),
        Err(e) => info!("异常退出{:?}", e),
    }

    Ok(())
}
