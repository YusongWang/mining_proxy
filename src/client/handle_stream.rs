use std::{f32::consts::E, io::Error};

use anyhow::{bail, Result};

use hex::FromHex;
use tracing::{debug, info};

use lru::LruCache;
use openssl::symm::{decrypt, Cipher};

use tokio::{
    io::{
        AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader,
        Lines, ReadHalf, WriteHalf,
    },
    net::TcpStream,
    select, time,
};

use crate::{
    client::*,
    protocol::{
        ethjson::{
            EthServer, EthServerRoot, EthServerRootObject,
            EthServerRootObjectBool, EthServerRootObjectError,
            EthServerRootObjectJsonRpc,
        },
        rpc::eth::{
            Server, ServerId1, ServerJobsWithHeight, ServerRootErrorValue,
            ServerSideJob,
        },
        CLIENT_GETWORK, CLIENT_LOGIN, CLIENT_SUBHASHRATE, CLIENT_SUBMITWORK,
        SUBSCRIBE,
    },
    state::Worker,
    util::{config::Settings, get_eth_wallet, is_fee_random},
};

use super::write_to_socket;

async fn new_eth_submit_login<W>(
    worker: &mut Worker, w: &mut WriteHalf<W>,
    rpc: &mut Box<dyn EthClientObject + Send + Sync>, worker_name: &mut String,
) -> Result<()>
where
    W: AsyncWrite,
{
    if let Some(wallet) = rpc.get_eth_wallet() {
        rpc.set_id(CLIENT_LOGIN);
        let mut temp_worker = wallet.clone();
        let mut split = wallet.split(".").collect::<Vec<&str>>();
        if split.len() > 1 {
            worker.login(
                temp_worker.clone(),
                split.get(1).unwrap().to_string(),
                wallet.clone(),
            );
            *worker_name = temp_worker;
        } else {
            temp_worker.push_str(".");
            temp_worker = temp_worker + rpc.get_worker_name().as_str();
            worker.login(
                temp_worker.clone(),
                rpc.get_worker_name(),
                wallet.clone(),
            );
            *worker_name = temp_worker;
        }

        write_to_socket_byte(w, rpc.to_vec()?, &worker_name).await
    } else {
        bail!("请求登录出错。可能收到暴力攻击");
    }
}

async fn new_eth_submit_work<W, W1, W2>(
    worker: &mut Worker, pool_w: &mut WriteHalf<W>,
    proxy_w: &mut WriteHalf<W1>, worker_w: &mut WriteHalf<W2>,
    rpc: &mut Box<dyn EthClientObject + Send + Sync>, worker_name: &String,
    mine_send_jobs: &mut LruCache<
        std::string::String,
        Vec<std::string::String>,
    >,
    develop_send_jobs: &mut LruCache<
        std::string::String,
        Vec<std::string::String>,
    >,
    config: &Settings, state: &mut State,
) -> Result<()>
where
    W: AsyncWrite,
    W1: AsyncWrite,
    W2: AsyncWrite,
{
    rpc.set_id(CLIENT_SUBMITWORK);
    if let Some(job_id) = rpc.get_job_id() {
        #[cfg(debug_assertions)]
        debug!("提交的JobID {}", job_id);
        if mine_send_jobs.contains(&job_id) {
            let hostname = config.get_share_name().unwrap();
            // state
            //     .proxy_share
            //     .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            rpc.set_worker_name(&hostname);
            #[cfg(debug_assertions)]
            debug!("得到抽水任务。{:?}", rpc);

            write_to_socket_byte(proxy_w, rpc.to_vec()?, &config.share_name)
                .await?;
            return Ok(());
        } else {
            worker.share_index_add();
            //rpc.set_id(worker.share_index);
            write_to_socket_byte(pool_w, rpc.to_vec()?, &worker_name).await
        }
    } else {
        worker.share_index_add();
        //rpc.set_id(worker.share_index);
        write_to_socket_byte(pool_w, rpc.to_vec()?, &worker_name).await
    }
}

async fn new_eth_submit_hashrate<W>(
    worker: &mut Worker, w: &mut WriteHalf<W>,
    rpc: &mut Box<dyn EthClientObject + Send + Sync>, worker_name: &String,
) -> Result<()>
where
    W: AsyncWrite,
{
    worker.new_submit_hashrate(rpc);
    rpc.set_id(CLIENT_SUBHASHRATE);
    write_to_socket_byte(w, rpc.to_vec()?, &worker_name).await
}

