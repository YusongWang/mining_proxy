use futures::StreamExt;

use serde::Serialize;
use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
};

use anyhow::{bail, Result};

use log::{debug, info};

use tokio::{
    io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, WriteHalf},
    net::TcpStream,
    select,
    sync::broadcast,
    time,
};

use crate::{
    client::{parse_client, parse_client_workername, write_to_socket_string},
    jobs::JobQueue,
    protocol::{
        rpc::eth::{Server, ServerId1, ServerJobsWithHeight, ServerRpc, ServerSideJob},
        CLIENT_GETWORK, CLIENT_LOGIN, CLIENT_SUBHASHRATE, SUBSCRIBE,
    },
    state::Worker,
    util::config::Settings,
};

use super::write_to_socket;

async fn shutdown<W>(w: &mut WriteHalf<W>) -> Result<()>
where
    W: AsyncWrite,
{
    match w.shutdown().await {
        Ok(_) => Ok(()),
        Err(_) => bail!("关闭Pool 链接失败"),
    }
}

async fn eth_submitLogin<W, T>(
    worker: &mut Worker,
    w: &mut WriteHalf<W>,
    rpc: &mut T,
    worker_name: &mut String,
) -> Result<()>
where
    W: AsyncWrite,
    T: crate::protocol::rpc::eth::ClientRpc + Serialize,
{
    if let Some(wallet) = rpc.get_wallet() {
        //rpc.id = CLIENT_LOGIN;
        rpc.set_id(CLIENT_LOGIN);
        let mut temp_worker = wallet.clone();
        temp_worker.push_str(".");
        temp_worker = temp_worker + rpc.get_worker_name().as_str();
        worker.login(temp_worker.clone(), rpc.get_worker_name(), wallet.clone());
        *worker_name = temp_worker;
        write_to_socket(w, &rpc, &worker_name).await
    } else {
        bail!("请求登录出错。可能收到暴力攻击");
    }
}

async fn eth_submitWork<W, W1, T>(
    worker: &mut Worker,
    pool_w: &mut WriteHalf<W>,
    worker_w: &mut WriteHalf<W1>,
    rpc: &mut T,
    worker_name: &String,
    mine_send_jobs: &mut HashMap<String, (u64, u64)>,
    develop_send_jobs: &mut HashMap<String, (u64, u64)>,
    proxy_fee_sender: &broadcast::Sender<(u64, String)>,
    develop_fee_sender: &broadcast::Sender<(u64, String)>,
) -> Result<()>
where
    W: AsyncWrite,
    W1: AsyncWrite,
    T: crate::protocol::rpc::eth::ClientRpc + Serialize,
{
    worker.share_index_add();

    if let Some(job_id) = rpc.get_job_id() {
        if mine_send_jobs.contains_key(&job_id) {
            if let Some(thread_id) = mine_send_jobs.remove(&job_id) {
                let rpc_string = serde_json::to_string(&rpc)?;

                proxy_fee_sender
                    .send((thread_id.0, rpc_string))
                    .expect("可以提交给矿池任务失败。通道异常了");

                let s = ServerId1 {
                    id: rpc.get_id(),
                    //jsonrpc: "2.0".into(),
                    result: true,
                };
                write_to_socket(worker_w, &s, &worker_name).await
            } else {
                bail!("任务失败.找到jobid .但是remove失败了");
            }
        } else if develop_send_jobs.contains_key(&job_id) {
            if let Some(thread_id) = develop_send_jobs.remove(&job_id) {
                let rpc_string = serde_json::to_string(&rpc)?;

                //debug!("------- 开发者 收到 指派任务。可以提交给矿池了 {:?}", job_id);

                develop_fee_sender
                    .send((thread_id.0, rpc_string))
                    .expect("可以提交给矿池任务失败。通道异常了");
                let s = ServerId1 {
                    id: rpc.get_id(),
                    //jsonrpc: "2.0".into(),
                    result: true,
                };
                write_to_socket(worker_w, &s, &worker_name).await
            } else {
                bail!("任务失败.找到jobid .但是remove失败了");
            }
        } else {
            rpc.set_id(worker.share_index);
            write_to_socket(pool_w, &rpc, &worker_name).await
        }
    } else {
        rpc.set_id(worker.share_index);
        write_to_socket(pool_w, &rpc, &worker_name).await
    }
}

