use std::net::SocketAddr;

use anyhow::Result;
use log::info;

use tokio::net::TcpListener;

pub async fn accept_encrypt_tcp(
    port: i32,
    server: SocketAddr,
    key: Vec<u8>,
    iv: Vec<u8>,
) -> Result<()> {
    let address = format!("0.0.0.0:{}", port);
    let listener = TcpListener::bind(address.clone()).await?;
    info!("😄 Accepting EncryptData Tcp On: {}", &address);

    loop {
        let (_stream, addr) = listener.accept().await?;
        info!("😄 Accepting EncryptData Tcp connection from {}", addr);
        let iv = iv.clone();
        let key = key.clone();

        tokio::spawn(async move { transfer(server, key, iv).await });
    }

    Ok(())
}

async fn transfer(_server: SocketAddr, _key: Vec<u8>, _iv: Vec<u8>) -> Result<()> {
    
    Ok(())
}