async fn new_eth_get_work<W>(
    w: &mut WriteHalf<W>, rpc: &mut Box<dyn EthClientObject + Send + Sync>,
    worker_name: &String,
) -> Result<()>
where
    W: AsyncWrite,
{
    rpc.set_id(CLIENT_GETWORK);
    write_to_socket_byte(w, rpc.to_vec()?, &worker_name).await
}

async fn new_subscribe<W>(
    w: &mut WriteHalf<W>, rpc: &mut Box<dyn EthClientObject + Send + Sync>,
    worker_name: &String,
) -> Result<()>
where
    W: AsyncWrite,
{
    rpc.set_id(SUBSCRIBE);
    write_to_socket_byte(w, rpc.to_vec()?, &worker_name).await
}

// pub fn new_job_diff_change(
//     diff: &mut u64,
//     rpc: &EthServerRootObject,
//     a: &mut VecDeque<(String, Vec<String>)>,
//     b: &mut VecDeque<(String, Vec<String>)>,
//     c: &mut VecDeque<(String, Vec<String>)>,
// ) -> bool
// {
//     let job_diff = rpc.get_diff();
//     if job_diff == 0 {
//         return true;
//     }

//     if job_diff > *diff {
//         // 写入新难度
//         *diff = job_diff;
//         // 清空已有任务队列
//         a.clear();
//         b.clear();
//         c.clear();
//     }

//     true
// }

async fn buf_parse_to_string<W>(
    w: &mut WriteHalf<W>, buffer: &[u8],
) -> Result<String>
where W: AsyncWrite {
    let buf = match String::from_utf8(buffer.to_vec()) {
        Ok(s) => Ok(s),
        Err(_) => {
            //tracing::warn!("无法解析的字符串{:?}", buffer);
            match w.shutdown().await {
                Ok(_) => {
                    //tracing::warn!("端口可能被恶意扫描: {}", buf);
                }
                Err(e) => {
                    tracing::error!("Error Shutdown Socket {:?}", e);
                }
            };
            bail!("端口可能被恶意扫描。也可能是协议被加密了。");
        }
    };

    buf
}

pub async fn write_rpc<W, T>(
    encrypt: bool, w: &mut WriteHalf<W>, rpc: &T, worker: &String, key: String,
    iv: String,
) -> Result<()>
where
    W: AsyncWrite,
    T: Serialize,
{
    if encrypt {
        write_encrypt_socket(w, &rpc, &worker, key, iv).await
    } else {
        write_to_socket(w, &rpc, &worker).await
    }
}

async fn develop_pool_login(
    hostname: String,
) -> Result<(Lines<BufReader<ReadHalf<TcpStream>>>, WriteHalf<TcpStream>)> {
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

    let develop_name = hostname + "_develop";
    let login_develop = ClientWithWorkerName {
        id: CLIENT_LOGIN,
        method: "eth_submitLogin".into(),
        params: vec![get_eth_wallet(), "x".into()],
        worker: develop_name.to_string(),
    };

    write_to_socket(&mut develop_w, &login_develop, &develop_name).await?;

    Ok((develop_lines, develop_w))
}

pub async fn pool_with_tcp_reconnect(
    config: &Settings,
) -> Result<(Lines<BufReader<ReadHalf<TcpStream>>>, WriteHalf<TcpStream>)> {
    let (stream_type, pools) = match crate::client::get_pool_ip_and_type(config)
    {
        Ok(pool) => pool,
        Err(_) => {
            bail!("未匹配到矿池 或 均不可链接。请修改后重试");
        }
    };

    // if stream_type == crate::client::TCP {
    let (outbound, _) = match crate::client::get_pool_stream(&pools) {
        Some((stream, addr)) => (stream, addr),
        None => {
            bail!("所有TCP矿池均不可链接。请修改后重试");
        }
    };

    let stream = TcpStream::from_std(outbound)?;

    let (pool_r, pool_w) = tokio::io::split(stream);
    let pool_r = tokio::io::BufReader::new(pool_r);
    let mut pool_lines = pool_r.lines();
    Ok((pool_lines, pool_w))
    // } else if stream_type == crate::client::SSL {
    // let (stream, _) =
    //     match crate::client::get_pool_stream_with_tls(&pools,
    // "proxy".into()).await {         Some((stream, addr)) => (stream,
    // addr),         None => {
    //             bail!("所有SSL矿池均不可链接。请修改后重试");
    //         }
    //     };

    // let (pool_r, pool_w) = tokio::io::split(stream);
    // let pool_r = tokio::io::BufReader::new(pool_r);

    // Ok((pool_r, pool_w))
    // } else {
    //     tracing::error!("致命错误：未找到支持的矿池BUG 请上报");
    //     bail!("致命错误：未找到支持的矿池BUG 请上报");
    // }
}

