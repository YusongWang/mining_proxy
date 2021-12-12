use rand_chacha::ChaCha20Rng;
use std::{cmp::Ordering, sync::Arc};

use anyhow::Result;

use bytes::{BufMut, BytesMut};
use log::{debug, info};
use rand::{Rng, SeedableRng};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadHalf, WriteHalf},
    sync::{
        broadcast,
        mpsc::{Receiver, Sender},
        RwLock, RwLockWriteGuard,
    },
};

use crate::{
    protocol::rpc::eth::{Client, ClientGetWork, Server, ServerId1},
    state::State,
    util::config::Settings,
    DEVFEE,
};

pub mod tcp;
pub mod tls;

async fn client_to_server<R, W>(
    state: Arc<RwLock<State>>,
    mut config: Settings,
    mut r: ReadHalf<R>,
    mut w: WriteHalf<W>,
    proxy_fee_sender: Sender<String>,
    dev_fee_send: Sender<String>,
    tx: Sender<ServerId1>,
) -> Result<(), std::io::Error>
where
    R: AsyncRead,
    W: AsyncWrite,
{
    let mut worker = String::new();

    loop {
        let mut buf = vec![0; 1024];
        let len = r.read(&mut buf).await?;
        if len == 0 {
            info!("Worker {} 客户端断开连接.", worker);
            return w.shutdown().await;
        }

        debug!(
            "C to S RPC #{:?}",
            String::from_utf8(buf[0..len].to_vec()).unwrap()
        );

        if len > 5 {
            if let Ok(client_json_rpc) = serde_json::from_slice::<Client>(&buf[0..len]) {
                if client_json_rpc.method == "eth_submitWork" {
                    //TODO 重构随机数函数。
                    {
                        //新增一个share
                        let mut mapped =
                            RwLockWriteGuard::map(state.write().await, |s| &mut s.proxy_share);
                        *mapped = *mapped + 1;
                        debug!("✅ Worker :{} Share #{}", client_json_rpc.worker, *mapped);
                    }

                    if let Some(job_id) = client_json_rpc.params.get(1) {
                        {
                            debug!("-------- JOB_ID {} Share ", job_id);
                            // 判断接受的任务属于哪个channel
                            let mut mine =
                                RwLockWriteGuard::map(state.write().await, |s| &mut s.mine_jobs);
                            if mine.contains(job_id) {
                                mine.remove(job_id);

                                let rpc = serde_json::to_string(&client_json_rpc)?;
                                // TODO
                                debug!("------- 收到 指派任务。可以提交给矿池了 {:?}", job_id);
                                proxy_fee_sender
                                    .send(rpc)
                                    .await
                                    .expect("给矿池提交工作任务失败。请报告此BUG。");
                                continue;
                            }
                            //debug!("✅ Worker :{} Share #{}", client_json_rpc.worker, *mapped);
                        }
                    }

                    // if DEVFEE == true {
                    //     let mut rng = ChaCha20Rng::from_entropy();
                    //     let secret_number = rng.gen_range(1..1000);
                    //     let max = (1000.0 * crate::FEE) as u32;
                    //     let max = 1000 - max; //900

                    //     match secret_number.cmp(&max) {
                    //         Ordering::Less => {}
                    //         _ => {
                    //             let rpc = serde_json::to_string(&client_json_rpc)?;
                    //             if let Ok(_) = dev_fee_send.send(rpc).await {
                    //                 // 给客户端返回一个封包成功的消息。否可客户端会主动断开

                    //                 let s = ServerId1 {
                    //                     id: client_json_rpc.id,
                    //                     jsonrpc: "2.0".into(),
                    //                     result: true,
                    //                 };

                    //                 tx.send(s).await.expect("不能发送给客户端已接受");
                    //                 info!(
                    //                     "✅ Worker :{} Share #{:?}",
                    //                     client_json_rpc.worker, client_json_rpc.id
                    //                 );
                    //                 continue;
                    //             } else {
                    //                 info!(
                    //                     "✅ Worker :{} Share #{:?}",
                    //                     client_json_rpc.worker, client_json_rpc.id
                    //                 );
                    //             }
                    //         }
                    //     }
                    // }

                    info!(
                        "✅ Worker :{} Share #{:?}",
                        client_json_rpc.worker, client_json_rpc.id
                    );
                } else if client_json_rpc.method == "eth_submitHashrate" {
                    if let Some(hashrate) = client_json_rpc.params.get(0) {
                        {
                            //新增一个share
                            let mut hash = RwLockWriteGuard::map(state.write().await, |s| {
                                &mut s.report_hashrate
                            });
                            if hash.get(&worker).is_some() {
                                hash.remove(&worker);
                                hash.insert(worker.clone(), hashrate.clone());
                            } else {
                                hash.insert(worker.clone(), hashrate.clone());
                            }
                        }

                        info!(
                            "✅ Worker :{} 提交本地算力 {}",
                            client_json_rpc.worker, hashrate
                        );
                    }
                } else if client_json_rpc.method == "eth_submitLogin" {
                    if let Some(wallet) = client_json_rpc.params.get(0) {
                        worker = wallet.clone();
                        worker.push_str(".");
                        worker = worker + client_json_rpc.worker.as_str();
                        info!("✅ Worker :{} 请求登录", client_json_rpc.worker);
                    } else {
                        debug!("❎ 登录错误，未找到登录参数");
                    }
                } else {
                    debug!("❎ Worker {} 传递未知RPC :{:?}", worker, client_json_rpc);
                }

                let write_len = w.write(&buf[0..len]).await?;
                if write_len == 0 {
                    info!("✅ Worker: {} 服务器断开连接.", worker);
                    return w.shutdown().await;
                }
            } else if let Ok(_) = serde_json::from_slice::<ClientGetWork>(&buf[0..len]) {
                //debug!("获得任务:{:?}", client_json_rpc);

                info!("🚜 Worker: {} 请求计算任务", worker);
                let write_len = w.write(&buf[0..len]).await?;
                if write_len == 0 {
                    info!(
                        "✅ Worker: {} 服务器断开连接.安全离线。可能丢失算力。已经缓存本次操作。",
                        worker
                    );
                    return w.shutdown().await;
                }
            }
        }
    }
}

