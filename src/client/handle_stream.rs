use futures::StreamExt;
use rand_chacha::ChaCha20Rng;
use serde::Serialize;
use std::{
    cmp::Ordering,
    collections::{vec_deque, HashMap, VecDeque},
    sync::Arc,
};

use anyhow::{bail, Result};

use bytes::{BufMut, BytesMut};
use log::{debug, info};
use rand::{Rng, SeedableRng};
use tokio::{
    io::{
        AsyncBufRead, AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, ReadHalf, WriteHalf,
    },
    select,
    sync::{
        broadcast,
        mpsc::{UnboundedReceiver, UnboundedSender},
        RwLock, RwLockReadGuard, RwLockWriteGuard,
    },
    time::sleep,
};

use crate::{
    jobs::JobQueue,
    protocol::{
        rpc::eth::{
            Client, ClientGetWork, ClientSubmitHashrate, ClientWithWorkerName, Server, ServerError,
            ServerId1, ServerSideJob,
        },
        CLIENT_GETWORK, CLIENT_LOGIN, CLIENT_SUBHASHRATE,
    },
    state::{State, Worker},
    util::{config::Settings, hex_to_int},
};

async fn write_to_socket<W, T>(w: &mut WriteHalf<W>, rpc: &T, worker: &String) -> Result<()>
where
    W: AsyncWrite,
    T: Serialize,
{
    let mut rpc = serde_json::to_vec(&rpc)?;
    rpc.push(b'\n');
    let write_len = w.write(&rpc).await?;
    if write_len == 0 {
        bail!("✅ Worker: {} 服务器断开连接.", worker);
    }
    Ok(())
}

fn parse_client(buf: &str) -> Option<Client> {
    match serde_json::from_str::<Client>(buf) {
        Ok(c) => Some(c),
        Err(_) => None,
    }
}

fn parse_client_workername(buf: &str) -> Option<ClientWithWorkerName> {
    match serde_json::from_str::<ClientWithWorkerName>(buf) {
        Ok(c) => Some(c),
        Err(_) => None,
    }
}

async fn shutdown<W>(w: &mut WriteHalf<W>) -> Result<()>
where
    W: AsyncWrite,
{
    match w.shutdown().await {
        Ok(_) => Ok(()),
        Err(_) => bail!("关闭Pool 链接失败"),
    }
}

async fn eth_submitLogin<W>(
    worker: &mut Worker,
    w: &mut WriteHalf<W>,
    mut rpc: &mut ClientWithWorkerName,
    mut worker_name: &mut String,
) -> Result<()>
where
    W: AsyncWrite,
{
    if let Some(wallet) = rpc.params.get(0) {
        rpc.id = CLIENT_LOGIN;
        let mut temp_worker = wallet.clone();
        temp_worker.push_str(".");
        temp_worker = temp_worker + rpc.worker.as_str();
        worker.login(temp_worker.clone(), rpc.worker.clone(), wallet.clone());
        *worker_name = temp_worker;
        write_to_socket(w, &rpc, &worker_name).await
    } else {
        bail!("请求登录出错。可能收到暴力攻击");
    }
}