pub async fn pool_with_ssl_reconnect(
    config: &Settings,
) -> Result<(Lines<BufReader<ReadHalf<TcpStream>>>, WriteHalf<TcpStream>)> {
    let (stream_type, pools) = match crate::client::get_pool_ip_and_type(config)
    {
        Ok(pool) => pool,
        Err(_) => {
            bail!("未匹配到矿池 或 均不可链接。请修改后重试");
        }
    };
    let (outbound, _) = match crate::client::get_pool_stream(&pools) {
        Some((stream, addr)) => (stream, addr),
        None => {
            bail!("所有TCP矿池均不可链接。请修改后重试");
        }
    };

    let stream = TcpStream::from_std(outbound)?;

    let (pool_r, pool_w) = tokio::io::split(stream);
    let pool_r = tokio::io::BufReader::new(pool_r);
    let mut pool_lines = pool_r.lines();
    Ok((pool_lines, pool_w))
}

pub async fn handle_stream<R, W>(
    worker: &mut Worker, workers_queue: UnboundedSender<Worker>,
    worker_r: tokio::io::BufReader<tokio::io::ReadHalf<R>>,
    mut worker_w: WriteHalf<W>,
    pool_r: tokio::io::BufReader<tokio::io::ReadHalf<TcpStream>>,
    mut pool_w: WriteHalf<TcpStream>, config: &Settings, mut state: State,
    is_encrypted: bool,
) -> Result<()>
where
    R: AsyncRead,
    W: AsyncWrite,
{
    let mut worker_name: String = String::new();
    let mut eth_server_result = EthServerRoot {
        id: 0,
        jsonrpc: "2.0".into(),
        result: true,
    };

    let s = config.get_share_name().unwrap();

    let (mut proxy_lines, mut proxy_w) =
        proxy_pool_login(&config, s.clone()).await?;

    // 池子 给矿机的封包总数。
    let mut pool_job_idx: u64 = 0;
    //最后一次发送的rpc_id
    let mut rpc_id = 0;

    let mut send_proxy_jobs: LruCache<String, Vec<String>> = LruCache::new(500);
    let mut send_develop_jobs: LruCache<String, Vec<String>> =
        LruCache::new(500);

    // 包装为封包格式。
    let mut pool_lines = pool_r.lines();

    let mut worker_lines;

    if is_encrypted {
        worker_lines = worker_r.split(SPLIT);
    } else {
        worker_lines = worker_r.split(b'\n');
    }

    let mut sleep_count: usize = 0;

    let sleep = time::sleep(tokio::time::Duration::from_secs(15));
    tokio::pin!(sleep);

    let mut first_submit_hashrate = true;

    loop {
        select! {
            res = worker_lines.next_segment() => {
                let start = std::time::Instant::now();
                let mut buf_bytes = seagment_unwrap(&mut pool_w,res,&worker_name).await?;


                if is_encrypted {
                    let key = Vec::from_hex(config.key.clone()).unwrap();
                    let iv = Vec::from_hex(config.iv.clone()).unwrap();
                    let cipher = Cipher::aes_256_cbc();

                    buf_bytes = match base64::decode(&buf_bytes[..]) {
                        Ok(buffer) => buffer,
                        Err(e) => {
                            tracing::error!("{}",e);
                            match pool_w.shutdown().await  {
                                Ok(_) => {},
                                Err(_) => {
                                    tracing::error!("Error Shutdown Socket {:?}",e);
                                },
                            };
                            bail!("解密矿机请求失败{}",e);
                        },
                    };

                    buf_bytes = match decrypt(
                        cipher,
                        &key,
                        Some(&iv),
                        &buf_bytes[..]) {
                            Ok(s) => s,
                            Err(e) => {
                                tracing::warn!("加密报文解密失败");
                                match pool_w.shutdown().await  {
                                    Ok(_) => {},
                                    Err(e) => {
                                        tracing::error!("Error Shutdown Socket {:?}",e);
                                    },
                                };
                                bail!("解密矿机请求失败{}",e);
                        },
                    };
                }

                #[cfg(debug_assertions)]
                debug!("0:  矿机 -> 矿池 {} #{:?}", worker_name, buf_bytes);
                let buf_bytes = buf_bytes.split(|c| *c == b'\n');
                for buffer in buf_bytes {
                    if buffer.is_empty() {
                        continue;
                    }

                    if let Some(mut json_rpc) = parse(&buffer) {
                        #[cfg(debug_assertions)]
                        info!("接受矿工: {} 提交 RPC {:?}",worker.worker_name,json_rpc);

                        rpc_id = json_rpc.get_id();
                        let res = match json_rpc.get_method().as_str() {
                            "eth_submitLogin" => {
                                eth_server_result.id = rpc_id;
                                new_eth_submit_login(worker,&mut pool_w,&mut json_rpc,&mut worker_name).await?;
                                write_rpc(is_encrypted,&mut worker_w,&eth_server_result,&worker_name,config.key.clone(),config.iv.clone()).await?;
                                Ok(())
                            },
                            "eth_submitWork" => {
                                eth_server_result.id = rpc_id;
                                new_eth_submit_work(worker,&mut pool_w,&mut proxy_w,&mut worker_w,&mut json_rpc,&mut worker_name,&mut send_proxy_jobs,&mut send_develop_jobs,&config,&mut state).await?;
                                write_rpc(is_encrypted,&mut worker_w,&eth_server_result,&worker_name,config.key.clone(),config.iv.clone()).await?;
                                Ok(())
                            },
                            "eth_submitHashrate" => {
                                eth_server_result.id = rpc_id;
                                // TODO 削弱报告算力 FIX ME
                                // if true {
                                //     let mut hash = json_rpc.get_submit_hashrate();
                                //     hash = hash - (hash  as f32 * config.share_rate) as u64;
                                //     json_rpc.set_submit_hashrate(format!("0x{:x}", hash));
                                // }
                                new_eth_submit_hashrate(worker,&mut pool_w,&mut json_rpc,&mut worker_name).await?;
                                write_rpc(is_encrypted,&mut worker_w,&eth_server_result,&worker_name,config.key.clone(),config.iv.clone()).await?;

                                Ok(())
                            },
                            "eth_getWork" => {
                                //eth_server_result.id = rpc_id;
                                new_eth_get_work(&mut pool_w,&mut json_rpc,&mut worker_name).await?;
                                //write_rpc(is_encrypted,&mut worker_w,eth_server_result,&worker_name,config.key.clone(),config.iv.clone()).await?;
                                Ok(())
                            },
                            _ => {
                                //tracing::warn!("Not found method {:?}",json_rpc);
                                eth_server_result.id = rpc_id;
                                write_to_socket_byte(&mut pool_w,buffer.to_vec(),&mut worker_name).await?;
                                Ok(())
                            },
                        };

                        if res.is_err() {
                            tracing::warn!("写入任务错误: {:?}",res);
                            return res;
                        }
                    } else {
                        tracing::warn!("协议解析错误: {:?}",buffer);
                        bail!("未知的协议{}",buf_parse_to_string(&mut worker_w,&buffer).await?);
                    }
                }
                #[cfg(debug_assertions)]
                info!("接受矿工: {} 提交处理时间{:?}",worker.worker_name,start.elapsed());
            },
            res = pool_lines.next_line() => {
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
                        let job_id = job_rpc.get_job_id().unwrap();
                        let job_res = job_rpc.get_job_result().unwrap();

                        if is_fee_random(config.share_rate.into()) {
                            #[cfg(debug_assertions)]
                            info!("中转抽水回合");
                            // if let Some(job_res) = unsend_proxy_jobs.pop_back() {
                            //     if let Some(job_id) = job_res.get(0) {
                            //         //TODO 伪装为result 有多少位就取多少位
                            //         job_rpc.result = job_res.clone();
                            //         send_proxy_jobs.put(job_id.to_string(),job_res);
                            //     }
                            // }
                         //获取开发者抽水任务
                        }

                        write_rpc(is_encrypted,&mut worker_w,&job_rpc,&worker_name,config.key.clone(),config.iv.clone()).await?;
                    } else if let Ok(mut result_rpc) = serde_json::from_str::<EthServerRoot>(&buf) {
                        if result_rpc.id == CLIENT_LOGIN {
                            worker.logind();
                        } else if result_rpc.id == CLIENT_SUBHASHRATE {

                        } else if result_rpc.id == CLIENT_GETWORK {

                        } else if result_rpc.id == SUBSCRIBE{
                        } else if result_rpc.id == CLIENT_SUBMITWORK && result_rpc.result {
                            worker.share_accept();
                        } else if result_rpc.id == CLIENT_SUBMITWORK {
                            worker.share_reject();
                        }
                    }
                }
            },
            () = &mut sleep  => {
                match workers_queue.send(worker.clone()) {
                    Ok(_) => {},
                    Err(_) => {
                        tracing::warn!("发送矿工状态失败");
                    },
                };

                //info!("提交常规任务");
                sleep.as_mut().reset(time::Instant::now() + time::Duration::from_secs(30));
            },
        }
    }
}
