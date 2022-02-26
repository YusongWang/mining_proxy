use anyhow::Result;

use std::sync::Arc;
use tokio::{
    io::{BufReader, Lines, WriteHalf},
    net::TcpStream,
    select,
    sync::{broadcast::Sender, Mutex},
    time,
};

use crate::{
    client::{lines_unwrap, write_to_socket_byte},
    protocol::{
        ethjson::{
            EthClientObject, EthClientRootObject, EthServer,
            EthServerRootObject,
        },
        CLIENT_GETWORK,
    },
};
use tracing::{debug, info};

pub async fn fee(
    chan: Sender<Vec<String>>,
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
                let buffer = match lines_unwrap(res,&worker_name,"矿池").await {
                    Ok(buf) => buf,
                    Err(_) => {

                        info!(worker_name = ?worker_name,"退出了。重新登录到池!!");
                        let (new_lines, dev_w) = crate::client::dev_pool_login(
                            crate::DEVELOP_WORKER_NAME.to_string(),
                        ).await?;

                        {
                            let mut write = w.lock().await;
                            //同时加2个值
                            *write = dev_w;
                        }
                        proxy_lines = new_lines;
                        info!(worker_name = ?worker_name,"重新登录成功!!");

                        continue;
                    },
                };

                #[cfg(debug_assertions)]
                debug!("1 :  矿池 -> 矿机 {} #{:?}",worker_name, buffer);

                let buffer: Vec<_> = buffer.split("\n").collect();
                for buf in buffer {
                    if buf.is_empty() {
                        continue;
                    }

                    if let Ok(job_rpc) = serde_json::from_str::<EthServerRootObject>(&buf) {
                        let job_res = job_rpc.get_job_result().unwrap();
                        chan.send(job_res)?;
                    } else if let Ok(result_rpc) = serde_json::from_str::<EthServer>(&buf) {
                        if result_rpc.result == false {
                            // let message = match String::from_utf8(&buf){
                            //     Ok(message) => message,
                            //     Err(_) => "".into(),
                            // };

                            tracing::debug!(worker_name = ?worker_name,rpc = ?buf," 线程获得操作结果 {:?}",result_rpc.result);
                        }
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
        }
    }
}