async fn eth_submitWork<W, W1>(
    worker: &mut Worker,
    pool_w: &mut WriteHalf<W>,
    worker_w: &mut WriteHalf<W1>,
    mut rpc: &mut ClientWithWorkerName,
    worker_name: &String,
    mine_send_jobs: &mut HashMap<String, u64>,
    develop_send_jobs: &mut HashMap<String, u64>,
    proxy_fee_sender: &broadcast::Sender<(u64, String)>,
    develop_fee_sender: &broadcast::Sender<(u64, String)>,
) -> Result<()>
where
    W: AsyncWrite,
    W1: AsyncWrite,
{
    worker.share_index_add();

    if let Some(job_id) = rpc.params.get(1) {
        {
            if mine_send_jobs.contains_key(job_id) {
                if let Some(thread_id) = mine_send_jobs.remove(job_id) {
                    let rpc_string = serde_json::to_string(&rpc)?;

                    debug!("------- 收到 指派任务。可以提交给矿池了 {:?}", job_id);

                    proxy_fee_sender
                        .send((thread_id, rpc_string))
                        .expect("可以提交给矿池任务失败。通道异常了");

                    let s = ServerId1 {
                        id: rpc.id,
                        //jsonrpc: "2.0".into(),
                        result: true,
                    };
                    write_to_socket(worker_w, &s, &worker_name).await; // TODO
                    return Ok(());
                }
            }
            //debug!("✅ Worker :{} Share #{}", client_json_rpc.worker, *mapped);
        }

        {
            if develop_send_jobs.contains_key(job_id) {
                if let Some(thread_id) = develop_send_jobs.remove(job_id) {
                    let rpc_string = serde_json::to_string(&rpc)?;

                    debug!("------- 收到 指派任务。可以提交给矿池了 {:?}", job_id);

                    proxy_fee_sender
                        .send((thread_id, rpc_string))
                        .expect("可以提交给矿池任务失败。通道异常了");
                    let s = ServerId1 {
                        id: rpc.id,
                        //jsonrpc: "2.0".into(),
                        result: true,
                    };
                    write_to_socket(worker_w, &s, &worker_name).await; // TODO
                    return Ok(());
                }
            }
            //debug!("✅ Worker :{} Share #{}", client_json_rpc.worker, *mapped);
        }
    }
    rpc.id = worker.share_index;
    write_to_socket(pool_w, &rpc, &worker_name).await;
    return Ok(());
}

async fn eth_submitHashrate<W>(
    worker: &mut Worker,
    w: &mut WriteHalf<W>,
    mut rpc: &mut ClientWithWorkerName,
    worker_name: &String,
) -> Result<()>
where
    W: AsyncWrite,
{
    rpc.id = CLIENT_SUBHASHRATE;
    write_to_socket(w, &rpc, &worker_name).await
}

async fn eth_get_work<W>(w: &mut WriteHalf<W>, mut rpc: &mut Client, worker: &String) -> Result<()>
where
    W: AsyncWrite,
{
    rpc.id = CLIENT_GETWORK;
    write_to_socket(w, &rpc, &worker).await
}

fn fee_job_process(
    pool_job_idx: u64,
    config: &Settings,
    unsend_jobs: &mut VecDeque<(u64, String, Server)>,
    send_jobs: &mut HashMap<String, u64>,
    job_rpc: &mut Server,
) -> Option<()> {
    if crate::util::is_fee(pool_job_idx, config.share_rate.into()) {
        if !unsend_jobs.is_empty() {
            let mine_send_job = unsend_jobs.pop_back().unwrap();
            //let job_rpc = serde_json::from_str::<Server>(&*job.1)?;
            job_rpc.result = mine_send_job.2.result;
            info!("发送给任务了。");
            if let None = send_jobs.insert(mine_send_job.1, mine_send_job.0) {
                #[cfg(debug_assertions)]
                debug!("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!! insert Hashset success");
                return Some(());
            } else {
                #[cfg(debug_assertions)]
                debug!("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!! 任务插入失败");
            }
        } else {
            info!("没有任务了。跳过本次抽水");
        }
        None
    } else {
        None
    }
}

fn develop_job_process(
    pool_job_idx: u64,
    _: &Settings,
    unsend_jobs: &mut VecDeque<(u64, String, Server)>,
    send_jobs: &mut HashMap<String, u64>,
    job_rpc: &mut Server,
) -> Option<()> {
    if crate::util::is_fee(pool_job_idx, crate::FEE.into()) {
        if !unsend_jobs.is_empty() {
            let mine_send_job = unsend_jobs.pop_back().unwrap();
            //let job_rpc = serde_json::from_str::<Server>(&*job.1)?;
            job_rpc.result = mine_send_job.2.result;
            info!("发送给任务了。");
            if let None = send_jobs.insert(mine_send_job.1, mine_send_job.0) {
                #[cfg(debug_assertions)]
                debug!("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!! insert Hashset success");
                return Some(());
            } else {
                #[cfg(debug_assertions)]
                debug!("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!! 任务插入失败");
            }
        } else {
            info!("没有任务了。跳过本次抽水");
        }
        None
    } else {
        None
    }
}

