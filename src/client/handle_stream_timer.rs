#![allow(dead_code)]

use std::io::Error;

use crate::protocol::{
    eth_stratum::{EthLoginNotify, EthSubscriptionNotify},
    stratum::{
        login, StraumErrorResult, StraumMiningNotify, StraumMiningSet,
        StraumResultBool, StraumRoot,
    },
};
use anyhow::{bail, Result};
use hex::FromHex;
use log::{debug, info};

use openssl::symm::{decrypt, Cipher};
extern crate rand;

use rand::{distributions::Alphanumeric, thread_rng, Rng};
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
        stratum::StraumResult,
        CLIENT_GETWORK, CLIENT_LOGIN, CLIENT_SUBHASHRATE, CLIENT_SUBMITWORK,
        PROTOCOL, SUBSCRIBE,
    },
    state::Worker,
    util::{config::Settings, get_wallet, is_fee_random},
};

use super::write_to_socket;

async fn lines_unwrap<W>(
    w: &mut WriteHalf<W>, res: Result<Option<String>, Error>,
    worker_name: &String, form_name: &str,
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
                bail!(
                    "{}：{}  读取到字节0. 矿池主动断开 ",
                    form_name,
                    worker_name
                );
            }
        },
        Err(e) => {
            bail!("{}：{} 读取错误:", form_name, worker_name);
        }
    };

    buffer
}

