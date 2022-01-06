use std::{net::SocketAddr, time::Duration};

use anyhow::{bail, Result};
use log::{debug, info};

use openssl::symm::{decrypt, encrypt, Cipher};

use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    select,
};

use crate::client::{self_write_socket_byte, write_to_socket_byte};

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
        let (stream, addr) = listener.accept().await?;
        info!("😄 Accepting EncryptData Tcp connection from {}", addr);
        let iv = iv.clone();
        let key = key.clone();

        tokio::spawn(async move { transfer(stream, server, key, iv).await });
    }

    Ok(())
}

async fn transfer(stream: TcpStream, addr: SocketAddr, key: Vec<u8>, iv: Vec<u8>) -> Result<()> {
    let (worker_r, mut worker_w) = tokio::io::split(stream);
    let worker_r = tokio::io::BufReader::new(worker_r);
    let mut worker_r = worker_r.lines();

    let std_stream = match std::net::TcpStream::connect_timeout(&addr, Duration::new(5, 0)) {
        Ok(stream) => stream,
        Err(_) => {
            info!("{} 远程地址不通！", addr);
            std::process::exit(1);
        }
    };

    std_stream.set_nonblocking(true).unwrap();
    let pool_stream = TcpStream::from_std(std_stream)?;
    let (pool_r, mut pool_w) = tokio::io::split(pool_stream);
    let pool_r = tokio::io::BufReader::new(pool_r);
    let mut pool_r = pool_r.split(crate::SPLIT);
    let mut client_timeout_sec = 1;

    let key = key.clone();
    let mut iv = iv.clone();

    loop {
        select! {
            res = tokio::time::timeout(std::time::Duration::new(client_timeout_sec,0), worker_r.next_line()) => {
                let start = std::time::Instant::now();
                let buffer = match res{
                    Ok(res) => {
                        match res {
                            Ok(buf) => match buf{
                                    Some(buf) => buf,
                                    None =>       {
                                    pool_w.shutdown().await;
                                    info!("矿机下线了");
                                    bail!("矿机下线了")},
                                },
                            _ => {
                                pool_w.shutdown().await;
                                info!("矿机下线了");
                                bail!("矿机下线了")
                            },
                        }
                    },
                    Err(e) => {pool_w.shutdown().await; bail!("读取超时了 矿机下线了: {}",e)},
                };

                if client_timeout_sec == 1 {
                    client_timeout_sec = 60;
                }

                #[cfg(debug_assertions)]
                debug!("------> :  矿机 -> 矿池  {:?}", buffer);
                let buffer: Vec<_> = buffer.split("\n").collect();
                for buf in buffer {
                    if buf.is_empty() {
                        continue;
                    }
                    // let key = Vec::from_hex(key).unwrap();
                    // let mut iv = Vec::from_hex(iv).unwrap();
                    // 加密
                    //let key = AesKey::new_encrypt(&key).unwrap();
                    //let plain_text = buf.to_string().as_bytes();
                    //let mut output = buf.as_bytes().to_vec().clone();

                    let cipher = Cipher::aes_256_cbc();
                    //let data = b"Some Crypto String";
                    let ciphertext = encrypt(
                        cipher,
                        &key,
                        Some(&iv),
                        buf.as_bytes()).unwrap();

                    info!("{:?}",ciphertext);

                    let base64 = base64::encode(&ciphertext[..]);
                    // let write_len = w.write(&base64.as_bytes()).await?;

                    match self_write_socket_byte(&mut pool_w,base64.as_bytes().to_vec(),&"加密".to_string()).await{
                        Ok(_) => {},
                        Err(e) => {info!("{}",e);bail!("矿机下线了 {}",e)}
                    }
                }
            },
            res = pool_r.next_segment() => {
                let start = std::time::Instant::now();
                let buffer = match res{
                    Ok(res) => {
                        match res {
                            Some(buf) => buf,
                            None => {
                                worker_w.shutdown().await;
                                info!("矿机下线了");
                                bail!("矿机下线了")
                            }
                        }
                    },
                    Err(e) => {info!("矿机下线了");bail!("矿机下线了: {}",e)},
                };


                #[cfg(debug_assertions)]
                debug!("<------ :  矿池 -> 矿机  {:?}", buffer);

                let buffer = buffer[0..buffer.len()].split(|c| *c == crate::SPLIT);
                for buf in buffer {
                    if buf.is_empty() {
                        continue;
                    }


                    let buf = match base64::decode(&buf[..]) {
                        Ok(buf) => buf,
                        Err(e) => {
                            log::error!("{}",e);
                            pool_w.shutdown().await;
                            return Ok(());
                        },
                    };


                    let cipher = Cipher::aes_256_cbc();
                    // 解密
                    let buffer = match decrypt(
                        cipher,
                        &key,
                        Some(&iv),
                        &buf[..]) {
                            Ok(s) => s,
                            Err(e) => {
                                info!("解密失败 {}",e);
                                pool_w.shutdown().await;
                                return Ok(());
                            },
                        };

                    match write_to_socket_byte(&mut worker_w,buffer,&"解密".to_string()).await{
                        Ok(_) => {},
                        Err(e) => {info!("{}",e);bail!("矿机下线了 {}",e)}
                    }
                }
            }
        }
    }
}
