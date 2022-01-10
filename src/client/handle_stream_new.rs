use anyhow::{bail, Result};

use hex::FromHex;
use log::{debug, info};

use openssl::symm::{decrypt, Cipher};
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, WriteHalf},
    net::TcpStream,
    select, time,
};

use crate::{
    client::*,
    protocol::{
        ethjson::{EthServerRoot, EthServerRootObjectBool, EthServerRootObjectError},
        rpc::eth::{Server, ServerId1, ServerJobsWithHeight, ServerRootErrorValue, ServerSideJob},
        CLIENT_GETWORK, CLIENT_LOGIN, CLIENT_SUBHASHRATE, SUBSCRIBE,
    },
    state::Worker,
    util::{config::Settings, get_wallet},
};

use super::write_to_socket;

async fn new_eth_submit_login<W>(
    worker: &mut Worker,
    w: &mut WriteHalf<W>,
    rpc: &mut Box<dyn EthClientObject + Send + Sync>,
    worker_name: &mut String,
) -> Result<()>
where
    W: AsyncWrite,
{
    if let Some(wallet) = rpc.get_wallet() {
        rpc.set_id(CLIENT_LOGIN);
        let mut temp_worker = wallet.clone();
        temp_worker.push_str(".");
        temp_worker = temp_worker + rpc.get_worker_name().as_str();
        worker.login(temp_worker.clone(), rpc.get_worker_name(), wallet.clone());
        *worker_name = temp_worker;
        write_to_socket_byte(w, rpc.to_vec()?, &worker_name).await
    } else {
        bail!("请求登录出错。可能收到暴力攻击");
    }
}

async fn new_eth_submit_work<W, W1, W2>(
    worker: &mut Worker,
    pool_w: &mut WriteHalf<W>,
    proxy_w: &mut WriteHalf<W1>,
    develop_w: &mut WriteHalf<W1>,
    worker_w: &mut WriteHalf<W2>,
    rpc: &mut Box<dyn EthClientObject + Send + Sync>,
    worker_name: &String,
    mine_send_jobs: &mut Vec<String>,
    develop_send_jobs: &mut Vec<String>,
    config: &Settings,
    state: &mut State,
) -> Result<()>
where
    W: AsyncWrite,
    W1: AsyncWrite,
    W2: AsyncWrite,
{
    if let Some(job_id) = rpc.get_job_id() {
        #[cfg(debug_assertions)]
        debug!("提交的JobID {}", job_id);
        if mine_send_jobs.contains(&job_id) {
            //if let Some(_thread_id) = mine_send_jobs.get(&job_id) {
            let hostname = config.get_share_name().unwrap();
            state
                .proxy_share
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            rpc.set_worker_name(&hostname);
            #[cfg(debug_assertions)]
            debug!("得到抽水任务。{:?}", rpc);

            write_to_socket_byte(proxy_w, rpc.to_vec()?, &config.share_name).await?;
            return Ok(());
        } else if develop_send_jobs.contains(&job_id) {
            //if let Some(_thread_id) = develop_send_jobs.get(&job_id) {
            let mut hostname = String::from("develop_");
            state
                .develop_share
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

            let name = hostname::get()?;
            hostname += name.to_str().unwrap();
            rpc.set_worker_name(&hostname);
            #[cfg(debug_assertions)]
            debug!("得到开发者抽水任务。{:?}", rpc);
            write_to_socket_byte(develop_w, rpc.to_vec()?, &config.share_name).await?;
            return Ok(());
        } else {
            worker.share_index_add();
            rpc.set_id(worker.share_index);
            write_to_socket_byte(proxy_w, rpc.to_vec()?, &worker_name).await
        }
    } else {
        worker.share_index_add();
        rpc.set_id(worker.share_index);
        write_to_socket_byte(proxy_w, rpc.to_vec()?, &worker_name).await
    }
}

async fn new_eth_submit_hashrate<W>(
    worker: &mut Worker,
    w: &mut WriteHalf<W>,
    rpc: &mut Box<dyn EthClientObject + Send + Sync>,
    worker_name: &String,
) -> Result<()>
where
    W: AsyncWrite,
{
    worker.new_submit_hashrate(rpc);
    rpc.set_id(CLIENT_SUBHASHRATE);
    write_to_socket_byte(w, rpc.to_vec()?, &worker_name).await
}