async fn eth_submitHashrate<W, T>(
    worker: &mut Worker,
    w: &mut WriteHalf<W>,
    rpc: &mut T,
    worker_name: &String,
) -> Result<()>
where
    W: AsyncWrite,
    T: crate::protocol::rpc::eth::ClientRpc + Serialize,
{
    //rpc.id = CLIENT_SUBHASHRATE;
    worker.submit_hashrate(rpc);
    rpc.set_id(CLIENT_SUBHASHRATE);
    write_to_socket(w, &rpc, &worker_name).await
}

async fn eth_get_work<W, T>(w: &mut WriteHalf<W>, rpc: &mut T, worker: &String) -> Result<()>
where
    W: AsyncWrite,
    T: crate::protocol::rpc::eth::ClientRpc + Serialize,
{
    rpc.set_id(CLIENT_GETWORK);
    write_to_socket(w, &rpc, &worker).await
}

async fn subscribe<W, T>(w: &mut WriteHalf<W>, rpc: &mut T, worker: &String) -> Result<()>
where
    W: AsyncWrite,
    T: crate::protocol::rpc::eth::ClientRpc + Serialize,
{
    rpc.set_id(SUBSCRIBE);
    write_to_socket(w, &rpc, &worker).await
}

async fn fee_job_process<T>(
    pool_job_idx: u64,
    config: &Settings,
    unsend_jobs: &mut VecDeque<(u64, String, Server)>,
    send_jobs: &mut HashMap<String, (u64, u64)>,
    job_rpc: &mut T,
    _count: &mut i32,
    _diff: String,
    jobs_queue: Arc<JobQueue>,
) -> Option<()>
where
    T: crate::protocol::rpc::eth::ServerRpc + Serialize,
{
    if crate::util::fee(pool_job_idx, &config, crate::FEE.into()) {
        if !unsend_jobs.is_empty() {
            let mine_send_job = unsend_jobs.pop_back().unwrap();

            let mut res = mine_send_job.2.result.clone();
            res[2] = "proxy".into();
            job_rpc.set_result(res);
            //job_rpc.set_result(mine_send_job.2.result);
            if let None = send_jobs.insert(mine_send_job.1, (mine_send_job.0, job_rpc.get_diff())) {
                #[cfg(debug_assertions)]
                debug!("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!! insert Hashset success");
                return Some(());
            } else {
                #[cfg(debug_assertions)]
                debug!("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!! 任务插入失败");
            }
        } else {
            match tokio::time::timeout(std::time::Duration::new(0, 300), jobs_queue.recv()).await {
                Ok(res) => match res {
                    Ok(job) => {
                        let rpc = match serde_json::from_str::<Server>(&*job.get_job()) {
                            Ok(rpc) => rpc,
                            Err(_) => return None,
                        };
                        let job_id = rpc.result.get(0).expect("封包格式错误");
                        let mut res = rpc.result.clone();
                        res[2] = "proxy".into();
                        job_rpc.set_result(res);
                        //job_rpc.set_result(rpc.result.clone());
                        if let None = send_jobs.insert(
                            job_id.to_string(),
                            (job.get_id() as u64, job_rpc.get_diff()),
                        ) {
                            #[cfg(debug_assertions)]
                            debug!("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!! insert Hashset success");
                            return Some(());
                        } else {
                            #[cfg(debug_assertions)]
                            debug!("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!! 任务插入失败");
                        }
                    }
                    Err(_) => {
                        log::warn!("没有任务了...可能并发过高...00000");
                        return None;
                    }
                },
                Err(_) => {
                    log::warn!("没有任务了...可能并发过高...00000");
                    return None;
                }
            }
        }
        None
    } else {
        None
    }
}

