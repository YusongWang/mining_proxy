use std::sync::Arc;

use anyhow::Result;
use log::info;

use tokio::io::split;
use tokio::net::{TcpListener, TcpStream};

use futures::FutureExt;
use tokio::sync::{mpsc::Sender, RwLock};

use crate::client::{client_to_server, server_to_client};
use crate::protocol::rpc::eth::ServerId1;
use crate::state::State;
use crate::util::config::Settings;

pub async fn accept_tcp(
    state: Arc<RwLock<State>>,
    config: Settings,
    send: Sender<String>,
    d_send: Sender<String>,
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
        tokio::spawn(async move {
            let transfer = transfer(state, stream, c, s, d).map(|r| {
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
    inbound: TcpStream,
    config: Settings,
    send: Sender<String>,
    fee: Sender<String>,
) -> Result<()> {
    let outbound = TcpStream::connect(&config.pool_tcp_address.to_string()).await?;

    let (r_client, w_client) = split(inbound);
    let (r_server, w_server) = split(outbound);
    use tokio::sync::mpsc;
    let (tx, rx) = mpsc::channel::<ServerId1>(100);

    tokio::try_join!(
        client_to_server(
            state.clone(),
            config.clone(),
            r_client,
            w_server,
            send.clone(),
            fee.clone(),
            tx.clone()
        ),
        server_to_client(state.clone(), r_server, w_client, send.clone(), rx)
    )?;

    Ok(())
}
