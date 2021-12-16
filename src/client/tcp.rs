use std::io::{Read, Write};
use std::net::ToSocketAddrs;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{bail, Result};

use bytes::{buf, BufMut};
use log::info;

use tokio::io::{split, AsyncReadExt};
use tokio::net::{TcpListener, TcpStream};

use futures::FutureExt;
use tokio::sync::broadcast;

use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::{mpsc::Sender, RwLock};

use crate::client::{client_to_server, server_to_client};
use crate::protocol::rpc::eth::ServerId1;
use crate::state::State;
use crate::util::config::Settings;

pub async fn accept_tcp(
    state: Arc<RwLock<State>>,
    config: Settings,
    job_send: broadcast::Sender<String>,
    proxy_fee_sender: UnboundedSender<String>,
    develop_fee_sender: UnboundedSender<String>,
    state_send: UnboundedSender<String>,
    dev_state_send: UnboundedSender<String>,
) -> Result<()> {
    if config.pool_tcp_address.is_empty() {
        return Ok(());
    }

    let address = format!("0.0.0.0:{}", config.tcp_port);
    let listener = TcpListener::bind(address.clone()).await?;
    info!("😄 Accepting Tcp On: {}", &address);

    loop {
        // Asynchronously wait for an inbound TcpStream.
        let (stream, addr) = listener.accept().await?;
        info!("😄 accept connection from {}", addr);
        let c = config.clone();
        let proxy_fee_sender = proxy_fee_sender.clone();
        let d = develop_fee_sender.clone();
        let state = state.clone();

        let jobs_recv = job_send.subscribe();
        let state_send = state_send.clone();
        let dev_state_send = dev_state_send.clone();
        tokio::spawn(async move {
            let transfer = transfer(
                state,
                jobs_recv,
                stream,
                c,
                proxy_fee_sender,
                d,
                state_send,
                dev_state_send,
            )
            .map(|r| {
                if let Err(e) = r {
                    info!("❎ 线程退出 : {}", e);
                }
            });

            info!("初始化完成");
            tokio::spawn(transfer);
        });
    }
}

async fn transfer(
    state: Arc<RwLock<State>>,
    jobs_recv: broadcast::Receiver<String>,
    inbound: TcpStream,
    config: Settings,
    proxy_fee_send: UnboundedSender<String>,
    fee: UnboundedSender<String>,
    state_send: UnboundedSender<String>,
    dev_state_send: UnboundedSender<String>,
) -> Result<()> {
    let (stream,addr) = match crate::util::get_pool_stream(&config.pool_ssl_address) {
        Some((stream,addr)) => (stream,addr),
        None => {
            info!("所有SSL矿池均不可链接。请修改后重试");
            std::process::exit(100);
        }
    };

    let outbound = TcpStream::from_std(stream)?;
    let (r_client, w_client) = split(inbound);
    let (r_server, w_server) = split(outbound);

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<ServerId1>();
    let worker = Arc::new(RwLock::new(String::new()));

    info!("start client and server");
    tokio::try_join!(
        client_to_server(
            state.clone(),
            worker.clone(),
            config.clone(),
            r_client,
            w_server,
            proxy_fee_send.clone(),
            //state_send.clone(),
            fee.clone(),
            tx.clone()
        ),
        server_to_client(
            state.clone(),
            worker,
            config.clone(),
            jobs_recv,
            r_server,
            w_client,
            proxy_fee_send.clone(),
            state_send.clone(),
            dev_state_send.clone(),
            rx
        )
    )?;

    Ok(())
}
