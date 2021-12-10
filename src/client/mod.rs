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
            debug!("收到包大小 : {}", len);

            if let Ok(client_json_rpc) = serde_json::from_slice::<Client>(&buf[0..len]) {
                if client_json_rpc.method == "eth_submitWork" {
                    info!(
                        "矿机 :{} Share #{:?}",
                        client_json_rpc.worker, client_json_rpc.id
                    );
                    //debug!("传递给Server :{:?}", client_json_rpc);
                } else if client_json_rpc.method == "eth_submitHashrate" {
                    if let Some(hashrate) = client_json_rpc.params.get(0) {
                        info!("矿机 :{} 提交本地算力 {}", client_json_rpc.worker, hashrate);
                    }
                } else if client_json_rpc.method == "eth_submitLogin" {
                    info!("矿机 :{} 请求登录", client_json_rpc.worker);
                } else {
                    debug!("矿机传递未知RPC :{:?}", client_json_rpc);
                }

                let write_len = w.write(&buf[0..len]).await?;
                if write_len == 0 {
                    info!("服务器断开连接.");
                    return w.shutdown().await;
                }
            } else if let Ok(_) = serde_json::from_slice::<ClientGetWork>(&buf[0..len]) {
                //debug!("获得任务:{:?}", client_json_rpc);
                let write_len = w.write(&buf[0..len]).await?;
                if write_len == 0 {
                    info!("服务器断开连接.");
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
            if let Ok(server_json_rpc) = serde_json::from_slice::<Server>(&buf[0..len]) {
                //debug!("Got Job :{:?}", server_json_rpc);
                //w_client.write_all(&buf[0..len]).await?;
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