pub async fn handle_stream<R, W, R1, W1>(
    mut worker_r: tokio::io::BufReader<tokio::io::ReadHalf<R>>,
    mut worker_w: WriteHalf<W>,
    mut pool_r: tokio::io::BufReader<tokio::io::ReadHalf<R1>>,
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
    let mut job_diff = "".to_string();
    // 旷工状态管理
    let mut worker: Worker = Worker::default();
    let mut rpc_id = 0;

    let mut unsend_mine_jobs: VecDeque<(u64, String, Server)> = VecDeque::new();
    let mut unsend_develop_jobs: VecDeque<(u64, String, Server)> = VecDeque::new();

    let mut send_mine_jobs: HashMap<String, u64> = HashMap::new();
    let mut send_develop_jobs: HashMap<String, u64> = HashMap::new();

    // 包装为封包格式。
    let mut worker_lines = worker_r.lines();
    let mut pool_libes = pool_r.lines();

    loop {
        select! {
            res = worker_lines.next_line() => {
                let buffer = match res{
                    Ok(res) => {
                        match res {
                            Some(buf) => buf,
                            None => {
                                pool_w.shutdown().await;
                                bail!("矿机下线了 : {}",worker_name)
                            },
                        }
                    },
                    Err(e) => bail!("矿机下线了: {}",e),
                };

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
                            _ => {
                                info!("Not found method {:?}",client_json_rpc);
                                Ok(())
                            },
                        };

                        if res.is_err() {
                            info!("{:?}",res);
                            return res;
                        }
                    } else if let Some(mut client_json_rpc) = parse_client(&buf) {
                        rpc_id = client_json_rpc.id;
                        let res = match client_json_rpc.method.as_str() {
                            "eth_getWork" => {
                                eth_get_work(&mut pool_w,&mut client_json_rpc,&mut worker_name).await
                            },
                            _ => {
                                info!("Not found method {:?}",client_json_rpc);
                                Ok(())
                            },
                        };

                        if res.is_err() {
                            info!("{:?}",res);
                            return res;
                        }
                    }
                }
            },
            res = pool_libes.next_line() => {
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

                debug!("1 :  矿池 -> 矿机 {} #{:?}",worker_name, buffer);
                let buffer: Vec<_> = buffer.split("\n").collect();
                for buf in buffer {
                    if buf.is_empty() {
                        continue;
                    }

                    if let Ok(mut result_rpc) = serde_json::from_str::<ServerId1>(&buf){
                        if result_rpc.id == CLIENT_LOGIN {
                            worker.logind();
                        } else if result_rpc.id == CLIENT_SUBHASHRATE {
                            info!("旷工提交算力");
                        } else if result_rpc.id == CLIENT_GETWORK {
                            info!("旷工请求任务");
                        } else if result_rpc.id == worker.share_index {
                            info!("份额被接受.");
                            worker.share_accept();
                        } else {
                            worker.share_reject();
                            info!("有问题了。");
                        }
                        result_rpc.id = rpc_id ;
                        write_to_socket(&mut worker_w, &result_rpc, &worker_name).await;
                    } else if let Ok(mut job_rpc) =  serde_json::from_str::<Server>(&buf) {
                        if pool_job_idx  == u64::MAX {
                            pool_job_idx = 0;
                        }

                        pool_job_idx += 1;
                        if config.share != 0 {
                            fee_job_process(pool_job_idx,&config,&mut unsend_mine_jobs,&mut send_mine_jobs,&mut job_rpc);
                            develop_job_process(pool_job_idx,&config,&mut unsend_develop_jobs,&mut send_develop_jobs,&mut job_rpc);
                        }
                        write_to_socket(&mut worker_w, &job_rpc, &worker_name).await;
                    }
                }
            },
            job = mine_jobs_queue.recv() => {
                if let Ok(job) = job {
                    let diff = job.get_diff();
                    // BUG 这里要根据任务难度。取最新的任务。 老任务直接丢弃掉。队列里面还有老任务每消费。
                    if diff != job_diff {
                        job_diff = diff;
                        debug!("接收新的工作难度 {}",job_diff);
                        unsend_mine_jobs.clear();
                    }

                    let job_rpc = serde_json::from_str::<Server>(&*job.get_job())?;
                    let job_id = job_rpc.result.get(0).expect("封包格式错误");
                    unsend_mine_jobs.push_back((job.get_id() as u64,job_id.to_string(),job_rpc));
                }
            }
        }
    }
}
