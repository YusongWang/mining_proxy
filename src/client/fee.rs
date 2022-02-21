use anyhow::Result;
use broadcaster::BroadcastChannel;
use std::sync::Arc;
use tokio::{
    io::{AsyncRead, AsyncWrite, BufReader, Lines},
    net::TcpStream,
    select,
    sync::{RwLock, RwLockReadGuard, RwLockWriteGuard},
};
use tracing::{debug, info};
//RwLock, RwLockReadGuard, RwLockWriteGuard,
use crate::{
    client::{lines_unwrap, write_to_socket_byte},
    protocol::ethjson::{EthServerRoot, EthServerRootObject},
    proxy::Proxy,
    util::config::Settings,
};

pub async fn fee(
    chan: BroadcastChannel<Vec<String>>,
    mut proxy_lines: Lines<
        BufReader<tokio::io::ReadHalf<tokio::net::TcpStream>>,
    >,
    worker_name: String,
) -> Result<()> {
    //let mut chan = proxy.chan.clone();
    //let job_send = proxy.job_send.clone();
    //let mut proxy_w = Arc::clone(&proxy.proxy_write);
    //let mut proxy_w = *proxy_w.clone();

    //let mut proxy_w = Arc::g(&mut proxy_w).unwrap();
    //let mut proxy_w = *proxy_w;

    loop {
        select! {
            res = proxy_lines.next_line() => {
                let buffer = lines_unwrap(res,&worker_name,"矿池").await?;

                #[cfg(debug_assertions)]
                debug!("1 :  矿池 -> 矿机 {} #{:?}",worker_name, buffer);

                let buffer: Vec<_> = buffer.split("\n").collect();
                for buf in buffer {
                    if buf.is_empty() {
                        continue;
                    }

                    #[cfg(debug_assertions)]
                    tracing::info!(
                        "1    ---- Worker : {}  Send Rpc {}",
                        worker_name,
                        buf
                    );

                    if let Ok(job_rpc) = serde_json::from_str::<EthServerRootObject>(&buf) {
                        let job_res = job_rpc.get_job_result().unwrap();
                        chan.send(&job_res).await?;
                        // {
                        //     let mut job = RwLockWriteGuard::map(proxy.job.write().await, |f| f);
                        //     job.push_back(job_res);
                        // }

                    } else if let Ok(result_rpc) = serde_json::from_str::<EthServerRoot>(&buf) {
                        tracing::debug!(result_rpc = ?result_rpc,"ProxyFee 线程获得操作结果 {:?}",result_rpc.result);
                    }
                }
            }
            // Ok(json_rpc) = recv.recv() => {
            //     tracing::debug!(json=?json_rpc,"获得抽水任务");
            //     if let crate::client::FEE::PROXYFEE(mut rpc) = json_rpc {
            //         rpc.set_worker_name(&worker_name);
            //         write_to_socket_byte(&mut *proxy_w, rpc.to_vec()?, &worker_name).await?
            //     }
            // }
        }
    }
}
