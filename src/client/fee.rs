use anyhow::Result;
use broadcaster::BroadcastChannel;
use std::sync::Arc;
use tokio::{
    io::{AsyncRead, AsyncWrite, BufReader, Lines, WriteHalf},
    net::TcpStream,
    select,
    sync::{Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard},
    time,
};

use tracing::{debug, info};
//RwLock, RwLockReadGuard, RwLockWriteGuard,
use crate::{
    client::{lines_unwrap, write_to_socket_byte},
    protocol::{
        ethjson::{
            EthClientObject, EthClientRootObject, EthServerRoot,
            EthServerRootObject,
        },
        CLIENT_GETWORK,
    },
};

pub async fn fee(
    chan: BroadcastChannel<Vec<String>>,
    mut proxy_lines: Lines<
        BufReader<tokio::io::ReadHalf<tokio::net::TcpStream>>,
    >,
    w: Arc<Mutex<WriteHalf<TcpStream>>>, worker_name: String,
) -> Result<()> {
    let mut eth_get_work = EthClientRootObject {
        id: CLIENT_GETWORK,
        method: "eth_getWork".into(),
        params: vec![],
    };

    let sleep = time::sleep(tokio::time::Duration::from_secs(10));
    tokio::pin!(sleep);

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
                    } else if let Ok(result_rpc) = serde_json::from_str::<EthServerRoot>(&buf) {
                        tracing::debug!(worker_name = ?worker_name,result_rpc = ?result_rpc," 线程获得操作结果 {:?}",result_rpc.result);
                    }
                }
            },
            () = &mut sleep  => {
                {

                    let mut write = w.lock().await;
                    //同时加2个值
                    write_to_socket_byte(&mut write, eth_get_work.to_vec()?, &worker_name).await?;
                }

                sleep.as_mut().reset(time::Instant::now() + time::Duration::from_secs(10));
            },
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
