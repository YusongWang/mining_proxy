use std::net::TcpStream;

use anyhow::Result;

use log::{debug, info};
use tokio::io::{split, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadHalf, WriteHalf};
//use tokio::io::{split, AsyncReadExt, AsyncWriteExt};
use crate::protocol::rpc::eth::{Client, ClientGetWork, Server, ServerId1};

pub mod tcp;
pub mod tls;

async fn client_to_server<R, W>(
    mut r: ReadHalf<R>,
    mut w: WriteHalf<W>,
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
                    info!(
                        "✅ 矿机 :{} Share #{:?}",
                        client_json_rpc.worker, client_json_rpc.id
                    );
                    //debug!("传递给Server :{:?}", client_json_rpc);
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
) -> Result<(), std::io::Error>
where
    R: AsyncRead,
    W: AsyncWrite,
{
    let mut is_login = false;
    let mut worker: String;
    let mut id: u32;

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
                info!(
                    "❎ 登录失败{:?}",
                    String::from_utf8(buf.clone()[0..len].to_vec()).unwrap()
                );
                return w.shutdown().await;
            }
        } else {
            if let Ok(server_json_rpc) = serde_json::from_slice::<ServerId1>(&buf[0..len]) {
                //debug!("Got Result :{:?}", server_json_rpc);
                info!("✅ 矿机 Share Accept");
            } else if let Ok(server_json_rpc) = serde_json::from_slice::<Server>(&buf[0..len]) {
                //debug!("Got jobs {}",server_json_rpc);
                if let Some(diff) = server_json_rpc.result.get(3) {
                    info!("✅ Got Job Diff {}", diff);
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
