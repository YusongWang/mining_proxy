use anyhow::{anyhow, Result};

use tokio::{
    io::{AsyncRead, AsyncWrite, BufReader, Lines, WriteHalf},
    select,
    sync::broadcast::Sender,
    sync::mpsc::Receiver,
};

use crate::protocol::ethjson::EthClientObject;

use crate::{
    client::lines_unwrap,
    protocol::ethjson::{
        EthClientRootObject, EthClientWorkerObject, EthServer,
        EthServerRootObject,
    },
};

use super::write_to_socket_byte;

use tracing::{debug, info};

pub async fn fee_ssl<R: 'static>(
    rx: Receiver<Vec<String>>, chan: Sender<Vec<String>>,
    proxy_lines: Lines<BufReader<tokio::io::ReadHalf<R>>>,
    w: tokio::io::WriteHalf<tokio_native_tls::TlsStream<tokio::net::TcpStream>>,
    worker_name: String,
) -> Result<()>
where
    R: AsyncRead + Send,
{
    let worker_name_write = worker_name.clone();

    let worker_write = tokio::spawn(async move {
        match async_write(rx, worker_name_write, w).await {
            Ok(()) => todo!(),
            Err(e) => std::panic::panic_any(e),
        }
    });

    let worker_reader = tokio::spawn(async move {
        match worker_reader(proxy_lines, chan, worker_name).await {
            Ok(()) => todo!(),
            Err(e) => std::panic::panic_any(e),
        }
    });

    let (_, _) = tokio::join!(worker_write, worker_reader);

    Ok(())
}

pub async fn fee<W: 'static, R: 'static>(
    rx: Receiver<Vec<String>>, chan: Sender<Vec<String>>,
    proxy_lines: Lines<BufReader<tokio::io::ReadHalf<R>>>, w: WriteHalf<W>,
    worker_name: String,
) -> Result<()>
where
    R: AsyncRead + Send,
    W: AsyncWrite + Send,
{
    let worker_name_write = worker_name.clone();

    let worker_write = tokio::spawn(async move {
        match async_write(rx, worker_name_write, w).await {
            Ok(()) => todo!(),
            Err(e) => std::panic::panic_any(e),
        }
    });

    let worker_reader = tokio::spawn(async move {
        match worker_reader(proxy_lines, chan, worker_name).await {
            Ok(()) => todo!(),
            Err(e) => std::panic::panic_any(e),
        }
    });

    let (_, _) = tokio::join!(worker_write, worker_reader);

    return Err(anyhow!("异常退出了"));
}

async fn async_write<W>(
    mut rx: Receiver<Vec<String>>, worker_name: String, mut w: WriteHalf<W>,
) -> Result<()>
where W: AsyncWrite + Send {
    let mut get_work = EthClientRootObject {
        id: 6,
        method: "eth_getWork".into(),
        params: vec![],
    };

    let mut json_rpc = EthClientWorkerObject {
        id: 40,
        method: "eth_submitWork".into(),
        params: vec![],
        worker: worker_name.clone(),
    };

    let sleep = tokio::time::sleep(tokio::time::Duration::from_secs(5));
    tokio::pin!(sleep);
    let mut share_job_idx: u64 = 0;

    loop {
        select! {
            Some(params) = rx.recv() => {
                share_job_idx+=1;
                json_rpc.id = share_job_idx;
		json_rpc.params = params;
                //tracing::debug!(worker_name = ?worker_name,rpc = ?job_rpc,id=share_job_idx," //获得抽水工作份额");
   //     info!(worker=?worker_name,json=?json_rpc,"获得任务");
                write_to_socket_byte(&mut w, json_rpc.to_vec()?, &worker_name).await?;
            },
            () = &mut sleep  => {
                write_to_socket_byte(&mut w, get_work.to_vec()?, &worker_name).await?;
                sleep.as_mut().reset(tokio::time::Instant::now() + tokio::time::Duration::from_secs(10));
            },
        }
    }
}

async fn worker_reader<R>(
    mut proxy_lines: Lines<BufReader<tokio::io::ReadHalf<R>>>,
    chan: Sender<Vec<String>>, worker_name: String,
) -> Result<()>
where
    R: AsyncRead + Send,
{
    loop {
        select! {
            res = proxy_lines.next_line() => {
                let buffer = match lines_unwrap(res,&worker_name,"矿池").await {
                    Ok(buf) => buf,
                    Err(_) => {
                        return Err(anyhow!("抽水旷工掉线了a"));
                    },
                };

                #[cfg(debug_assertions)]
                debug!("1 :  矿池 -> 矿机 {} #{:?}",worker_name, buffer);

                if let Ok(job_rpc) = serde_json::from_str::<EthServerRootObject>(&buffer) {
                    let job_res = job_rpc.get_job_result().unwrap();
                    chan.send(job_res)?;
                } else if let Ok(result_rpc) = serde_json::from_str::<EthServer>(&buffer) {
                    if result_rpc.result == false {
                        tracing::debug!(worker_name = ?worker_name,rpc = ?buffer,"线程获得操作结果 {:?}",result_rpc.result);
                    }
                }
            }

        }
    }
}
// pub async fn p_fee_ssl(
//     mut rx: tokio::sync::mpsc::Receiver<Box<dyn EthClientObject + Send +
// Sync>>,     proxy: Arc<Proxy>, chan: Sender<Vec<String>>,
//     mut proxy_lines: tokio::io::Lines<
//         tokio::io::BufReader<
//             tokio::io::ReadHalf<
//                 tokio_native_tls::TlsStream<tokio::net::TcpStream>,
//             >,
//         >,
//     >,
//     mut w: tokio::io::WriteHalf<
//         tokio_native_tls::TlsStream<tokio::net::TcpStream>,
//     >,
//     worker_name: String,
// ) -> Result<()> {
//     let mut config: Settings;
//     {
//         let rconfig = RwLockReadGuard::map(proxy.config.read().await, |s| s);
//         config = rconfig.clone();
//     }
//     let mut get_work = EthClientRootObject {
//         id: CLIENT_GETWORK,
//         method: "eth_getWork".into(),
//         params: vec![],
//     };
//     let mut share_job_idx = 0;