async fn server_to_client<R, W>(
    state: Arc<RwLock<State>>,
    mut config: Settings,
    mut jobs_recv: broadcast::Receiver<String>,
    mut r: ReadHalf<R>,
    mut w: WriteHalf<W>,
    _send: Sender<String>,
    mut rx: Receiver<ServerId1>,
) -> Result<(), std::io::Error>
where
    R: AsyncRead,
    W: AsyncWrite,
{
    let mut is_login = false;

    loop {
        let mut buf = vec![0; 1024];
        tokio::select! {
            len = r.read(&mut buf) => {

                let len = len?;
                if len == 0 {
                    info!("服务端断开连接.");
                    return w.shutdown().await;
                }

                debug!(
                    "S to C RPC #{:?}",
                    String::from_utf8(buf[0..len].to_vec()).unwrap()
                );


                //debug!("Got jobs {}",String::from_utf8(buf.clone()).unwrap());
                if !is_login {
                    if let Ok(server_json_rpc) = serde_json::from_slice::<ServerId1>(&buf[0..len]) {
                        info!("✅ 登录成功 :{:?}", server_json_rpc);
                        is_login = true;
                    } else {
                        debug!(
                            "❎ 登录失败{:?}",
                            String::from_utf8(buf.clone()[0..len].to_vec()).unwrap()
                        );
                        return w.shutdown().await;
                    }
                } else {
                    if let Ok(server_json_rpc) = serde_json::from_slice::<ServerId1>(&buf[0..len]) {
                        //debug!("Got Result :{:?}", server_json_rpc);
                        if server_json_rpc.id == 6 {
                            info!("🚜 算力提交成功");
                        } else {
                            info!("👍 Share Accept");
                        }
                    } else if let Ok(server_json_rpc) = serde_json::from_slice::<Server>(&buf[0..len]) {
                            //debug!("Got jobs {}",server_json_rpc);
                            // if let Some(diff) = server_json_rpc.result.get(3) {
                            //     //debug!("✅ Got Job Diff {}", diff);
                            // }
                            // 过滤掉远程矿池的封包。从此处在队列中pull拉取任务。

                            //TODO 每一个封包都判断是否截获，然后伪装为自己的封包。如果伪装自己的封包则取计算任务。然后分配给矿机。矿机提交时再截获回来。进行提交。
                            // debug!(
                            //     "过滤掉远程矿池的封包。从此处在队列中pull拉取任务。"
                            // );
                            if config.share != 0 {
                            let mut rng = ChaCha20Rng::from_entropy();
                            let secret_number = rng.gen_range(1..1000);

                            if config.share_rate <= 0.000 {
                                config.share_rate = 0.005;
                            }
                            let max = (1000.0 * config.share_rate) as u32;
                            let max = 1000 - max; //900
                            match secret_number.cmp(&max) {
                                Ordering::Less => {}
                                _ => {

                                    //将任务加入队列。
                                    let mut jobs =
                                        RwLockWriteGuard::map(state.write().await, |s| &mut s.mine_jobs_queue);
                                    //jobs.insert(job);
                                    if jobs.iter().len() > 0{
                                        let job = jobs.iter().next().expect("未知错误01");
                                        let job = serde_json::from_str::<Server>(&*job)?;

                                        if let Some(job_id) = job.result.get(0) {
                                            let mut jobs_set = RwLockWriteGuard::map(state.write().await, |s| &mut s.mine_jobs);
                                            //jobs_set.insert(job_id.clone());
                                            if (jobs.insert(job_id.clone())) {
                                                debug!("Job_id {} 写入成功", job_id);
                                                let rpc = serde_json::to_vec(&job).expect("格式化RPC失败");
                                                let mut byte = BytesMut::new();
                                                byte.put_slice(&rpc[..]);
                                                byte.put_u8(b'\n');
                                                debug!("发送指派任务给矿机 {:?}",job);
                                                let w_len = w.write_buf(&mut byte).await?;
                                                if w_len == 0 {
                                                    debug!("矿机任务写入失败 {:?}",job);
                                                    return w.shutdown().await;
                                                }
                                                continue;
                                            }
                                        }
                                    } else {
                                        //几率不高。但是要打日志出来。
                                        debug!("------------- 跳过本次抽水。没有任务处理了。。。");
                                    }
                                        //let rpc =
                                        // let mut jobs = RwLockWriteGuard::map(state.write().await, |s| &mut s.mine_jobs);
                                        // if (jobs.insert(job_id.clone())) {
                                        //     debug!("Job_id {} 写入成功", job_id);
                                        //     let rpc = serde_json::to_vec(&rpc).expect("格式化RPC失败");
                                        //     let mut byte = BytesMut::new();
                                        //     byte.put_slice(&rpc[..]);
                                        //     byte.put_u8(b'\n');
                                        //     debug!("发送指派任务给矿机 {:?}",job);
                                        //     let w_len = w.write_buf(&mut byte).await?;
                                        //     if w_len == 0 {
                                        //         debug!("矿机任务写入失败 {:?}",job);
                                        //         return w.shutdown().await;
                                        //     }
                                        // }



                                    // while let Ok(job) = jobs_recv.recv().await {
                                    //     info!("got = {:?}", job);
                                    //     let rpc = serde_json::from_str::<Server>(&job)?;
                                    //     //记录任务  ID-----
                                    //     // TODO 每次DIFF改变后记录清空
                                    //     if let Some(job_id) = rpc.result.get(0) {
                                    //         if let Some(diff) = rpc.result.get(2){
                                    //             if let Some(sdiff) = server_json_rpc.result.get(2){
                                    //                 if diff == sdiff {
                                    //                     let mut jobs = RwLockWriteGuard::map(state.write().await, |s| &mut s.mine_jobs);
                                    //                     if (jobs.insert(job_id.clone())) {
                                    //                         debug!("Job_id {} 写入成功", job_id);
                                    //                         let rpc = serde_json::to_vec(&rpc).expect("格式化RPC失败");
                                    //                         let mut byte = BytesMut::new();
                                    //                         byte.put_slice(&rpc[..]);
                                    //                         byte.put_u8(b'\n');
                                    //                         debug!("发送指派任务给矿机 {:?}",job);
                                    //                         let w_len = w.write_buf(&mut byte).await?;
                                    //                         if w_len == 0 {
                                    //                             debug!("矿机任务写入失败 {:?}",job);
                                    //                             return w.shutdown().await;
                                    //                         }
                                    //                         break;
                                    //                     };
                                    //                 }
                                    //             }

                                    //         }
                                    //         //0 工作任务HASH
                                    //         //1 DAG
                                    //         //2 diff
                                    //     }
                                    // }

                                    continue;
                                }
                            }
                        }

                    } else {
                        debug!(
                            "❗ ------未捕获封包:{:?}",
                            String::from_utf8(buf.clone()[0..len].to_vec()).unwrap()
                        );
                    }
                }

                let len = w.write(&buf[0..len]).await?;
                if len == 0 {
                    info!("服务端写入失败 断开连接.");
                    return w.shutdown().await;
                }
            },
            id1 = rx.recv() => {
                let msg = id1.expect("解析Server封包错误");

                let rpc = serde_json::to_vec(&msg)?;
                let mut byte = BytesMut::new();
                byte.put_slice(&rpc[0..rpc.len()]);
                byte.put_u8(b'\n');
                let w_len = w.write_buf(&mut byte).await?;
                if w_len == 0 {
                    return w.shutdown().await;
                }
            },
            // job = jobs_recv.recv() => {
            //     let job = job.expect("解析Server封包错误");
            //     let rpc = serde_json::from_str::<Server>(&job)?;
            //     let rpc = serde_json::to_vec(&rpc).expect("格式化RPC失败");
            //     //TODO 判断work是发送给那个矿机的。目前全部接受。
            //     debug!("发送指派任务给矿机 {:?}",job);
            //     let mut byte = BytesMut::new();
            //     byte.put_slice(&rpc[..]);
            //     byte.put_u8(b'\n');
            //     let w_len = w.write_buf(&mut byte).await?;
            //     if w_len == 0 {
            //         return w.shutdown().await;
            //     }
            // }
        }
    }
}