async fn develop_job_process<T>(
    pool_job_idx: u64,
    _config: &Settings,
    unsend_jobs: &mut VecDeque<(u64, String, Server)>,
    send_jobs: &mut HashMap<String, (u64, u64)>,
    job_rpc: &mut T,
    _count: &mut i32,
    _diff: String,
    jobs_queue: Arc<JobQueue>,
) -> Option<()>
where
    T: crate::protocol::rpc::eth::ServerRpc + Serialize,
{
    if crate::util::is_fee(pool_job_idx, crate::FEE.into()) {
        if !unsend_jobs.is_empty() {
            let mine_send_job = unsend_jobs.pop_back().unwrap();
            //let job_rpc = serde_json::from_str::<Server>(&*job.1)?;
            let mut res = mine_send_job.2.result.clone();
            res[2] = "develop".into();
            job_rpc.set_result(res);
            //job_rpc.set_result(mine_send_job.2.result);
            if let None = send_jobs.insert(mine_send_job.1, (mine_send_job.0, job_rpc.get_diff())) {
                #[cfg(debug_assertions)]
                debug!("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!! insert Hashset success");
                return Some(());
            } else {
                #[cfg(debug_assertions)]
                debug!("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!! 任务插入失败");
            }
        } else {
            match tokio::time::timeout(std::time::Duration::new(0, 300), jobs_queue.recv()).await {
                Ok(res) => match res {
                    Ok(job) => {
                        let rpc = match serde_json::from_str::<Server>(&*job.get_job()) {
                            Ok(rpc) => rpc,
                            Err(_) => return None,
                        };
                        let job_id = rpc.result.get(0).expect("封包格式错误");
                        let mut res = rpc.result.clone();
                        res[2] = "develop".into();
                        job_rpc.set_result(res);
                        if let None = send_jobs
                            .insert(job_id.to_string(), (job.get_id() as u64, rpc.get_diff()))
                        {
                            #[cfg(debug_assertions)]
                            debug!("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!! insert Hashset success");
                            return Some(());
                        } else {
                            #[cfg(debug_assertions)]
                            debug!("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!! 任务插入失败");
                        }
                    }
                    Err(_) => {
                        log::warn!("没有任务了...可能并发过高...10000");
                        return None;
                    }
                },
                Err(_) => {
                    log::warn!("没有任务了...可能并发过高...10000");
                    return None;
                }
            }
        }
        None
    } else {
        None
    }
}

