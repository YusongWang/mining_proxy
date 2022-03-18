use std::net::SocketAddr;

use anyhow::{anyhow, Result};
use tracing::{debug, info};

use tokio::{
    io::AsyncBufReadExt,
    net::{TcpListener, TcpStream},
    select,
};

pub async fn accept_encrypt_tcp(port: i32, server: SocketAddr) -> Result<()> {
    let address = format!("0.0.0.0:{}", port);
    let listener = TcpListener::bind(address.clone()).await?;
    info!("本地加密协议端口{}启动成功!!!", &address);
    loop {
        let (stream, addr) = listener.accept().await?;

        info!(ip=?addr,"有新机器上线了");
        tokio::spawn(async move {
            match transfer(stream, addr).await {
                Ok(_) => {
                    info!(ip=?addr,"矿机安全下线");
                }
                Err(e) => {
                    info!(ip=?addr,err = ?e,"矿机下线");
                }
            }
        });
    }
}

async fn transfer(stream: TcpStream, addr: SocketAddr) -> Result<()> {
    let (worker_r, mut _worker_w) = tokio::io::split(stream);
    let worker_r = tokio::io::BufReader::new(worker_r);
    let mut worker_r = worker_r.lines();

    info!("开始读取链接");
    let first_pacakge = match worker_r.next_line().await {
        Ok(res) => match res {
            Some(buf) => buf,
            None => {
                return Err(anyhow!("矿机下线"));
            }
        },
        Err(e) => return Err(anyhow!("读取矿机输入超时 {}", e)),
    };

    dbg!(first_pacakge);
    //parse package

    //if package not pensend. count+1

    //if count == 5 block the IpAddress.

    loop {
        select! {
            res = worker_r.next_line() => {
                let buffer = match res {
                    Ok(res) => match res {
                        Some(buf) => buf,
                        None => return Err(anyhow!("矿机下线了"))
                    },
                    Err(e) => return Err(anyhow!("读取超时了 矿机下线了: {}",e)),
                };

                #[cfg(debug_assertions)]
                debug!("------> :  矿机 -> 矿池  {:?}", buffer);
                let buffer: Vec<_> = buffer.split("\n").collect();
                for buf in buffer {
                    if buf.is_empty() {
                        continue;
                    }
            dbg!(buf);
                }
            }
        }
    }
}