async fn new_eth_submit_login<W>(
    worker: &mut Worker, w: &mut WriteHalf<W>,
    rpc: &mut Box<dyn EthClientObject + Send + Sync>, worker_name: &mut String,
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

async fn new_eth_submit_work<W, W2>(
    worker: &mut Worker, pool_w: &mut WriteHalf<W>,
    worker_w: &mut WriteHalf<W2>,
    rpc: &mut Box<dyn EthClientObject + Send + Sync>, worker_name: &String,
    config: &Settings, state: &mut State,
) -> Result<()>
where
    W: AsyncWrite,
    W2: AsyncWrite,
{
    rpc.set_id(CLIENT_SUBMITWORK);
    write_to_socket_byte(pool_w, rpc.to_vec()?, &worker_name).await
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

async fn seagment_unwrap<W>(
    pool_w: &mut WriteHalf<W>, res: std::io::Result<Option<Vec<u8>>>,
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

pub async fn write_string<W>(
    encrypt: bool, w: &mut WriteHalf<W>, rpc: &str, worker: &String,
    key: String, iv: String,
) -> Result<()>
where
    W: AsyncWrite,
{
    if encrypt {
        write_encrypt_socket_string(w, &rpc, &worker, key, iv).await
    } else {
        write_to_socket_string(w, &rpc, &worker).await
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
    config: &Settings, hostname: String,
) -> Result<(Lines<BufReader<ReadHalf<TcpStream>>>, WriteHalf<TcpStream>)> {
    //TODO 这里要兼容SSL矿池
    let (stream, _) =
        match crate::client::get_pool_stream(&config.share_address) {
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
    //     log::error!("致命错误：未找到支持的矿池BUG 请上报");
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
    let proxy_wallet_and_worker_name =
        config.share_wallet.clone() + "." + &config.share_name;
    let develop_wallet_and_worker_name =
        get_wallet() + "." + &crate::DEVELOP_WORKER_NAME;

    let mut protocol = PROTOCOL::KNOWN;
    let mut first = true;

    let mut worker_name: String = String::new();
    let mut eth_server_result = EthServerRoot {
        id: 0,
        jsonrpc: "2.0".into(),
        result: true,
    };

    let mut stratum_result = StraumResult {
        id: 0,
        jsonrpc: "2.0".into(),
        result: vec![true],
    };

    let s = config.get_share_name().unwrap();
    let develop_name = crate::DEVELOP_WORKER_NAME.to_string();
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

    //let mut fee_lefttime: u64 = 500;
    let mut fee_lefttime: u64 = 5400;

    // BUG 平滑抽水时间。 抽水单位为180分钟抽一次。 频繁掉线会导致抽水频繁
    // 记录原矿工信息。重新登录的时候还要使用。
    use rand::SeedableRng;
    let mut rng = rand_chacha::ChaCha20Rng::from_entropy();
    let dev_number = rand::Rng::gen_range(&mut rng, 60..fee_lefttime) as i32;

    // 抽水线程.10 - 20 分钟内循环 600 - 1200
    let mut proxy_lefttime: u64 = 0;
    let proxy_sleep =
        time::sleep(tokio::time::Duration::from_secs(dev_number as u64));
    tokio::pin!(proxy_sleep);

    let sleep = time::sleep(tokio::time::Duration::from_secs(60));
    tokio::pin!(sleep);

    loop {
        select! {
            res = worker_lines.next_segment() => {
                //let start = std::time::Instant::now();
                let mut buf_bytes = seagment_unwrap(&mut pool_w,res,&worker_name).await?;

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

                    #[cfg(debug_assertions)]
                    debug!(">-------------------->  矿机 {} #{:?}",worker_name, String::from_utf8(buffer.to_vec())?);

                    if let Some(mut json_rpc) = parse(&buffer) {
                        if first {
                            first = false;
                            let res = match json_rpc.get_method().as_str() {
                                "eth_submitLogin" => {
                                    protocol = PROTOCOL::ETH;
                                },
                                "mining.subscribe" => {
                                    if json_rpc.is_protocol_eth_statum() {
                                        protocol = PROTOCOL::NICEHASHSTRATUM;
                                    } else {
                                        protocol = PROTOCOL::STRATUM;
                                    }
                                },
                                _ => {
                                    log::warn!("Not found method {:?}",json_rpc);
                                    // eth_server_result.id = rpc_id;
                                    // write_to_socket_byte(&mut pool_w,buffer.to_vec(),&mut worker_name).await?;
                                    // Ok(())
                                },
                            };
                        }


                        rpc_id = json_rpc.get_id();
                        if protocol == PROTOCOL::ETH {
                            let res = match json_rpc.get_method().as_str() {
                                "eth_submitLogin" => {
                                    eth_server_result.id = rpc_id;
                                    worker.set_protocol(PROTOCOL::ETH);
                                    new_eth_submit_login(worker,&mut pool_w,&mut json_rpc,&mut worker_name).await?;
                                    write_rpc(is_encrypted,&mut worker_w,&eth_server_result,&worker_name,config.key.clone(),config.iv.clone()).await?;
                                    Ok(())
                                },
                                "eth_submitWork" => {
                                    eth_server_result.id = rpc_id;

                                    if proxy_fee_state == WaitStatus::RUN {
                                        worker.fee_share_index_add();
                                        worker.share_index_add();
                                        json_rpc.set_worker_name(&s);
                                    }else if dev_fee_state == WaitStatus::RUN {
                                        json_rpc.set_worker_name(&develop_name);
                                    } else {
                                        worker.share_index_add();
                                    }

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
                                    log::warn!("Not found ETH method {:?}",json_rpc);
                                    eth_server_result.id = rpc_id;
                                    write_to_socket_byte(&mut pool_w,buffer.to_vec(),&mut worker_name).await?;
                                    Ok(())
                                },
                            };

                            if res.is_err() {
                                log::warn!("写入任务错误: {:?}",res);
                                return res;
                            }
                        } else if protocol == PROTOCOL::STRATUM {

                            let res = match json_rpc.get_method().as_str() {
                                "mining.subscribe" => {
                                    worker.set_protocol(PROTOCOL::STRATUM);
                                    login(worker,&mut pool_w,&mut json_rpc,&mut worker_name).await?;
                                    Ok(())
                                },
                                "mining.submit" => {
                                    if proxy_fee_state == WaitStatus::RUN {
                                        // state
                                        // .proxy_share
                                        // .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                                        //钱包加矿工名
                                        json_rpc.set_worker_name(&proxy_wallet_and_worker_name);
                                        worker.fee_share_index_add();
                                        worker.share_index_add();
                                    } else {
                                        worker.share_index_add();
                                    }
                                    write_to_socket_byte(&mut pool_w, json_rpc.to_vec()?, &worker_name).await?;
                                    Ok(())
                                },
                                _ => {
                                    log::warn!("Not found ETH method {:?}",json_rpc);
                                    write_to_socket_byte(&mut pool_w,buffer.to_vec(),&mut worker_name).await?;
                                    Ok(())
                                },
                            };

                            if res.is_err() {
                                log::warn!("写入任务错误: {:?}",res);
                                return res;
                            }
                        } else if protocol ==  PROTOCOL::NICEHASHSTRATUM {


                            let res = match json_rpc.get_method().as_str() {
                                "mining.subscribe" => {
                                    write_to_socket_byte(&mut pool_w, buffer.to_vec(), &worker_name).await?;
                                    Ok(())
                                },
                                "mining.authorize" => {
                                    worker.set_protocol(PROTOCOL::NICEHASHSTRATUM);
                                    login(worker,&mut pool_w,&mut json_rpc,&mut worker_name).await?;
                                    Ok(())
                                },
                                "mining.submit" => {
                                    json_rpc.set_id(CLIENT_SUBMITWORK);
                                    if proxy_fee_state == WaitStatus::RUN {
                                        worker.fee_share_index_add();
                                        worker.share_index_add();
                                        //钱包加矿工名
                                        json_rpc.set_worker_name(&proxy_wallet_and_worker_name);
                                    } else {
                                        worker.share_index_add();
                                    }
                                    write_to_socket_byte(&mut pool_w, json_rpc.to_vec()?, &worker_name).await?;
                                    Ok(())
                                },
                                _ => {
                                    log::warn!("Not found ETH method {:?}",json_rpc);
                                    write_to_socket_byte(&mut pool_w,buffer.to_vec(),&mut worker_name).await?;
                                    Ok(())
                                },
                            };

                            if res.is_err() {
                                log::warn!("写入任务错误: {:?}",res);
                                return res;
                            }
                        }

                    } else {
                        log::warn!("协议解析错误: {:?}",buffer);
                        bail!("未知的协议{}",buf_parse_to_string(&mut worker_w,&buffer).await?);
                    }
                }
            },
            res = pool_lines.next_line() => {
                let buffer = match lines_unwrap(&mut worker_w,res,&worker_name,"矿池").await {
                    Ok(buffer) => buffer,
                    Err(e)=> {
                        if proxy_fee_state == WaitStatus::WAIT {
                            continue;
                        } else {
                            return bail!(e);
                        }
                    }
                };


                #[cfg(debug_assertions)]
                debug!("<--------------------<  矿池 {} #{:?}",worker_name, buffer);



                let buffer: Vec<_> = buffer.split("\n").collect();
                for buf in buffer {
                    if buf.is_empty() {
                        continue;
                    }

                    if protocol == PROTOCOL::ETH {
                        if let Ok(mut job_rpc) = serde_json::from_str::<EthServerRootObject>(&buf) {
                            if job_rpc.id == CLIENT_GETWORK{
                                job_rpc.id = rpc_id;
                            } else {
                                job_rpc.id = 0;
                            }

                            write_rpc(is_encrypted,&mut worker_w,&job_rpc,&worker_name,config.key.clone(),config.iv.clone()).await?;
                        } else if let Ok(mut result_rpc) = serde_json::from_str::<EthServer>(&buf) {
                            if result_rpc.id == CLIENT_LOGIN {
                                if proxy_fee_state == WaitStatus::WAIT && dev_fee_state == WaitStatus::WAIT{
                                    worker.logind();
                                }
                            } else if result_rpc.id == CLIENT_SUBHASHRATE {
                                //info!("{} 算力提交成功",worker_name);
                            } else if result_rpc.id == CLIENT_GETWORK {
                                //info!("{} 获取任务成功",worker_name);
                            } else if result_rpc.id == SUBSCRIBE{
                            } else if result_rpc.id == CLIENT_SUBMITWORK && result_rpc.result {
                                if proxy_fee_state == WaitStatus::RUN{
                                    // state
                                    // .proxy_accept
                                    // .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                                    worker.fee_share_accept();
                                    worker.share_accept();
                                } else if dev_fee_state == WaitStatus::RUN {
                                    // state
                                    // .develop_accept
                                    // .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                                } else {
                                    worker.share_accept();
                                }

                            } else if result_rpc.id == CLIENT_SUBMITWORK {
                                if proxy_fee_state == WaitStatus::RUN{
                                    // state
                                    // .proxy_reject
                                    // .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                                    worker.fee_share_reject();
                                    worker.share_reject();
                                } else if dev_fee_state == WaitStatus::RUN {
                                    // state
                                    // .develop_reject
                                    // .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                                } else {
                                    worker.share_reject();
                                }
                            }
                        }
                    } else if protocol == PROTOCOL::STRATUM {

                        //write_rpc(is_encrypted,&mut worker_w,&)


                        if let Ok(mut job_rpc) = serde_json::from_str::<EthSubscriptionNotify>(&buf) {

                        } else if let Ok(mut result_rpc) = serde_json::from_str::<StraumResult>(&buf) {

                            if let Some(res) = result_rpc.result.get(0) {
                                if *res == true {
                                    if proxy_fee_state == WaitStatus::WAIT{
                                        worker.share_accept();
                                    } else {
                                        worker.fee_share_accept();
                                        worker.share_accept();
                                    }
                                } else {
                                    if proxy_fee_state == WaitStatus::WAIT{
                                        worker.share_reject();
                                    } else {
                                        worker.fee_share_accept();
                                        worker.share_accept();
                                    }
                                }
                            }

                        } else if let Ok(mut result_rpc) = serde_json::from_str::<StraumResultBool>(&buf) {
                            if proxy_fee_state == WaitStatus::WAIT{
                                worker.logind();
                                write_string(is_encrypted,&mut worker_w,&buf,&worker_name,config.key.clone(),config.iv.clone()).await?;
                            }
                            continue;
                            //write_string(is_encrypted,&mut worker_w,&buf,&worker_name,config.key.clone(),config.iv.clone()).await?;
                        } else {
                            log::error!("致命错误。未找到的协议{:?}",buf);
                        }

                        write_string(is_encrypted,&mut worker_w,&buf,&worker_name,config.key.clone(),config.iv.clone()).await?;
                    } else if protocol ==  PROTOCOL::NICEHASHSTRATUM {
                        if let Ok(mut result_rpc) = serde_json::from_str::<StraumResult>(&buf) {

                        } else if let Ok(mut result_rpc) = serde_json::from_str::<StraumResultBool>(&buf) {

                            if result_rpc.id == CLIENT_SUBMITWORK {
                                result_rpc.id = rpc_id;
                                if result_rpc.result == true {
                                    if proxy_fee_state == WaitStatus::WAIT{
                                        worker.share_accept();
                                    } else {
                                        //state.proxy_accept.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                                        worker.fee_share_accept();
                                        worker.share_accept();
                                    }
                                } else {
                                    if proxy_fee_state == WaitStatus::WAIT{
                                        worker.share_reject();
                                    } else {
                                        //state.proxy_reject.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                                        worker.fee_share_accept();
                                        worker.share_accept();
                                    }
                                }

                                write_rpc(is_encrypted,&mut worker_w,&result_rpc,&worker_name,config.key.clone(),config.iv.clone()).await?;
                            } else if result_rpc.id == CLIENT_LOGIN {
                                continue;
                            } else {
                                if proxy_fee_state == WaitStatus::WAIT{
                                    worker.logind();
                                    write_string(is_encrypted,&mut worker_w,&buf,&worker_name,config.key.clone(),config.iv.clone()).await?;
                                }
                            }

                            continue;
                        } else if let Ok(mut set_rpc) = serde_json::from_str::<StraumMiningSet>(&buf) {
                        } else if let Ok(mut set_rpc) = serde_json::from_str::<EthSubscriptionNotify>(&buf) {
                            if proxy_fee_state == WaitStatus::WAIT && set_rpc.id == CLIENT_LOGIN {
                                write_string(is_encrypted,&mut worker_w,&buf,&worker_name,config.key.clone(),config.iv.clone()).await?;
                            }
                            continue;
                        }
                        write_string(is_encrypted,&mut worker_w,&buf,&worker_name,config.key.clone(),config.iv.clone()).await?;
                    }
                }
            },
            () = &mut proxy_sleep  => {
                //info!("中转抽水时间片");
                if proxy_fee_state == WaitStatus::WAIT {

                    let (stream_type, pools) = match crate::client::get_pool_ip_and_type_for_proxyer(&config) {
                        Ok(s) => s,
                        Err(_) => {
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
                    if protocol == PROTOCOL::ETH {
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
                    } else if protocol == PROTOCOL::STRATUM {
                        let login = StraumRoot {
                            id: CLIENT_LOGIN,
                            method: "mining.subscribe".into(),
                            params: vec![proxy_wallet_and_worker_name.clone(), "x".into()],
                        };

                        match write_to_socket(&mut proxy_w, &login, &proxy_name).await {
                            Ok(_) => {}
                            Err(e) => {
                                log::error!("Error writing Socket {:?}", login);
                                return Err(e);
                            }
                        }
                    } else if protocol == PROTOCOL::NICEHASHSTRATUM {
                        let login = StraumRoot {
                            id: CLIENT_LOGIN,
                            method: "mining.subscribe".into(),
                            params: vec!["mining_proxy/1.0.0".into(), "EthereumStratum/1.0.0".into()],
                        };

                        match write_to_socket(&mut proxy_w, &login, &proxy_name).await {
                            Ok(_) => {}
                            Err(e) => {
                                log::error!("Error writing Socket {:?}", login);
                                return Err(e);
                            }
                        }

                        let login = StraumRoot {
                            id: CLIENT_LOGIN,
                            method: "mining.authorize".into(),
                            params: vec![proxy_wallet_and_worker_name.clone(), "x".into()],
                        };

                        match write_to_socket(&mut proxy_w, &login, &proxy_name).await {
                            Ok(_) => {}
                            Err(e) => {
                                log::error!("Error writing Socket {:?}", login);
                                return Err(e);
                            }
                        }
                    }


                    pool_lines = proxy_lines;
                    pool_w = proxy_w;

                    proxy_fee_state = WaitStatus::RUN;
                    let proxy_time = (fee_lefttime as f32 * config.share_rate) as u64;

                    //info!("{} 本次中转抽水时间为 {} 秒",worker.worker_name,proxy_time);
                    proxy_sleep.as_mut().reset(time::Instant::now() + time::Duration::from_secs(proxy_time));
                } else if proxy_fee_state == WaitStatus::RUN {
                    let (stream_type, pools) = match crate::client::get_pool_ip_and_type(&config) {
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
                    let (new_pool_r, mut new_pool_w) = tokio::io::split(stream);
                    let new_pool_r = tokio::io::BufReader::new(new_pool_r);
                    let mut new_pool_r = new_pool_r.lines();
                    if protocol == PROTOCOL::ETH {
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
                    } else if protocol == PROTOCOL::STRATUM {
                        let login = StraumRoot {
                            id: CLIENT_LOGIN,
                            method: "mining.subscribe".into(),
                            params: vec![worker.worker.clone(), "x".into()],
                        };

                        match write_to_socket(&mut new_pool_w, &login, &worker_name).await {
                            Ok(_) => {}
                            Err(e) => {
                                log::error!("Error writing Socket {:?}", login);
                                return Err(e);
                            }
                        }
                    } else if protocol == PROTOCOL::NICEHASHSTRATUM {
                        let login = StraumRoot {
                            id: CLIENT_LOGIN,
                            method: "mining.subscribe".into(),
                            params: vec!["mining_proxy/1.0.0".into(), "EthereumStratum/1.0.0".into()],
                        };

                        match write_to_socket(&mut new_pool_w, &login, &worker_name).await {
                            Ok(_) => {}
                            Err(e) => {
                                log::error!("Error writing Socket {:?}", login);
                                return Err(e);
                            }
                        }

                        let login = StraumRoot {
                            id: CLIENT_LOGIN,
                            method: "mining.authorize".into(),
                            params: vec![worker.worker.clone(), "x".into()],
                        };

                        match write_to_socket(&mut new_pool_w, &login, &worker_name).await {
                            Ok(_) => {}
                            Err(e) => {
                                log::error!("Error writing Socket {:?}", login);
                                return Err(e);
                            }
                        }
                    }



                    pool_lines = new_pool_r;
                    pool_w = new_pool_w;

                    proxy_fee_state = WaitStatus::WAIT;
                    proxy_sleep.as_mut().reset(time::Instant::now() + time::Duration::from_secs(fee_lefttime));
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
                sleep.as_mut().reset(time::Instant::now() + time::Duration::from_secs(60));
            },
        }
    }
}