//     let sleep = time::sleep(tokio::time::Duration::from_secs(5));
//     tokio::pin!(sleep);
//     loop {
//         select! {
//             res = proxy_lines.next_line() => {
//                 let buffer = match
// lines_unwrap(res,&worker_name,"矿池").await {                     Ok(buf) =>
// buf,                     Err(e) => {

//                         info!(worker_name =
// ?worker_name,"退出了。重新登录到池!!");                         let
// (new_lines, dev_w) = crate::client::proxy_pool_login_with_ssl(
// &config,                             config.share_name.clone(),
//                         ).await?;

//                         //同时加2个值
//                         w = dev_w;
//                         proxy_lines = new_lines;
//                         info!(worker_name = ?worker_name,"重新登录成功!!");

//                         continue;
//                     },
//                 };

//                 #[cfg(debug_assertions)]
//                 debug!("1 :  矿池 -> 矿机 {} #{:?}",worker_name, buffer);

//                 let buffer: Vec<_> = buffer.split("\n").collect();
//                 for buf in buffer {
//                     if buf.is_empty() {
//                         continue;
//                     }

//                     if let Ok(job_rpc) =
// serde_json::from_str::<EthServerRootObject>(&buf) {
// let job_res = job_rpc.get_job_result().unwrap();
// chan.send(job_res)?;                     } else if let Ok(result_rpc) =
// serde_json::from_str::<EthServer>(&buf) {                         if
// result_rpc.result == false {
// tracing::debug!(worker_name = ?worker_name,rpc = ?buf," 线程获得操作结果
// {:?}",result_rpc.result);                         }
//                     }
//                 }
//             },
//             Some(mut job_rpc) = rx.recv() => {
//                 share_job_idx+=1;
//                 job_rpc.set_id(share_job_idx);
//                 //tracing::debug!(worker_name = ?worker_name,rpc =
// ?job_rpc,id=share_job_idx," 获得抽水工作份额");
// write_to_socket_byte(&mut w, job_rpc.to_vec()?, &worker_name).await?
//             },
//             () = &mut sleep  => {
//                 write_to_socket_byte(&mut w, get_work.to_vec()?,
// &worker_name).await?;
// sleep.as_mut().reset(time::Instant::now() + time::Duration::from_secs(10));
//             },
//         }
//     }
// }
// pub async fn fee<R, W>(
//     mut rx: tokio::sync::mpsc::Receiver<Box<dyn EthClientObject + Send +
// Sync>>,     proxy: Arc<Proxy>, chan: Sender<Vec<String>>,
//     mut proxy_lines: Lines<BufReader<tokio::io::ReadHalf<R>>>,
//     mut w: WriteHalf<W>, worker_name: String,
// ) -> Result<()>
// where
//     R: AsyncRead,
//     W: AsyncWrite,
// {
//     let mut config: Settings;
//     {
//         let rconfig = RwLockReadGuard::map(proxy.config.read().await, |s| s);
//         config = rconfig.clone();
//     }

//     loop {
//         select! {
//             res = proxy_lines.next_line() => {
//                 let buffer = match
// lines_unwrap(res,&worker_name,"矿池").await {                     Ok(buf) =>
// buf,                     Err(e) => {

//                         info!(worker_name =
// ?worker_name,"退出了。重新登录到池!!");                         let
// (new_lines, dev_w) = crate::client::proxy_pool_login(
// &config,                             config.share_name.clone(),
//                         ).await?;

//                         //同时加2个值
//                         w = dev_w;
//                         proxy_lines = new_lines;
//                         info!(worker_name = ?worker_name,"重新登录成功!!");

//                         continue;
//                     },
//                 };

//                 #[cfg(debug_assertions)]
//                 debug!("1 :  矿池 -> 矿机 {} #{:?}",worker_name, buffer);

//                 let buffer: Vec<_> = buffer.split("\n").collect();
//                 for buf in buffer {
//                     if buf.is_empty() {
//                         continue;
//                     }

//                     if let Ok(job_rpc) =
// serde_json::from_str::<EthServerRootObject>(&buf) {
// let job_res = job_rpc.get_job_result().unwrap();
// chan.send(job_res)?;                     } else if let Ok(result_rpc) =
// serde_json::from_str::<EthServer>(&buf) {                         if
// result_rpc.result == false {
// tracing::debug!(worker_name = ?worker_name,rpc = ?buf," 线程获得操作结果
// {:?}",result_rpc.result);                         }
//                     }
//                 }
//             },
//             Some(mut job_rpc) = rx.recv() => {
//                 write_to_socket_byte(&mut w, job_rpc.to_vec()?,
// &worker_name).await?             }
//         }
//     }
// }
