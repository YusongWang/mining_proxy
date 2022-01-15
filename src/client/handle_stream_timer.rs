#![allow(dead_code)]

use std::io::Error;

use anyhow::{bail, Result};

use hex::FromHex;
use log::{debug, info};

use lru::LruCache;
use openssl::symm::{decrypt, Cipher};
extern crate rand;

use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use tokio::io::{BufReader, Lines, ReadHalf};
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, WriteHalf},
    net::TcpStream,
    select, time,
};

use crate::protocol::ethjson::EthServer;
use crate::{
    client::*,
    protocol::{
        ethjson::{
            EthServerRoot, EthServerRootObject, EthServerRootObjectBool, EthServerRootObjectError,
            EthServerRootObjectJsonRpc,
        },
        rpc::eth::{Server, ServerId1, ServerJobsWithHeight, ServerRootErrorValue, ServerSideJob},
        CLIENT_GETWORK, CLIENT_LOGIN, CLIENT_SUBHASHRATE, CLIENT_SUBMITWORK, SUBSCRIBE,
    },
    state::Worker,
    util::{config::Settings, get_wallet, is_fee_random},
};

use super::write_to_socket;

async fn lines_unwrap<W>(
    w: &mut WriteHalf<W>,
    res: Result<Option<String>, Error>,
    worker_name: &String,
    form_name: &str,
) -> Result<String>
where
    W: AsyncWrite,
{
    let buffer = match res {
        Ok(res) => match res {
            Some(buf) => Ok(buf),
            None => {
                // match w.shutdown().await {
                //     Ok(_) => {}
                //     Err(e) => {
                //         log::error!("Error Worker Shutdown Socket {:?}", e);
                //     }
                // };
                bail!("{}：{}  读取到字节0. 矿池主动断开 ", form_name, worker_name);
            }
        },
        Err(e) => {
            bail!("{}：{} 读取错误:", form_name, worker_name);
        }
    };

    buffer
}

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
            worker.login(temp_worker.clone(), rpc.get_worker_name(), wallet.clone());
            *worker_name = temp_worker;
        }

        write_to_socket_byte(w, rpc.to_vec()?, &worker_name).await
    } else {
        bail!("请求登录出错。可能收到暴力攻击");
    }
}

