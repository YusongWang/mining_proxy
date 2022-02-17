use anyhow::Result;
use std::sync::Arc;
use tokio::{
    select,
    sync::{RwLock, RwLockWriteGuard},
};
use tracing::{debug, info};
//RwLock, RwLockReadGuard, RwLockWriteGuard,
use crate::{
    client::lines_unwrap, protocol::ethjson::EthServerRootObject,
    util::config::Settings,
};

use super::proxy_pool_login;

pub async fn fee(
    config: &Settings, job: Arc<RwLock<Vec<String>>>,
) -> Result<()> {
    let worker_name = config.share_name.clone();

    let (mut proxy_lines, mut proxy_w) =
        proxy_pool_login(&config, worker_name.clone()).await?;

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

                    if let Ok(mut job_rpc) = serde_json::from_str::<EthServerRootObject>(&buf) {
                        // 推送多少次任务？
                        //let job_id = job_rpc.get_job_id().unwrap();

                        let mut job_res = job_rpc.get_job_result().unwrap();
                        job_res.push("fee".into());
                        // {
                        //     let mut j = job.lock().unwrap();
                        //     *j = job_res;
                        // }

                        {
                            let mut job = RwLockWriteGuard::map(job.write().await, |s| s);
                            *job = job_res;
                        }
                    }
                }
            }
        }
    }
}
