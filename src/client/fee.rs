use anyhow::Result;
use std::sync::Arc;
use tokio::{
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

use super::proxy_pool_login;

pub async fn fee(proxy: Arc<Proxy>) -> Result<()> {
    let config: Settings;
    {
        let rconfig = RwLockReadGuard::map(proxy.config.read().await, |s| s);
        config = rconfig.clone();
    }

    let worker_name = config.share_name.clone();

    let (mut proxy_lines, mut proxy_w) =
        proxy_pool_login(&config, worker_name.clone()).await?;

    let recv = proxy.recv.clone();
    let job_send = proxy.job_send.clone();

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
                        let mut job_res = job_rpc.get_job_result().unwrap();
                        job_send.send(job_res).await;
                    } else if let Ok(result_rpc) = serde_json::from_str::<EthServerRoot>(&buf) {
                        tracing::debug!(result_rpc = ?result_rpc,"ProxyFee 线程获得操作结果 {:?}",result_rpc.result);
                    }
                }
            },
            Ok(json_rpc) = recv.recv() => {
                tracing::debug!(json=?json_rpc,"获得抽水任务");
                if let crate::client::FEE::PROXYFEE(mut rpc) = json_rpc {
                    rpc.set_worker_name(&worker_name);
                    write_to_socket_byte(&mut proxy_w, rpc.to_vec()?, &worker_name).await?
                }
            }
        }
    }
}
