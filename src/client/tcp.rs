use std::sync::Arc;

use anyhow::Result;

use log::info;

use tokio::io::split;
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
    send: Sender<String>,
    d_send: Sender<String>,
    state_send: UnboundedSender<String>,
    
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
        let s = send.clone();
        let d = d_send.clone();
        let state = state.clone();

        let jobs_recv = job_send.subscribe();
        let state_send = state_send.clone();

        tokio::spawn(async move {
            let transfer = transfer(state, jobs_recv, stream, c, s, d,state_send).map(|r| {
                if let Err(e) = r {
                    info!("❎ 线程退出 : error={}", e);
                }
            });
            tokio::spawn(transfer);
        });
    }
}

async fn transfer(
    state: Arc<RwLock<State>>,
    jobs_recv: broadcast::Receiver<String>,
    inbound: TcpStream,
    config: Settings,
    proxy_fee_send: Sender<String>,
    fee: Sender<String>,
    state_send: UnboundedSender<String>,
) -> Result<()> {
    let outbound = TcpStream::connect(&config.pool_tcp_address.to_string()).await?;

    let (r_client, w_client) = split(inbound);
    let (r_server, w_server) = split(outbound);
    use tokio::sync::mpsc;
    let (tx, mut rx) = mpsc::unbounded_channel::<ServerId1>();

    tokio::try_join!(
        client_to_server(
            state.clone(),
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
            config.clone(),
            jobs_recv,
            r_server,
            w_client,
            proxy_fee_send.clone(),
            state_send.clone(),
            rx
        )
    )?;

    Ok(())
}
