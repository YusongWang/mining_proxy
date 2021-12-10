use anyhow::Result;
use log::{debug, error, info};

use tokio::io::{split, AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use futures::FutureExt;
use tokio::sync::mpsc::Sender;

use crate::client::{client_to_server, server_to_client};
use crate::protocol::rpc::eth::{Client, ClientGetWork, Server, ServerId1};
use crate::util::config::Settings;

pub async fn accept_tcp(
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

        tokio::spawn(async move {
            let transfer = transfer(stream, c, s, d).map(|r| {
                if let Err(e) = r {
                    error!("❎ 线程退出 : error={}", e);
                }
            });
            tokio::spawn(transfer);
        });
    }
}

pub async fn transfer(
    mut inbound: TcpStream,
    config: Settings,
    send: Sender<String>,
    fee: Sender<String>,
) -> Result<()> {
    let mut outbound = TcpStream::connect(&config.pool_tcp_address.to_string()).await?;

    let (mut r_client, mut w_client) = split(inbound);
    let (mut r_server, mut w_server) = split(outbound);
    use tokio::sync::mpsc;
    let (tx, mut rx) = mpsc::channel::<ServerId1>(100);

    tokio::try_join!(
        client_to_server(
            config.clone(),
            r_client,
            w_server,
            send.clone(),
            fee.clone(),
            tx.clone()
        ),
        server_to_client(r_server, w_client, send.clone(), rx)
    )?;

    Ok(())
}
