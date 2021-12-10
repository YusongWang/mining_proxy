use anyhow::Result;
use log::{debug, error, info};

use tokio::io::{split, AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use futures::FutureExt;

use crate::client::{client_to_server, server_to_client};
use crate::protocol::rpc::eth::{Client, ClientGetWork, Server, ServerId1};
use crate::util::config::Settings;

pub async fn accept_tcp(config: Settings) -> Result<()> {
    let address = format!("0.0.0.0:{}", config.tcp_port);
    let listener = TcpListener::bind(address.clone()).await?;
    info!("✅ Accepting Tcp On: {}", &address);

    loop {
        // Asynchronously wait for an inbound TcpStream.
        let (stream, addr) = listener.accept().await?;
        info!("✅ accept connection from {}", addr);
        let c = config.clone();

        tokio::spawn(async move {
            let transfer = transfer(stream, c).map(|r| {
                if let Err(e) = r {
                    error!("❎ 线程退出 : error={}", e);
                }
            });
            tokio::spawn(transfer);
        });
    }
}

pub async fn transfer(mut inbound: TcpStream, config: Settings) -> Result<()> {
    let mut outbound = TcpStream::connect(&config.pool_tcp_address.to_string()).await?;

    let (mut r_client, mut w_client) = split(inbound);
    let (mut r_server, mut w_server) = split(outbound);

    tokio::try_join!(
        client_to_server(r_client, w_server),
        server_to_client(r_server, w_client)
    )?;

    Ok(())
}