async fn new_eth_get_work<W>(
    w: &mut WriteHalf<W>,
    rpc: &mut Box<dyn EthClientObject + Send + Sync>,
    worker_name: &String,
) -> Result<()>
where
    W: AsyncWrite,
{
    rpc.set_id(CLIENT_GETWORK);
    write_to_socket_byte(w, rpc.to_vec()?, &worker_name).await
}

async fn new_subscribe<W>(
    w: &mut WriteHalf<W>,
    rpc: &mut Box<dyn EthClientObject + Send + Sync>,
    worker_name: &String,
) -> Result<()>
where
    W: AsyncWrite,
{
    rpc.set_id(SUBSCRIBE);
    write_to_socket_byte(w, rpc.to_vec()?, &worker_name).await
}

pub async fn handle_stream<R, W, R1, W1>(
    worker: &mut Worker,
    workers_queue: UnboundedSender<Worker>,
    worker_r: tokio::io::BufReader<tokio::io::ReadHalf<R>>,
    mut worker_w: WriteHalf<W>,
    pool_r: tokio::io::BufReader<tokio::io::ReadHalf<R1>>,
    mut pool_w: WriteHalf<W1>,
    config: &Settings,
    mut state: State,
    is_encrypted: bool,
) -> Result<()>
where
    R: AsyncRead,
    W: AsyncWrite,
    R1: AsyncRead,
    W1: AsyncWrite,
{
    let mut worker_name: String = String::new();

    //TODO 这里要兼容SSL矿池
    let (stream, _) = match crate::client::get_pool_stream(&config.share_tcp_address) {
        Some((stream, addr)) => (stream, addr),
        None => {
            log::error!("所有TCP矿池均不可链接。请修改后重试");
            bail!("所有TCP矿池均不可链接。请修改后重试");
        }
    };

    let outbound = TcpStream::from_std(stream)?;
    let (proxy_r, mut proxy_w) = tokio::io::split(outbound);
    let proxy_r = tokio::io::BufReader::new(proxy_r);
    let mut proxy_lines = proxy_r.lines();

    let s = config.get_share_name().unwrap();

    let login = ClientWithWorkerName {
        id: CLIENT_LOGIN,
        method: "eth_submitLogin".into(),
        params: vec![config.share_wallet.clone(), "x".into()],
        worker: s.clone(),
    };

    match write_to_socket(&mut proxy_w, &login, &s).await {
        Ok(_) => {}
        Err(e) => {
            log::error!("Error writing Socket {:?}", login);
            return Err(e);
        }
    }

    let stream = match pools::get_develop_pool_stream().await {
        Ok(s) => s,
        Err(e) => {
            debug!("无法链接到矿池{}", e);
            return Err(e);
        }
    };

    let outbound = TcpStream::from_std(stream)?;

    let (develop_r, mut develop_w) = tokio::io::split(outbound);
    let develop_r = tokio::io::BufReader::new(develop_r);
    let mut develop_lines = develop_r.lines();

    let develop_name = s.clone() + "_develop";
    let login_develop = ClientWithWorkerName {
        id: CLIENT_LOGIN,
        method: "eth_submitLogin".into(),
        params: vec![get_wallet(), "x".into()],
        worker: develop_name.to_string(),
    };

    match write_to_socket(&mut develop_w, &login_develop, &develop_name).await {
        Ok(_) => {}
        Err(e) => {
            log::error!("Error writing Socket {:?}", login);
            return Err(e);
        }
    }

    // 池子 给矿机的封包总数。
    let mut pool_job_idx: u64 = 0;
    let mut job_diff = 0;

    let mut rpc_id = 0;

    let mut unsend_mine_jobs: VecDeque<(String, Vec<String>)> = VecDeque::new();
    let mut unsend_develop_jobs: VecDeque<(String, Vec<String>)> = VecDeque::new();
    let mut unsend_agent_jobs: VecDeque<(String, Vec<String>)> = VecDeque::new();

    let mut develop_count = 0;

    //TODO 完善精简这里的核心代码。加速任务分配。
    // let mut send_mine_jobs: LruCache<String, (u64, u64)> = LruCache::new(50);
    // let mut send_develop_jobs: LruCache<String, (u64, u64)> = LruCache::new(50);
    // let mut send_agent_jobs: LruCache<String, (u64, u64)> = LruCache::new(50);
    // let mut send_normal_jobs: LruCache<String, i32> = LruCache::new(100);

    let mut send_mine_jobs: Vec<String> = vec![];
    let mut send_develop_jobs: Vec<String> = vec![];
    let mut send_agent_jobs: Vec<String> = vec![];
    let mut send_normal_jobs: Vec<String> = vec![];

    // 包装为封包格式。
    // let mut worker_lines = worker_r.lines();
    let mut pool_lines = pool_r.lines();
    let mut worker_lines;
    if is_encrypted {
        worker_lines = worker_r.split(SPLIT);
    } else {
        worker_lines = worker_r.split(b'\n');
    }

    let mut is_submithashrate = false;

    let mut sleep_count: usize = 0;
    let sleep = time::sleep(tokio::time::Duration::from_secs(15));
    tokio::pin!(sleep);

    info!("旷工初始化用时{:?}", worker.login_time.elapsed());
    let mut loop_timer = std::time::Instant::now();
    let temp_worker = "Default".to_string();

    loop {
        select! {
            res = worker_lines.next_segment() => {
                let start = std::time::Instant::now();
                let buf_bytes = match res {
                    Ok(buf) => match buf {
                            Some(buf) => buf,
                            None => {
                                match pool_w.shutdown().await  {
                                    Ok(_) => {},
                                    Err(e) => {
                                        log::error!("Error Shutdown Socket {:?}",e);
                                    },
                                }

                                info!("矿工: {} 断开时---------- 接受任务并返回时间 {:?}",worker.worker_name,loop_timer.elapsed());
                                bail!("矿工：{}  读取到字节0.矿工主动断开 ",worker_name);
                            },
                        },
                    Err(e) => {
                        match pool_w.shutdown().await  {
                            Ok(_) => {},
                            Err(e) => {
                                log::error!("Error Shutdown Socket {:?}",e);
                            },
                        }
                        bail!("矿工：{} {}",worker_name,e);
                    },
                };

                loop_timer = std::time::Instant::now();

                #[cfg(debug_assertions)]
                debug!("0:  矿机 -> 矿池 {} #{:?}", worker_name, buf_bytes);
                let buf_bytes = buf_bytes.split(|c| *c == b'\n');
                for buffer in buf_bytes {
                    if buffer.is_empty() {
                        continue;
                    }

                    if let Some(mut json_rpc) = parse(&buffer) {
                        info!("接受矿工: {} 提交 RPC {:?}",worker.worker_name,json_rpc);

                        rpc_id = json_rpc.get_id();
                        let res = match json_rpc.get_method().as_str() {
                            "eth_submitLogin" => {
                                let result_rpc = &EthServerRoot{ id: rpc_id, jsonrpc: "2.0".into(), result: true};
                                new_eth_submit_login(worker,&mut pool_w,&mut json_rpc,&mut worker_name).await?;
                                write_to_socket(&mut worker_w, &result_rpc, &worker_name).await?;
                                Ok(())
                            },
                            "eth_submitWork" => {
                                let result_rpc = &EthServerRoot{ id: rpc_id, jsonrpc: "2.0".into(), result: true};

                                new_eth_submit_work(worker,&mut pool_w,&mut proxy_w,&mut develop_w,&mut worker_w,&mut json_rpc,&mut worker_name,&mut send_mine_jobs,&mut send_develop_jobs,&config,&mut state).await?;
                                write_to_socket(&mut worker_w, &result_rpc, &worker_name).await?;

                                Ok(())
                            },
                            "eth_submitHashrate" => {
                                let result_rpc = &EthServerRoot{ id: rpc_id, jsonrpc: "2.0".into(), result: true};
                                new_eth_submit_hashrate(worker,&mut pool_w,&mut json_rpc,&mut worker_name).await?;
                                write_to_socket(&mut worker_w, &result_rpc, &worker_name).await?;
                                Ok(())
                            },
                            "eth_getWork" => {
                                let result_rpc = &EthServerRootObjectError{ id: rpc_id, jsonrpc: "2.0".into(), result: true, error: String::from("")};
                                new_eth_get_work(&mut pool_w,&mut json_rpc,&mut worker_name).await?;
                                write_to_socket(&mut worker_w, &result_rpc, &worker_name).await?;
                                Ok(())
                            },
                            "mining.subscribe" => {
                                new_subscribe(&mut pool_w,&mut json_rpc,&mut worker_name).await?;
                                Ok(())
                            },
                            _ => {
                                log::warn!("Not found method {:?}",json_rpc);
                                write_to_socket_byte(&mut pool_w,buffer.to_vec(),&mut worker_name).await?;
                                Ok(())
                            },
                        };

                        if res.is_err() {
                            log::warn!("写入任务错误: {:?}",res);
                            return res;
                        }
                    } else {
                        let buf = match String::from_utf8(buffer.to_vec()) {
                            Ok(s) => s,
                            Err(_e) => {
                                log::warn!("无法解析的字符串{:?}",buffer);

                                match pool_w.shutdown().await  {
                                    Ok(_) => {},
                                    Err(e) => {
                                        log::error!("Error Shutdown Socket {:?}",e);
                                    },
                                };

                                return Ok(());
                            },
                        };
                        log::warn!("未知: {}",buf);
                    }
                }

                info!("接受矿工: {} 提交处理时间{:?}",worker.worker_name,start.elapsed());
            },
            res = pool_lines.next_line() => {
                let start = std::time::Instant::now();

                let buffer = match res{
                    Ok(res) => {
                        match res {
                            Some(buf) => buf,
                            None => {
                                match worker_w.shutdown().await {
                                    Ok(_) => {},
                                    Err(e) => {
                                        log::error!("Error Worker Shutdown Socket {:?}",e);
                                    },
                                };

                                bail!("矿工：{}  读取到字节0. 矿池主动断开 ",worker_name)
                            }
                        }
                    },
                    Err(e) => {
                        bail!("矿工：{}  读取到字节0. 矿池主动断开 ",worker_name)
                    },
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
                            worker.logind();
                            match workers_queue.send(worker.clone()){
                                Ok(_) => {},
                                Err(_) => {
                                    log::warn!("发送矿工状态失败");
                                },
                            };
                        } else if result_rpc.id == CLIENT_SUBHASHRATE {
                            //info!("矿工提交算力");
                            if !is_submithashrate {
                                match workers_queue.send(worker.clone()){
                                    Ok(_) => {},
                                    Err(_) => {
                                        log::warn!("发送矿工状态失败");
                                    },
                                };
                                is_submithashrate = true;
                            }
                        } else if result_rpc.id == CLIENT_GETWORK {
                            //info!("矿工请求任务");
                            continue;
                        } else if result_rpc.id == SUBSCRIBE {
                            //info!("矿工请求任务");
                        } else if result_rpc.id == worker.share_index && result_rpc.result {
                            //info!("份额被接受.");
                            worker.share_accept();
                        } else if result_rpc.result {
                            //log::warn!("份额被接受，但是索引乱了.要上报给开发者 {:?}",result_rpc);
                            worker.share_accept();
                        } else if result_rpc.id == worker.share_index {
                            worker.share_reject();
                            //log::warn!("拒绝原因 {}",buf);
                            //crate::protocol::rpc::eth::handle_error_for_worker(&worker_name, &buf.as_bytes().to_vec());
                            result_rpc.result = true;
                        }

                        result_rpc.id = rpc_id;
                        // if is_encrypted {
                        //     match write_encrypt_socket(&mut worker_w, &result_rpc, &worker_name,config.key.clone(),config.iv.clone()).await {
                        //         Ok(_) => {},
                        //         Err(e) => {
                        //             log::error!("Error Worker Write Socket {:?}",e);
                        //         },
                        //     };
                        // } else {
                        //     match write_to_socket(&mut worker_w, &result_rpc, &worker_name).await {
                        //         Ok(_) => {},
                        //         Err(e) => {
                        //             log::error!("Error Worker Write Socket {:?}",e);
                        //         },
                        //     };
                        // }

                    } else if let Ok(mut job_rpc) =  serde_json::from_str::<ServerJobsWithHeight>(&buf) {
                        pool_job_idx += 1;

                        if pool_job_idx  == u64::MAX {
                            pool_job_idx = 0;
                        }


                        job_diff_change(&mut job_diff,&job_rpc,&mut unsend_mine_jobs,&mut unsend_develop_jobs,&mut unsend_agent_jobs,&mut send_mine_jobs,&mut send_develop_jobs,&mut send_agent_jobs,&mut send_normal_jobs);

                        if job_rpc.id != 0{
                            if job_rpc.id == CLIENT_GETWORK || job_rpc.id == worker.share_index{
                                job_rpc.id = rpc_id ;
                            }
                        }


                        if config.share != 0 {
                            match share_job_process(pool_job_idx,&config,&mut unsend_develop_jobs,&mut unsend_mine_jobs,&mut unsend_agent_jobs,&mut send_develop_jobs,&mut send_agent_jobs,&mut send_mine_jobs,&mut send_normal_jobs,&mut job_rpc,&mut develop_count,&mut worker_w,&worker_name,worker,rpc_id,format!("0x{:x}",job_diff),is_encrypted).await {
                                Some(_) => {},
                                None => {
                                    log::error!("任务没有分配成功! at_count :{}",pool_job_idx);
                                },
                            };
                        } else {

                            if is_encrypted {
                                match write_encrypt_socket(&mut worker_w, &job_rpc, &worker_name,config.key.clone(),config.iv.clone()).await{
                                    Ok(_) => {},
                                    Err(e) => {bail!("矿机下线了 {}",e);},
                                };
                            } else {
                                match write_to_socket(&mut worker_w, &job_rpc, &worker_name).await{
                                    Ok(_) => {},
                                    Err(e) => {bail!("矿机下线了 {}",e);},
                                };
                            }
                        }

                    } else if let Ok(mut job_rpc) =  serde_json::from_str::<ServerSideJob>(&buf) {
                        if pool_job_idx  == u64::MAX {
                            pool_job_idx = 0;
                        }
                        if job_rpc.id != 0{
                            if job_rpc.id == CLIENT_GETWORK || job_rpc.id == worker.share_index {
                                job_rpc.id = rpc_id ;
                            }
                        }

                        job_diff_change(&mut job_diff,&job_rpc,&mut unsend_mine_jobs,&mut unsend_develop_jobs,&mut unsend_agent_jobs,&mut send_mine_jobs,&mut send_develop_jobs,&mut send_agent_jobs,&mut send_normal_jobs);

                        pool_job_idx += 1;
                        if config.share != 0 {
                            match share_job_process(pool_job_idx,&config,&mut unsend_develop_jobs,&mut unsend_mine_jobs,&mut unsend_agent_jobs,&mut send_develop_jobs,&mut send_agent_jobs,&mut send_mine_jobs,&mut send_normal_jobs,&mut job_rpc,&mut develop_count,&mut worker_w,&worker_name,worker,rpc_id,format!("0x{:x}",job_diff),is_encrypted).await {
                                Some(_) => {},
                                None => {
                                    log::error!("任务没有分配成功! at_count :{}",pool_job_idx);
                                },
                            };
                        } else {

                            if is_encrypted {
                                match write_encrypt_socket(&mut worker_w, &job_rpc, &worker_name,config.key.clone(),config.iv.clone()).await{
                                    Ok(_) => {},
                                    Err(e) => {bail!("矿机下线了 {}",e);},
                                };
                            } else {
                                match write_to_socket(&mut worker_w, &job_rpc, &worker_name).await{
                                    Ok(_) => {},
                                    Err(e) => {bail!("矿机下线了 {}",e);},
                                };
                            }
                        }
                    } else if let Ok(mut job_rpc) =  serde_json::from_str::<Server>(&buf) {
                        if pool_job_idx  == u64::MAX {
                            pool_job_idx = 0;
                        }


                        job_diff_change(&mut job_diff,&job_rpc,&mut unsend_mine_jobs,&mut unsend_develop_jobs,&mut unsend_agent_jobs,&mut send_mine_jobs,&mut send_develop_jobs,&mut send_agent_jobs,&mut send_normal_jobs);
                        if job_rpc.id != 0{
                            if job_rpc.id == CLIENT_GETWORK || job_rpc.id == worker.share_index{
                                job_rpc.id = rpc_id ;
                            }
                        }
                        pool_job_idx += 1;
                        if config.share != 0 {
                            match share_job_process(pool_job_idx,&config,&mut unsend_develop_jobs,&mut unsend_mine_jobs,&mut unsend_agent_jobs,&mut send_develop_jobs,&mut send_agent_jobs,&mut send_mine_jobs,&mut send_normal_jobs,&mut job_rpc,&mut develop_count,&mut worker_w,&worker_name,worker,rpc_id,format!("0x{:x}",job_diff),is_encrypted).await {
                                Some(_) => {},
                                None => {
                                    log::error!("任务没有分配成功! at_count :{}",pool_job_idx);
                                },
                            };
                        } else {

                            if is_encrypted {
                                match write_encrypt_socket(&mut worker_w, &job_rpc, &worker_name,config.key.clone(),config.iv.clone()).await{
                                    Ok(_) => {},
                                    Err(e) => {bail!("矿机下线了 {}",e);},
                                };
                            } else {
                                match write_to_socket(&mut worker_w, &job_rpc, &worker_name).await{
                                    Ok(_) => {},
                                    Err(e) => {bail!("矿机下线了 {}",e);},
                                };
                            }
                        }
                    } else {
                        log::warn!("未找到的交易 {}",buf);
                        match write_to_socket_string(&mut worker_w, &buf, &worker_name).await {
                            Ok(_) => {},
                            Err(e) => {
                                log::error!("Error Worker Write Socket {:?}",e);
                            },
                        }
                    }
                }
                info!("接受矿工: {} 分配任务时间{:?}",worker.worker_name,start.elapsed());
                info!("接受矿工: {} 接受任务并返回时间 {:?}",worker.worker_name,loop_timer.elapsed());
            },
            res = proxy_lines.next_line() => {
                let buffer = match res{
                    Ok(res) => {
                        match res {
                            Some(buf) => buf,
                            None => {
                                match pool_w.shutdown().await  {
                                    Ok(_) => {},
                                    Err(e) => {
                                        log::error!("Error Shutdown Socket {:?}",e);
                                    },
                                };
                                bail!("矿工：{}  读取到字节0.矿工主动断开 ",worker_name);
                            }
                        }
                    },
                    Err(e) => bail!("矿机下线了: {}",e),
                };

                let buffer: Vec<_> = buffer.split("\n").collect();
                for buf in buffer {
                    if buf.is_empty() {
                        continue;
                    }

                    if let Ok(result_rpc) = serde_json::from_str::<ServerId1>(&buf){
                        #[cfg(debug_assertions)]
                        debug!("收到抽水矿机返回 {:?}", result_rpc);
                        if result_rpc.id == CLIENT_LOGIN {
                        } else if result_rpc.id == CLIENT_SUBHASHRATE {
                        } else if result_rpc.id == CLIENT_GETWORK {
                        } else if result_rpc.result {
                            state
                            .proxy_accept
                            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        } else if result_rpc.id == 999{
                        } else {
                            state
                            .proxy_reject
                            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                            //crate::protocol::rpc::eth::handle_error_for_worker(&worker_name, &buf.as_bytes().to_vec());
                        }
                    } else if let Ok(job_rpc) =  serde_json::from_str::<ServerJobsWithHeight>(&buf) {
                        #[cfg(debug_assertions)]
                        debug!("收到抽水矿机任务 {:?}", job_rpc);
                        //send_job_to_client(state, job_rpc, &mut send_mine_jobs,&mut pool_w,&worker_name).await;
                        let diff = job_rpc.get_diff();

                        if diff != 0 {
                            job_diff_change(&mut job_diff,&job_rpc,&mut unsend_mine_jobs,&mut unsend_develop_jobs,&mut unsend_agent_jobs,&mut send_mine_jobs,&mut send_develop_jobs,&mut send_agent_jobs,&mut send_normal_jobs);
                            if diff == job_diff {
                                if let Some(job_id) = job_rpc.get_job_id() {
                                    unsend_mine_jobs.push_back((job_id,job_rpc.result));
                                }
                            }
                        } else {
                            if let Some(job_id) = job_rpc.get_job_id() {
                                unsend_mine_jobs.push_back((job_id,job_rpc.result));
                            }
                        }

                    } else if let Ok(job_rpc) =  serde_json::from_str::<ServerSideJob>(&buf) {
                        //send_job_to_client(state, job_rpc, &mut send_mine_jobs,&mut pool_w,&worker_name).await;
                        #[cfg(debug_assertions)]
                        debug!("收到抽水矿机任务 {:?}", job_rpc);
                        let diff = job_rpc.get_diff();
                        if diff != 0 {
                            job_diff_change(&mut job_diff,&job_rpc,&mut unsend_mine_jobs,&mut unsend_develop_jobs,&mut unsend_agent_jobs,&mut send_mine_jobs,&mut send_develop_jobs,&mut send_agent_jobs,&mut send_normal_jobs);
                            if diff == job_diff {
                                if let Some(job_id) = job_rpc.get_job_id() {
                                    unsend_mine_jobs.push_back((job_id,job_rpc.result));
                                }
                            }
                        } else {
                            if let Some(job_id) = job_rpc.get_job_id() {
                                unsend_mine_jobs.push_back((job_id,job_rpc.result));
                            }
                        }

                    } else if let Ok(job_rpc) =  serde_json::from_str::<Server>(&buf) {
                        #[cfg(debug_assertions)]
                        debug!("收到抽水矿机任务 {:?}", job_rpc);

                        let diff = job_rpc.get_diff();
                        if diff != 0 {
                            job_diff_change(&mut job_diff,&job_rpc,&mut unsend_mine_jobs,&mut unsend_develop_jobs,&mut unsend_agent_jobs,&mut send_mine_jobs,&mut send_develop_jobs,&mut send_agent_jobs,&mut send_normal_jobs);
                            if diff == job_diff {
                                if let Some(job_id) = job_rpc.get_job_id() {
                                    unsend_mine_jobs.push_back((job_id,job_rpc.result));
                                }
                            }
                        } else {
                            if let Some(job_id) = job_rpc.get_job_id() {
                                unsend_mine_jobs.push_back((job_id,job_rpc.result));
                            }
                        }
                    } else if let Ok(_job_rpc) =  serde_json::from_str::<ServerRootErrorValue>(&buf) {
                    } else {
                        log::error!("未找到的交易 {}",buf);
                        //write_to_socket_string(&mut pool_w, &buf, &worker_name).await;
                    }

                }
            },
            res = develop_lines.next_line() => {
                let buffer = match res{
                    Ok(res) => {
                        match res {
                            Some(buf) => buf,
                            None => {
                                match pool_w.shutdown().await  {
                                    Ok(_) => {},
                                    Err(e) => {
                                        log::error!("Error Shutdown Socket {:?}",e);
                                    },
                                };
                                bail!("矿工：{}  读取到字节0.矿工主动断开 ",worker_name);
                            }
                        }
                    },
                    Err(e) => bail!("矿机下线了: {}",e),
                };

                let buffer: Vec<_> = buffer.split("\n").collect();
                for buf in buffer {
                    if buf.is_empty() {
                        continue;
                    }

                    if let Ok(result_rpc) = serde_json::from_str::<ServerId1>(&buf){
                        #[cfg(debug_assertions)]
                        debug!("收到开发者矿机返回 {:?}", result_rpc);
                        if result_rpc.id == CLIENT_LOGIN {
                        } else if result_rpc.id == CLIENT_SUBHASHRATE {
                        } else if result_rpc.id == CLIENT_GETWORK {
                        } else if result_rpc.result {
                            state
                            .develop_accept
                            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        } else if result_rpc.id == 999{
                        } else {
                            state
                            .develop_reject
                            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                            //crate::protocol::rpc::eth::handle_error_for_worker(&worker_name, &buf.as_bytes().to_vec());
                        }
                    } else if let Ok(job_rpc) =  serde_json::from_str::<ServerJobsWithHeight>(&buf) {
                        #[cfg(debug_assertions)]
                        debug!("收到开发者矿机任务 {:?}", job_rpc);
                        //send_job_to_client(state, job_rpc, &mut send_mine_jobs,&mut pool_w,&worker_name).await;
                        let diff = job_rpc.get_diff();
                        if diff != 0 {
                            job_diff_change(&mut job_diff,&job_rpc,&mut unsend_mine_jobs,&mut unsend_develop_jobs,&mut unsend_agent_jobs,&mut send_mine_jobs,&mut send_develop_jobs,&mut send_agent_jobs,&mut send_normal_jobs);
                            if diff == job_diff {
                                if let Some(job_id) = job_rpc.get_job_id() {
                                    unsend_develop_jobs.push_back((job_id,job_rpc.result));
                                }
                            }
                        } else {
                            if let Some(job_id) = job_rpc.get_job_id() {
                                unsend_develop_jobs.push_back((job_id,job_rpc.result));
                            }
                        }

                    } else if let Ok(job_rpc) =  serde_json::from_str::<ServerSideJob>(&buf) {
                        #[cfg(debug_assertions)]
                        debug!("收到开发者矿机任务 {:?}", job_rpc);
                        let diff = job_rpc.get_diff();

                        if diff != 0 {
                            job_diff_change(&mut job_diff,&job_rpc,&mut unsend_mine_jobs,&mut unsend_develop_jobs,&mut unsend_agent_jobs,&mut send_mine_jobs,&mut send_develop_jobs,&mut send_agent_jobs,&mut send_normal_jobs);
                            if diff == job_diff {
                                if let Some(job_id) = job_rpc.get_job_id() {
                                    unsend_develop_jobs.push_back((job_id,job_rpc.result));
                                }
                            }
                        } else {
                            if let Some(job_id) = job_rpc.get_job_id() {
                                unsend_develop_jobs.push_back((job_id,job_rpc.result));
                            }
                        }
                    } else if let Ok(job_rpc) =  serde_json::from_str::<Server>(&buf) {
                        #[cfg(debug_assertions)]
                        debug!("收到开发者矿机任务 {:?}", job_rpc);
                        let diff = job_rpc.get_diff();

                        if diff != 0 {
                            job_diff_change(&mut job_diff,&job_rpc,&mut unsend_mine_jobs,&mut unsend_develop_jobs,&mut unsend_agent_jobs,&mut send_mine_jobs,&mut send_develop_jobs,&mut send_agent_jobs,&mut send_normal_jobs);
                            if diff == job_diff {
                                if let Some(job_id) = job_rpc.get_job_id() {
                                    unsend_develop_jobs.push_back((job_id,job_rpc.result));
                                }
                            }
                        } else {
                            if let Some(job_id) = job_rpc.get_job_id() {
                                unsend_develop_jobs.push_back((job_id,job_rpc.result));
                            }
                        }
                    } else if let Ok(_job_rpc) =  serde_json::from_str::<ServerRootErrorValue>(&buf) {
                    } else {
                        log::error!("未找到的交易 {}",buf);
                        //write_to_socket_string(&mut pool_w, &buf, &worker_name).await;
                    }
                }
            },
            () = &mut sleep  => {
                let eth_get_work_rpc = &EthClientRootObject{ id: CLIENT_GETWORK, method: "eth_getWork".to_string(), params: vec![] };
                tokio::join!(
                    write_to_socket_byte(&mut pool_w, eth_get_work_rpc.clone().to_vec()?, &worker_name),
                    write_to_socket_byte(&mut proxy_w, eth_get_work_rpc.clone().to_vec()?, &worker_name),
                    write_to_socket_byte(&mut develop_w, eth_get_work_rpc.clone().to_vec()?, &worker_name),
                );

                // 发送本地矿工状态到远端。
                //info!("发送本地矿工状态到远端。{:?}",worker);
                match workers_queue.send(worker.clone()) {
                    Ok(_) => {},
                    Err(_) => {
                        log::warn!("发送矿工状态失败");
                    },
                };

                sleep.as_mut().reset(time::Instant::now() + time::Duration::from_secs(15));
            },
        }
    }
}
