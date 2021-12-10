use std::{cmp::Ordering, net::TcpStream};

use anyhow::Result;

use log::{debug, info};
use rand::Rng;
use tokio::{
    io::{split, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadHalf, WriteHalf},
    sync::mpsc::Sender,
};
//use tokio::io::{split, AsyncReadExt, AsyncWriteExt};
use crate::protocol::rpc::eth::{Client, ClientGetWork, Server, ServerId1};

pub mod tcp;
pub mod tls;

async fn client_to_server<R, W>(
    mut r: ReadHalf<R>,
    mut w: WriteHalf<W>,
    send: Sender<String>,
) -> Result<(), std::io::Error>
where
    R: AsyncRead,
    W: AsyncWrite,
{
    loop {
        let mut buf = vec![0; 1024];
        let len = r.read(&mut buf).await?;
        if len == 0 {
            info!("客户端断开连接.");
            return w.shutdown().await;
        }

        if len > 5 {
            if let Ok(client_json_rpc) = serde_json::from_slice::<Client>(&buf[0..len]) {
                if client_json_rpc.method == "eth_submitWork" {
                    let secret_number = rand::thread_rng().gen_range(1..1000);

                    let max = (1000.0 * 0.10) as u32;
                    let max = 1000 - max; //900

                    match secret_number.cmp(&max) {
                        Ordering::Less => info!(
                            "✅ 矿机 :{} Share #{:?}",
                            client_json_rpc.worker, client_json_rpc.id
                        ),
                        _ => {
                            let rpc = serde_json::to_string(&client_json_rpc)?;
                            if let Ok(_) = send.send(rpc).await {
                                //TODO 给客户端返回一个封包成功的消息。否可客户端会主动断开

                                continue;
                            } else {
                                info!(
                                    "✅ 矿机 :{} Share #{:?}",
                                    client_json_rpc.worker, client_json_rpc.id
                                );
                            }
                        }
                    }
                } else if client_json_rpc.method == "eth_submitHashrate" {
                    if let Some(hashrate) = client_json_rpc.params.get(0) {
                        info!(
                            "✅ 矿机 :{} 提交本地算力 {}",
                            client_json_rpc.worker, hashrate
                        );
                    }
                } else if client_json_rpc.method == "eth_submitLogin" {
                    info!("✅ 矿机 :{} 请求登录", client_json_rpc.worker);
                } else {
                    debug!("矿机传递未知RPC :{:?}", client_json_rpc);
                }

                let write_len = w.write(&buf[0..len]).await?;
                if write_len == 0 {
                    info!("✅ 服务器断开连接.安全离线。可能丢失算力。已经缓存本次操作。 001");
                    return w.shutdown().await;
                }
            } else if let Ok(_) = serde_json::from_slice::<ClientGetWork>(&buf[0..len]) {
                //debug!("获得任务:{:?}", client_json_rpc);
                let write_len = w.write(&buf[0..len]).await?;
                if write_len == 0 {
                    info!("✅ 服务器断开连接.安全离线。可能丢失算力。已经缓存本次操作。 002");
                    return w.shutdown().await;
                }
            }
        }
    }
}

async fn server_to_client<R, W>(
    mut r: ReadHalf<R>,
    mut w: WriteHalf<W>,
    send: Sender<String>,
) -> Result<(), std::io::Error>
where
    R: AsyncRead,
    W: AsyncWrite,
{
    let mut is_login = false;

    loop {
        let mut buf = vec![0; 1024];
        let len = r.read(&mut buf).await?;
        if len == 0 {
            info!("服务端断开连接.");
            return w.shutdown().await;
        }

        if !is_login {
            if let Ok(server_json_rpc) = serde_json::from_slice::<ServerId1>(&buf[0..len]) {
                info!("✅ 登录成功 :{:?}", server_json_rpc);
                is_login = true;
            } else {
                debug!(
                    "❎ 登录失败{:?}",
                    String::from_utf8(buf.clone()[0..len].to_vec()).unwrap()
                );
                return w.shutdown().await;
            }
        } else {
            if let Ok(server_json_rpc) = serde_json::from_slice::<ServerId1>(&buf[0..len]) {
                //debug!("Got Result :{:?}", server_json_rpc);
                if (server_json_rpc.id == 6) {
                    info!("✅ 算力提交成功");
                } else {
                    info!("✅ Share Accept");
                }
            } else if let Ok(server_json_rpc) = serde_json::from_slice::<Server>(&buf[0..len]) {
                //debug!("Got jobs {}",server_json_rpc);
                if let Some(diff) = server_json_rpc.result.get(3) {
                    //debug!("✅ Got Job Diff {}", diff);
                }
            } else {
                debug!(
                    "------未捕获封包:{:?}",
                    String::from_utf8(buf.clone()[0..len].to_vec()).unwrap()
                );
            }
        }

        let len = w.write(&buf[0..len]).await?;
        if len == 0 {
            info!("服务端写入失败 断开连接.");
            return w.shutdown().await;
        }
    }
}