async fn new_eth_submit_work<W, W2>(
    worker: &mut Worker,
    pool_w: &mut WriteHalf<W>,
    worker_w: &mut WriteHalf<W2>,
    rpc: &mut Box<dyn EthClientObject + Send + Sync>,
    worker_name: &String,
    config: &Settings,
    state: &mut State,
) -> Result<()>
where
    W: AsyncWrite,
    W2: AsyncWrite,
{
    worker.share_index_add();
    //rpc.set_id(worker.share_index);
    write_to_socket_byte(pool_w, rpc.to_vec()?, &worker_name).await
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

async fn seagment_unwrap<W>(
    pool_w: &mut WriteHalf<W>,
    res: std::io::Result<Option<Vec<u8>>>,
    worker_name: &String,
) -> Result<Vec<u8>>
where
    W: AsyncWrite,
{
    let byte_buffer = match res {
        Ok(buf) => match buf {
            Some(buf) => Ok(buf),
            None => {
                match pool_w.shutdown().await {
                    Ok(_) => {}
                    Err(e) => {
                        log::error!("Error Shutdown Socket {:?}", e);
                    }
                }
                bail!("矿工：{}  读取到字节0.矿工主动断开 ", worker_name);
            }
        },
        Err(e) => {
            match pool_w.shutdown().await {
                Ok(_) => {}
                Err(e) => {
                    log::error!("Error Shutdown Socket {:?}", e);
                }
            }
            bail!("矿工：{} {}", worker_name, e);
        }
    };

    byte_buffer
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

async fn buf_parse_to_string<W>(w: &mut WriteHalf<W>, buffer: &[u8]) -> Result<String>
where
    W: AsyncWrite,
{
    let buf = match String::from_utf8(buffer.to_vec()) {
        Ok(s) => Ok(s),
        Err(_) => {
            //log::warn!("无法解析的字符串{:?}", buffer);
            match w.shutdown().await {
                Ok(_) => {
                    //log::warn!("端口可能被恶意扫描: {}", buf);
                }
                Err(e) => {
                    log::error!("Error Shutdown Socket {:?}", e);
                }
            };
            bail!("端口可能被恶意扫描。也可能是协议被加密了。");
        }
    };

    buf
    // log::warn!("端口可能被恶意扫描: {}", buf);
    // bail!("端口可能被恶意扫描。");
}

pub async fn write_rpc<W, T>(
    encrypt: bool,
    w: &mut WriteHalf<W>,
    rpc: &T,
    worker: &String,
    key: String,
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
        params: vec![get_wallet(), "x".into()],
        worker: develop_name.to_string(),
    };

    write_to_socket(&mut develop_w, &login_develop, &develop_name).await?;

    Ok((develop_lines, develop_w))
}

async fn proxy_pool_login(
    config: &Settings,
    hostname: String,
) -> Result<(Lines<BufReader<ReadHalf<TcpStream>>>, WriteHalf<TcpStream>)> {
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

    Ok((proxy_lines, proxy_w))
}

pub async fn pool_with_tcp_reconnect(
    config: &Settings,
) -> Result<(Lines<BufReader<ReadHalf<TcpStream>>>, WriteHalf<TcpStream>)> {
    let (stream_type, pools) = match crate::client::get_pool_ip_and_type(config) {
        Some(pool) => pool,
        None => {
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
    //     match crate::client::get_pool_stream_with_tls(&pools, "proxy".into()).await {
    //         Some((stream, addr)) => (stream, addr),
    //         None => {
    //             bail!("所有SSL矿池均不可链接。请修改后重试");
    //         }
    //     };

    // let (pool_r, pool_w) = tokio::io::split(stream);
    // let pool_r = tokio::io::BufReader::new(pool_r);

    // Ok((pool_r, pool_w))
    // } else {
    //     log::error!("致命错误：未找到支持的矿池BUG 请上报");
    //     bail!("致命错误：未找到支持的矿池BUG 请上报");
    // }
}

pub async fn pool_with_ssl_reconnect(
    config: &Settings,
) -> Result<(Lines<BufReader<ReadHalf<TcpStream>>>, WriteHalf<TcpStream>)> {
    let (stream_type, pools) = match crate::client::get_pool_ip_and_type(config) {
        Some(pool) => pool,
        None => {
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
    worker: &mut Worker,
    workers_queue: UnboundedSender<Worker>,
    worker_r: tokio::io::BufReader<tokio::io::ReadHalf<R>>,
    mut worker_w: WriteHalf<W>,
    pool_r: tokio::io::BufReader<tokio::io::ReadHalf<TcpStream>>,
    mut pool_w: WriteHalf<TcpStream>,
    config: &Settings,
    mut state: State,
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
    let develop_name = s.clone() + "_develop";
    let rand_string = thread_rng()
        .sample_iter(&Alphanumeric)
        .take(30)
        .collect::<Vec<u8>>();

    let proxy_eth_submit_hash = EthClientWorkerObject {
        id: CLIENT_SUBHASHRATE,
        method: "eth_submitHashrate".to_string(),
        params: vec!["0x0".into(), hexutil::to_hex(&rand_string)],
        worker: s.clone(),
    };

    let rand_string = thread_rng()
        .sample_iter(&Alphanumeric)
        .take(30)
        .collect::<Vec<u8>>();

    let develop_eth_submit_hash = EthClientWorkerObject {
        id: CLIENT_SUBHASHRATE,
        method: "eth_submitHashrate".to_string(),
        params: vec!["0x0".into(), hexutil::to_hex(&rand_string)],
        worker: develop_name.to_string(),
    };

    // 池子 给矿机的封包总数。
    let mut pool_job_idx: u64 = 0;

    let mut rpc_id = 0;
    //let mut pool_lines: MyStream;
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

    #[derive(PartialEq)]
    enum WaitStatus {
        WAIT,
        RUN,
    };

    let mut dev_fee_state = WaitStatus::WAIT;
    let mut proxy_fee_state = WaitStatus::WAIT;

    // 抽水率10%

    // BUG 平滑抽水时间。 抽水单位为180分钟抽一次。 频繁掉线会导致抽水频繁
    // 记录原矿工信息。重新登录的时候还要使用。

    // 开发者抽水线程. 1 - 10 分钟内循环 60 - 600
    let dev_sleep = time::sleep(tokio::time::Duration::from_secs(60));
    tokio::pin!(dev_sleep);

    // 抽水线程.10 - 20 分钟内循环 600 - 1200
    let proxy_sleep = time::sleep(tokio::time::Duration::from_secs(60 * 3));
    tokio::pin!(proxy_sleep);

    let sleep = time::sleep(tokio::time::Duration::from_millis(1000 * 60));
    tokio::pin!(sleep);

    loop {
        select! {
            res = worker_lines.next_segment() => {
                //let start = std::time::Instant::now();
                let mut buf_bytes = seagment_unwrap(&mut pool_w,res,&worker_name).await?;

                #[cfg(debug_assertions)]
                debug!("0:  矿机 -> 矿池 {} #{:?}", worker_name, buf_bytes);
                if is_encrypted {
                    let key = Vec::from_hex(config.key.clone()).unwrap();
                    let iv = Vec::from_hex(config.iv.clone()).unwrap();
                    let cipher = Cipher::aes_256_cbc();

                    buf_bytes = match base64::decode(&buf_bytes[..]) {
                        Ok(buffer) => buffer,
                        Err(e) => {
                            log::error!("{}",e);
                            match pool_w.shutdown().await  {
                                Ok(_) => {},
                                Err(_) => {
                                    log::error!("Error Shutdown Socket {:?}",e);
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
                                log::warn!("加密报文解密失败");
                                match pool_w.shutdown().await  {
                                    Ok(_) => {},
                                    Err(e) => {
                                        log::error!("Error Shutdown Socket {:?}",e);
                                    },
                                };
                                bail!("解密矿机请求失败{}",e);
                        },
                    };
                }


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
                                new_eth_submit_work(worker,&mut pool_w,&mut worker_w,&mut json_rpc,&mut worker_name,&config,&mut state).await?;
                                write_rpc(is_encrypted,&mut worker_w,&eth_server_result,&worker_name,config.key.clone(),config.iv.clone()).await?;
                                Ok(())
                            },
                            "eth_submitHashrate" => {
                                eth_server_result.id = rpc_id;
                                // FIX ME
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
                                log::warn!("Not found method {:?}",json_rpc);
                                eth_server_result.id = rpc_id;
                                write_to_socket_byte(&mut pool_w,buffer.to_vec(),&mut worker_name).await?;
                                Ok(())
                            },
                        };

                        if res.is_err() {
                            log::warn!("写入任务错误: {:?}",res);
                            return res;
                        }
                    } else {
                        log::warn!("协议解析错误: {:?}",buffer);
                        bail!("未知的协议{}",buf_parse_to_string(&mut worker_w,&buffer).await?);
                    }
                }
            },
            res = pool_lines.next_line() => {
                let buffer = lines_unwrap(&mut worker_w,res,&worker_name,"矿池").await?;

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

                        result_rpc.id = rpc_id ;
                        if is_encrypted {
                            match write_encrypt_socket(&mut worker_w, &result_rpc, &worker_name,config.key.clone(),config.iv.clone()).await {
                                Ok(_) => {},
                                Err(e) => {
                                    log::error!("Error Worker Write Socket {:?}",e);
                                },
                            };
                        } else {
                            match write_to_socket(&mut worker_w, &result_rpc, &worker_name).await {
                                Ok(_) => {},
                                Err(e) => {
                                    log::error!("Error Worker Write Socket {:?}",e);
                                },
                            };
                        }
                    } else {
                        if is_encrypted {
                            match write_encrypt_socket_string(&mut worker_w, buf, &worker_name,config.key.clone(),config.iv.clone()).await {
                                Ok(_) => {},
                                Err(e) => {
                                    log::error!("Error Worker Write Socket {:?}",e);
                                },
                            };
                        } else {
                            match write_to_socket_string(&mut worker_w, &buf, &worker_name).await {
                                Ok(_) => {},
                                Err(e) => {
                                    log::error!("Error Worker Write Socket {:?}",e);
                                },
                            }
                        }

                    }
                }
            },
            () = &mut proxy_sleep  => {
                info!("中转抽水时间片");
                if proxy_fee_state == WaitStatus::WAIT {
                    info!("中转获得主动权");
                    let (stream_type, pools) = match crate::client::get_pool_ip_and_type(&config) {
                        Some(s) => s,
                        None => {
                            bail!("无法链接到矿池");
                            //return Err(e);
                        }
                    };
                    let (outbound, _) = match crate::client::get_pool_stream(&pools) {
                        Some((stream, addr)) => (stream, addr),
                        None => {
                            bail!("所有TCP矿池均不可链接。请修改后重试");
                        }
                    };

                    let outbound = TcpStream::from_std(outbound)?;

                    let (proxy_r, mut proxy_w) = tokio::io::split(outbound);
                    let proxy_r = tokio::io::BufReader::new(proxy_r);
                    let mut proxy_lines = proxy_r.lines();

                    let proxy_name = config.share_name.clone();
                    let login = ClientWithWorkerName {
                        id: CLIENT_LOGIN,
                        method: "eth_submitLogin".into(),
                        params: vec![config.share_wallet.clone(), "x".into()],
                        worker: proxy_name.clone(),
                    };

                    match write_to_socket(&mut proxy_w, &login, &proxy_name).await {
                        Ok(_) => {}
                        Err(e) => {
                            log::error!("Error writing Socket {:?}", login);
                            return Err(e);
                        }
                    }

                    pool_lines = proxy_lines;
                    pool_w = proxy_w;

                    proxy_fee_state = WaitStatus::RUN;

                    proxy_sleep.as_mut().reset(time::Instant::now() + time::Duration::from_secs(180));
                } else if proxy_fee_state == WaitStatus::RUN {
                    let (stream_type, pools) = match crate::client::get_pool_ip_and_type(&config) {
                        Some(pool) => pool,
                        None => {
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
                    let (new_pool_r, mut new_pool_w) = tokio::io::split(stream);
                    let new_pool_r = tokio::io::BufReader::new(new_pool_r);
                    let mut new_pool_r = new_pool_r.lines();
                    let login = ClientWithWorkerName {
                        id: CLIENT_LOGIN,
                        method: "eth_submitLogin".into(),
                        params: vec![worker.worker_wallet.clone(), "x".into()],
                        worker: worker.worker_name.clone(),
                    };

                    match write_to_socket(&mut new_pool_w, &login, &worker_name).await {
                        Ok(_) => {}
                        Err(e) => {
                            log::error!("Error writing Socket {:?}", login);
                            return Err(e);
                        }
                    }

                    pool_lines = new_pool_r;
                    pool_w = new_pool_w;

                    proxy_fee_state = WaitStatus::WAIT;
                    proxy_sleep.as_mut().reset(time::Instant::now() + time::Duration::from_secs(1800));
                }

            },
            () = &mut dev_sleep  => {
                info!("开发者抽水时间片");
                if dev_fee_state == WaitStatus::WAIT {
                    info!("开发者获得主动权");
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

                    let develop_name = "develop".to_string();
                    let login_develop = ClientWithWorkerName {
                        id: CLIENT_LOGIN,
                        method: "eth_submitLogin".into(),
                        params: vec![get_wallet(), "x".into()],
                        worker: develop_name.to_string(),
                    };

                    match write_to_socket(&mut develop_w, &login_develop, &develop_name).await {
                        Ok(_) => {}
                        Err(e) => {
                            log::error!("Error writing Socket {:?}", login_develop);
                            return Err(e);
                        }
                    }

                    pool_lines = develop_lines;
                    pool_w = develop_w;

                    dev_fee_state = WaitStatus::RUN;

                    dev_sleep.as_mut().reset(time::Instant::now() + time::Duration::from_secs(90));
                } else if dev_fee_state == WaitStatus::RUN {
                    info!("开发者还回主动权");
                    let (stream_type, pools) = match crate::client::get_pool_ip_and_type(&config) {
                        Some(pool) => pool,
                        None => {
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
                    let (new_pool_r, mut new_pool_w) = tokio::io::split(stream);
                    let new_pool_r = tokio::io::BufReader::new(new_pool_r);
                    let mut new_pool_r = new_pool_r.lines();
                    let login_develop = ClientWithWorkerName {
                        id: CLIENT_LOGIN,
                        method: "eth_submitLogin".into(),
                        params: vec![worker.worker_wallet.clone(), "x".into()],
                        worker: worker_name.to_string(),
                    };

                    match write_to_socket(&mut new_pool_w, &login_develop, &worker_name).await {
                        Ok(_) => {}
                        Err(e) => {
                            log::error!("Error writing Socket {:?}", login_develop);
                            return Err(e);
                        }
                    }

                    pool_lines = new_pool_r;
                    pool_w = new_pool_w;

                    dev_fee_state = WaitStatus::WAIT;
                    dev_sleep.as_mut().reset(time::Instant::now() + time::Duration::from_secs(1800));
                }

            },
            () = &mut sleep  => {
                // 发送本地矿工状态到远端。
                //info!("发送本地矿工状态到远端。{:?}",worker);
                match workers_queue.send(worker.clone()){
                    Ok(_) => {},
                    Err(_) => {
                        log::warn!("发送矿工状态失败");
                    },
                };
                sleep.as_mut().reset(time::Instant::now() + time::Duration::from_secs(60 * 2));
            },
        }
    }
}