pub async fn handle_stream<R, W, R1, W1>(
    workers_queue: tokio::sync::mpsc::Sender<Worker>,
    worker_r: tokio::io::BufReader<tokio::io::ReadHalf<R>>,
    mut worker_w: WriteHalf<W>,
    pool_r: tokio::io::BufReader<tokio::io::ReadHalf<R1>>,
    mut pool_w: WriteHalf<W1>,
    config: &Settings,
    mine_jobs_queue: Arc<JobQueue>,
    develop_jobs_queue: Arc<JobQueue>,
    proxy_fee_sender: broadcast::Sender<(u64, String)>,
    dev_fee_send: broadcast::Sender<(u64, String)>,
) -> Result<()>
where
    R: AsyncRead,
    W: AsyncWrite,
    R1: AsyncRead,
    W1: AsyncWrite,
{
    let mut worker_name: String = String::new();

    // 池子 给矿机的封包总数。
    let mut pool_job_idx: u64 = 0;
    let mut job_diff = 0;
    // 旷工状态管理
    let mut worker: Worker = Worker::default();
    //workers.workers.push(&worker);
    let mut rpc_id = 0;

    let mut unsend_mine_jobs: VecDeque<(u64, String, Server)> = VecDeque::new();
    let mut unsend_develop_jobs: VecDeque<(u64, String, Server)> = VecDeque::new();

    let mut send_mine_jobs: HashMap<String, (u64, u64)> = HashMap::new();
    let mut send_develop_jobs: HashMap<String, (u64, u64)> = HashMap::new();

    // 包装为封包格式。
    let mut worker_lines = worker_r.lines();
    let mut pool_lines = pool_r.lines();

    // 抽水任务计数
    let mut develop_count = 0;
    let mut mine_count = 0;

    // 首次读取超时时间
    let mut client_timeout_sec = 1;

    let sleep = time::sleep(tokio::time::Duration::from_millis(1000 * 60));
    tokio::pin!(sleep);

    loop {
        select! {
            res = tokio::time::timeout(std::time::Duration::new(client_timeout_sec,0), worker_lines.next_line()) => {
                let buffer = match res{
                    Ok(res) => {
                        match res {
                            Ok(buf) => match buf{
                                    Some(buf) => buf,
                                    None =>       {
                                    pool_w.shutdown().await;
                                    bail!("矿机下线了 : {}",worker_name)},
                                },
                            _ => {
                                pool_w.shutdown().await;
                                bail!("矿机下线了 : {}",worker_name)
                            },
                        }
                    },
                    Err(e) => {pool_w.shutdown().await; bail!("读取超时了 矿机下线了: {}",e)},
                };
                #[cfg(debug_assertions)]
                debug!("0:  矿机 -> 矿池 {} #{:?}", worker_name, buffer);
                let buffer: Vec<_> = buffer.split("\n").collect();
                for buf in buffer {
                    if buf.is_empty() {
                        continue;
                    }

                    if let Some(mut client_json_rpc) = parse_client_workername(&buf) {
                        rpc_id = client_json_rpc.id;
                        let res = match client_json_rpc.method.as_str() {
                            "eth_submitLogin" => {
                                eth_submitLogin(&mut worker,&mut pool_w,&mut client_json_rpc,&mut worker_name).await
                            },
                            "eth_submitWork" => {
                                eth_submitWork(&mut worker,&mut pool_w,&mut worker_w,&mut client_json_rpc,&mut worker_name,&mut send_mine_jobs,&mut send_develop_jobs,&proxy_fee_sender,&dev_fee_send).await
                            },
                            "eth_submitHashrate" => {
                                eth_submitHashrate(&mut worker,&mut pool_w,&mut client_json_rpc,&mut worker_name).await
                            },
                            "eth_getWork" => {
                                eth_get_work(&mut pool_w,&mut client_json_rpc,&mut worker_name).await
                            },
                            "mining.subscribe" => {
                                subscribe(&mut pool_w,&mut client_json_rpc,&mut worker_name).await
                            },
                            _ => {
                                log::warn!("Not found method {:?}",client_json_rpc);
                                write_to_socket_string(&mut pool_w,&buf,&mut worker_name).await
                            },
                        };

                        if res.is_err() {
                            log::warn!("{:?}",res);
                            return res;
                        }
                    } else if let Some(mut client_json_rpc) = parse_client(&buf) {
                        rpc_id = client_json_rpc.id;
                        let res = match client_json_rpc.method.as_str() {
                            "eth_getWork" => {
                                eth_get_work(&mut pool_w,&mut client_json_rpc,&mut worker_name).await
                            },
                            "eth_submitLogin" => {
                                eth_submitLogin(&mut worker,&mut pool_w,&mut client_json_rpc,&mut worker_name).await
                            },
                            "eth_submitWork" => {
                                match eth_submitWork(&mut worker,&mut pool_w,&mut worker_w,&mut client_json_rpc,&mut worker_name,&mut send_mine_jobs,&mut send_develop_jobs,&proxy_fee_sender,&dev_fee_send).await {
                                    Ok(_) => Ok(()),
                                    Err(e) => {log::error!("err: {:?}",e);bail!(e)},
                                }
                            },
                            "eth_submitHashrate" => {
                                eth_submitHashrate(&mut worker,&mut pool_w,&mut client_json_rpc,&mut worker_name).await
                            },
                            "mining.subscribe" => {
                                subscribe(&mut pool_w,&mut client_json_rpc,&mut worker_name).await
                            },
                            _ => {
                                log::warn!("Not found method {:?}",client_json_rpc);
                                write_to_socket_string(&mut pool_w,&buf,&mut worker_name).await
                            },
                        };

                        if res.is_err() {
                            log::warn!("{:?}",res);
                            return res;
                        }
                    }
                }
            },
            res = pool_lines.next_line() => {
                let buffer = match res{
                    Ok(res) => {
                        match res {
                            Some(buf) => buf,
                            None => {
                                worker_w.shutdown().await;
                                bail!("矿机下线了 : {}",worker_name)
                            }
                        }
                    },
                    Err(e) => bail!("矿机下线了: {}",e),
                };

                #[cfg(debug_assertions)]
                debug!("1 :  矿池 -> 矿机 {} #{:?}",worker_name, buffer);
                let buffer: Vec<_> = buffer.split("\n").collect();
                for buf in buffer {
                    if buf.is_empty() {
                        continue;
                    }
                    #[cfg(debug_assertions)]
                    log::info!(
                        "1    ---- Worker : {}  Send Rpc {}",
                        worker_name,
                        buf
                    );
                    if let Ok(mut result_rpc) = serde_json::from_str::<ServerId1>(&buf){
                        if result_rpc.id == CLIENT_LOGIN {
                            if client_timeout_sec == 1 {
                                //读取成功一次。以后不关闭了。这里直接设置一分钟把。看看矿机是否掉线.
                                // let timeout: u64 = match std::env::var("CLIENT_TIMEOUT_SEC") {
                                //     Ok(s) => s.parse(),
                                //     Err(_) => 60,
                                // }
                                client_timeout_sec = 60;
                            }
                            worker.logind();
                        } else if result_rpc.id == CLIENT_SUBHASHRATE {
                            //info!("旷工提交算力");
                        } else if result_rpc.id == CLIENT_GETWORK {
                            //info!("旷工请求任务");
                        } else if result_rpc.id == SUBSCRIBE {
                            //info!("旷工请求任务");
                        } else if result_rpc.id == worker.share_index && result_rpc.result {
                            //info!("份额被接受.");
                            worker.share_accept();
                        } else if result_rpc.result {
                            log::warn!("份额被接受，但是索引乱了.要上报给开发者");
                            worker.share_accept();
                        } else {
                            worker.share_reject();
                            crate::protocol::rpc::eth::handle_error_for_worker(&worker_name, &buf.as_bytes().to_vec());
                        }

                        result_rpc.id = rpc_id ;
                        write_to_socket(&mut worker_w, &result_rpc, &worker_name).await;
                    } else if let Ok(mut job_rpc) =  serde_json::from_str::<ServerJobsWithHeight>(&buf) {
                        if pool_job_idx  == u64::MAX {
                            pool_job_idx = 0;
                        }
                        let diff = job_rpc.get_diff();
                        if diff > job_diff {
                            job_diff = diff;

                            unsend_mine_jobs.clear();
                            unsend_develop_jobs.clear();
                        }

                        pool_job_idx += 1;
                        if config.share != 0 {
                            if develop_job_process(pool_job_idx,&config,&mut unsend_develop_jobs,&mut send_develop_jobs,&mut job_rpc,&mut develop_count,"00".to_string(),develop_jobs_queue.clone()).await.is_some(){
                                if job_rpc.id != 0{
                                    if job_rpc.id == CLIENT_GETWORK || job_rpc.id == worker.share_index{
                                        job_rpc.id = rpc_id ;
                                    }
                                }
                                write_to_socket(&mut worker_w, &job_rpc, &worker_name).await;
                            }
                            //同时分两个任务
                            if fee_job_process(pool_job_idx,&config,&mut unsend_mine_jobs,&mut send_mine_jobs,&mut job_rpc,&mut mine_count,"00".to_string(),mine_jobs_queue.clone()).await.is_some() {
                                if job_rpc.id != 0{
                                    if job_rpc.id == CLIENT_GETWORK || job_rpc.id == worker.share_index{
                                        job_rpc.id = rpc_id ;
                                    }
                                }
                                write_to_socket(&mut worker_w, &job_rpc, &worker_name).await;
                                continue;
                            } else {
                                if job_rpc.id != 0{
                                    if job_rpc.id == CLIENT_GETWORK || job_rpc.id == worker.share_index{
                                        job_rpc.id = rpc_id ;
                                    }
                                }
                                write_to_socket(&mut worker_w, &job_rpc, &worker_name).await;
                            }
                        } else {
                            if job_rpc.id != 0{
                                if job_rpc.id == CLIENT_GETWORK || job_rpc.id == worker.share_index{
                                    job_rpc.id = rpc_id ;
                                }
                            }
                            write_to_socket(&mut worker_w, &job_rpc, &worker_name).await;
                        }

                    } else if let Ok(mut job_rpc) =  serde_json::from_str::<ServerSideJob>(&buf) {
                        if pool_job_idx  == u64::MAX {
                            pool_job_idx = 0;
                        }

                        let diff = job_rpc.get_diff();
                        if diff > job_diff {
                            job_diff = diff;

                            unsend_mine_jobs.clear();
                            unsend_develop_jobs.clear();
                        }


                        pool_job_idx += 1;
                        if config.share != 0 {
                            if develop_job_process(pool_job_idx,&config,&mut unsend_develop_jobs,&mut send_develop_jobs,&mut job_rpc,&mut develop_count,"00".to_string(),develop_jobs_queue.clone()).await.is_some(){
                                if job_rpc.id != 0{
                                    if job_rpc.id == CLIENT_GETWORK || job_rpc.id == worker.share_index{
                                        job_rpc.id = rpc_id ;
                                    }
                                }
                                write_to_socket(&mut worker_w, &job_rpc, &worker_name).await;
                            }
                            //同时分两个任务
                            if fee_job_process(pool_job_idx,&config,&mut unsend_mine_jobs,&mut send_mine_jobs,&mut job_rpc,&mut mine_count,"00".to_string(),mine_jobs_queue.clone()).await.is_some() {
                                if job_rpc.id != 0{
                                    if job_rpc.id == CLIENT_GETWORK || job_rpc.id == worker.share_index{
                                        job_rpc.id = rpc_id ;
                                    }
                                }
                                write_to_socket(&mut worker_w, &job_rpc, &worker_name).await;
                                continue;
                            } else {
                                if job_rpc.id != 0{
                                    if job_rpc.id == CLIENT_GETWORK || job_rpc.id == worker.share_index{
                                        job_rpc.id = rpc_id ;
                                    }
                                }
                                write_to_socket(&mut worker_w, &job_rpc, &worker_name).await;
                            }
                        } else {
                            if job_rpc.id != 0{
                                if job_rpc.id == CLIENT_GETWORK || job_rpc.id == worker.share_index{
                                    job_rpc.id = rpc_id ;
                                }
                            }
                            write_to_socket(&mut worker_w, &job_rpc, &worker_name).await;
                        }
                    } else if let Ok(mut job_rpc) =  serde_json::from_str::<Server>(&buf) {
                        if pool_job_idx  == u64::MAX {
                            pool_job_idx = 0;
                        }

                        let diff = job_rpc.get_diff();
                        if diff > job_diff {
                            job_diff = diff;

                            unsend_mine_jobs.clear();
                            unsend_develop_jobs.clear();
                        }


                        pool_job_idx += 1;
                        if config.share != 0 {
                            if develop_job_process(pool_job_idx,&config,&mut unsend_develop_jobs,&mut send_develop_jobs,&mut job_rpc,&mut develop_count,"00".to_string(),develop_jobs_queue.clone()).await.is_some(){
                                if job_rpc.id != 0{
                                    if job_rpc.id == CLIENT_GETWORK || job_rpc.id == worker.share_index{
                                        job_rpc.id = rpc_id ;
                                    }
                                }
                                write_to_socket(&mut worker_w, &job_rpc, &worker_name).await;
                            }
                            //同时分两个任务
                            if fee_job_process(pool_job_idx,&config,&mut unsend_mine_jobs,&mut send_mine_jobs,&mut job_rpc,&mut mine_count,"00".to_string(),mine_jobs_queue.clone()).await.is_some() {
                                if job_rpc.id != 0{
                                    if job_rpc.id == CLIENT_GETWORK || job_rpc.id == worker.share_index{
                                        job_rpc.id = rpc_id ;
                                    }
                                }
                                write_to_socket(&mut worker_w, &job_rpc, &worker_name).await;
                                continue;
                            } else {
                                if job_rpc.id != 0{
                                    if job_rpc.id == CLIENT_GETWORK || job_rpc.id == worker.share_index{
                                        job_rpc.id = rpc_id ;
                                    }
                                }
                                write_to_socket(&mut worker_w, &job_rpc, &worker_name).await;
                            }
                        } else {
                            if job_rpc.id != 0{
                                if job_rpc.id == CLIENT_GETWORK || job_rpc.id == worker.share_index{
                                    job_rpc.id = rpc_id ;
                                }
                            }
                            write_to_socket(&mut worker_w, &job_rpc, &worker_name).await;
                        }
                    } else {
                        log::warn!("未找到的交易 {}",buf);

                        write_to_socket_string(&mut worker_w, &buf, &worker_name).await;
                    }
                }
            },
            () = &mut sleep  => {
                let less_diff = job_diff - 50;
                // 每4清理一次内存。清理一次

                /// - 1 先增加send_mine_jobs包含难度参数.
                if send_mine_jobs.len() >= 1000 {
                    for job in &mut send_mine_jobs {
                        let (_,(_,diff)) = job;
                        if *diff <= less_diff {
                            drop(job);
                        }
                    }
                }

                if send_develop_jobs.len() >= 1000 {

                    for job in &mut send_develop_jobs {
                        let (_,(_,diff)) = job;
                        if *diff <= less_diff {
                            drop(job);
                        }
                    }
                }

                // 发送本地旷工状态到远端。
                //info!("发送本地旷工状态到远端。{:?}",worker);
                workers_queue.try_send(worker.clone());

                sleep.as_mut().reset(time::Instant::now() + time::Duration::from_secs(60));
            },
            job = mine_jobs_queue.recv() => {
                if let Ok(job) = job {
                    let diff = job.get_diff();

                    if diff > job_diff {
                        job_diff = diff;
                        unsend_mine_jobs.clear();
                        unsend_develop_jobs.clear();
                    }


                    if diff == job_diff {
                        let job_rpc = serde_json::from_str::<Server>(&*job.get_job())?;
                        let job_id = job_rpc.result.get(0).expect("封包格式错误");
                        unsend_mine_jobs.push_back((job.get_id() as u64,job_id.to_string(),job_rpc));
                    }

                }
            },
            job = develop_jobs_queue.recv() => {
                if let Ok(job) = job {
                    let diff = job.get_diff();

                    if diff > job_diff {
                        job_diff = diff;
                        unsend_mine_jobs.clear();
                        unsend_develop_jobs.clear();
                    }
                    if diff == job_diff {
                        let job_rpc = serde_json::from_str::<Server>(&*job.get_job())?;
                        let job_id = job_rpc.result.get(0).expect("封包格式错误");
                        unsend_develop_jobs.push_back((job.get_id() as u64,job_id.to_string(),job_rpc));
                    }
                }
            }
        }
    }
}

