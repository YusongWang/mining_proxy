use anyhow::{anyhow, Result};

use std::sync::Arc;
use tokio::{
    io::{AsyncRead, AsyncWrite, BufReader, Lines, WriteHalf},
    net::TcpStream,
    select,
    sync::{broadcast::Sender, RwLockReadGuard},
    time,
};

use crate::{
    client::lines_unwrap,
    protocol::{
        ethjson::{
            EthClientObject, EthClientRootObject, EthServer,
            EthServerRootObject,
        },
        CLIENT_GETWORK,
    },
    proxy::Proxy,
    util::config::Settings,
};

use tracing::debug;

pub async fn fee_ssl(
    mut rx: tokio::sync::mpsc::Receiver<Box<dyn EthClientObject + Send + Sync>>,
    chan: Sender<Vec<String>>,
    mut proxy_lines: tokio::io::Lines<
        tokio::io::BufReader<
            tokio::io::ReadHalf<
                tokio_native_tls::TlsStream<tokio::net::TcpStream>,
            >,
        >,
    >,
    // mut w: tokio::io::WriteHalf<
    //     tokio_native_tls::TlsStream<tokio::net::TcpStream>,
    // >,
    worker_name: String,
) -> Result<()> {
    // let mut get_work = EthClientRootObject {
    //     id: CLIENT_GETWORK,
    //     method: "eth_getWork".into(),
    //     params: vec![],
    // };

    // let sleep = time::sleep(tokio::time::Duration::from_secs(5));
    // tokio::pin!(sleep);

    //let mut share_job_idx = 0;

    loop {
        select! {
            res = proxy_lines.next_line() => {
                let buffer = match lines_unwrap(res,&worker_name,"矿池").await {
                    Ok(buf) => buf,
                    Err(e) => {
                        //info!(worker_name = ?worker_name,"退出了。重新登录到池!!");
                        return Err(anyhow!("抽水旷工掉线了a"));
                        //anyhow::bail!(e);
                        // info!(worker_name = ?worker_name,"退出了。重新登录到池!!");
                        // let (new_lines, dev_w) = crate::client::dev_pool_ssl_login(
                        //     crate::DEVELOP_WORKER_NAME.to_string(),
                        // ).await?;

                        // //同时加2个值
                        // w = dev_w;

                        // proxy_lines = new_lines;
                        // info!(worker_name = ?worker_name,"重新登录成功!!");
                        // continue;
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
                            tracing::debug!(worker_name = ?worker_name,rpc = ?buf," 线程获得操作结果 {:?}",result_rpc.result);
                        }
                    }
                }
            },
            // Some(mut job_rpc) = rx.recv() => {
            //     share_job_idx+=1;
            //     job_rpc.set_id(share_job_idx);
            //     //tracing::debug!(worker_name = ?worker_name,rpc = ?job_rpc,id=share_job_idx," 获得抽水工作份额");
            //     write_to_socket_byte(&mut w, job_rpc.to_vec()?, &worker_name).await?
            // },
            // () = &mut sleep  => {
            //     write_to_socket_byte(&mut w, get_work.to_vec()?, &worker_name).await?;
            //     sleep.as_mut().reset(time::Instant::now() + time::Duration::from_secs(10));
            // },
        }
    }
}

pub async fn fee<R>(
    mut rx: tokio::sync::mpsc::Receiver<Box<dyn EthClientObject + Send + Sync>>,
    proxy: Arc<Proxy>,
    chan: Sender<Vec<String>>,
    mut proxy_lines: Lines<BufReader<tokio::io::ReadHalf<R>>>,
    //mut w: WriteHalf<W>,
    worker_name: String,
) -> Result<()>
where
    R: AsyncRead,
{
    let mut config: Settings;
    {
        let rconfig = RwLockReadGuard::map(proxy.config.read().await, |s| s);
        config = rconfig.clone();
    }
    let mut get_work = EthClientRootObject {
        id: CLIENT_GETWORK,
        method: "eth_getWork".into(),
        params: vec![],
    };

    // let sleep = time::sleep(tokio::time::Duration::from_secs(5));
    // tokio::pin!(sleep);
    let mut share_job_idx = 0;

    loop {
        select! {
                    res = proxy_lines.next_line() => {
                        let buffer = match lines_unwrap(res,&worker_name,"矿池").await {
                            Ok(buf) => buf,
                            Err(e) => {

                                //info!(worker_name = ?worker_name,"退出了。重新登录到池!!");
                                return Err(anyhow!("抽水旷工掉线了b"));
                                // let (new_lines, dev_w) = crate::client::proxy_pool_login(
                                //     &config,
                                //     config.share_name.clone(),
                                // ).await?;

                                // //同时加2个值
                                // w = dev_w;
                                // proxy_lines = new_lines;
                                // info!(worker_name = ?worker_name,"重新登录成功!!");

                                // continue;
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
                                    tracing::debug!(worker_name = ?worker_name,rpc = ?buf," 线程获得操作结果 {:?}",result_rpc.result);
                                }
                            }
                        }
                    }
        /*             Some(mut job_rpc) = rx.recv() => {
                        share_job_idx+=1;
                        job_rpc.set_id(share_job_idx);
                        //tracing::debug!(worker_name = ?worker_name,rpc = ?job_rpc,id=share_job_idx," 获得抽水工作份额");
                        write_to_socket_byte(&mut w, job_rpc.to_vec()?, &worker_name).await?
                    },
                    () = &mut sleep  => {
                        write_to_socket_byte(&mut w, get_work.to_vec()?, &worker_name).await?;
                        sleep.as_mut().reset(time::Instant::now() + time::Duration::from_secs(10));
                    }, */
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