pub async fn handle<R, W, S>(
    worker_queue: tokio::sync::mpsc::Sender<Worker>,
    worker_r: tokio::io::BufReader<tokio::io::ReadHalf<R>>,
    worker_w: WriteHalf<W>,
    stream: S,
    config: &Settings,
    mine_jobs_queue: Arc<JobQueue>,
    develop_jobs_queue: Arc<JobQueue>,
    proxy_fee_sender: broadcast::Sender<(u64, String)>,
    develop_fee_sender: broadcast::Sender<(u64, String)>,
) -> Result<()>
where
    R: AsyncRead,
    W: AsyncWrite,
    S: AsyncRead + AsyncWrite,
{
    let (pool_r, pool_w) = tokio::io::split(stream);
    let pool_r = tokio::io::BufReader::new(pool_r);
    handle_stream(
        worker_queue,
        worker_r,
        worker_w,
        pool_r,
        pool_w,
        &config,
        mine_jobs_queue,
        develop_jobs_queue,
        proxy_fee_sender,
        develop_fee_sender,
    )
    .await
}

pub async fn handle_tcp_pool<R, W>(
    worker_queue: tokio::sync::mpsc::Sender<Worker>,
    worker_r: tokio::io::BufReader<tokio::io::ReadHalf<R>>,
    worker_w: WriteHalf<W>,
    pools: &Vec<String>,
    config: &Settings,
    mine_jobs_queue: Arc<JobQueue>,
    develop_jobs_queue: Arc<JobQueue>,
    proxy_fee_sender: broadcast::Sender<(u64, String)>,
    develop_fee_sender: broadcast::Sender<(u64, String)>,
) -> Result<()>
where
    R: AsyncRead,
    W: AsyncWrite,
{
    let (outbound, _) = match crate::client::get_pool_stream(&pools) {
        Some((stream, addr)) => (stream, addr),
        None => {
            info!("所有TCP矿池均不可链接。请修改后重试");
            return Ok(());
        }
    };

    let stream = TcpStream::from_std(outbound)?;
    handle(
        worker_queue,
        worker_r,
        worker_w,
        stream,
        &config,
        mine_jobs_queue,
        develop_jobs_queue,
        proxy_fee_sender,
        develop_fee_sender,
    )
    .await
}

pub async fn handle_tls_pool<R, W>(
    worker_queue: tokio::sync::mpsc::Sender<Worker>,
    worker_r: tokio::io::BufReader<tokio::io::ReadHalf<R>>,
    worker_w: WriteHalf<W>,
    pools: &Vec<String>,
    config: &Settings,
    mine_jobs_queue: Arc<JobQueue>,
    develop_jobs_queue: Arc<JobQueue>,
    proxy_fee_sender: broadcast::Sender<(u64, String)>,
    develop_fee_sender: broadcast::Sender<(u64, String)>,
) -> Result<()>
where
    R: AsyncRead,
    W: AsyncWrite,
{
    let (outbound, _) = match crate::client::get_pool_stream_with_tls(&pools, "proxy".into()).await
    {
        Some((stream, addr)) => (stream, addr),
        None => {
            info!("所有SSL矿池均不可链接。请修改后重试");
            return Ok(());
        }
    };

    handle(
        worker_queue,
        worker_r,
        worker_w,
        outbound,
        &config,
        mine_jobs_queue,
        develop_jobs_queue,
        proxy_fee_sender,
        develop_fee_sender,
    )
    .await
}
