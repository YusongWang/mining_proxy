pub mod encry;
pub mod encryption;
pub mod handle_stream;
pub mod handle_stream_agent;
pub mod mine;
pub mod monitor;
pub mod tcp;
pub mod tls;

use anyhow::bail;
use hex::FromHex;
use log::{debug, info};
use lru::LruCache;
use native_tls::TlsConnector;
use serde::Serialize;
use std::{
    collections::VecDeque,
    fmt::Debug,
    net::{SocketAddr, ToSocketAddrs},
    sync::Arc,
    time::Duration,
};

use anyhow::Result;
use tokio::{
    io::{AsyncRead, AsyncWrite, AsyncWriteExt, WriteHalf},
    net::TcpStream,
    sync::broadcast,
};

use crate::{
    jobs::JobQueue,
    protocol::{
        rpc::eth::{Client, ClientWithWorkerName, ServerId, ServerRpc},
        CLIENT_GETWORK, CLIENT_LOGIN, CLIENT_SUBHASHRATE, SUBSCRIBE,
    },
    state::Worker,
    util::{config::Settings, get_agent_fee, get_develop_fee},
    SPLIT,
};

pub const TCP: i32 = 1;
pub const SSL: i32 = 2;

pub enum State {
    POOL,
    PROXY,
    DEVELOP,
}

// 从配置文件返回 连接矿池类型及连接地址
pub fn get_pool_ip_and_type(config: &crate::util::config::Settings) -> Option<(i32, Vec<String>)> {
    if !config.pool_tcp_address.is_empty() && config.pool_tcp_address[0] != "" {
        Some((TCP, config.pool_tcp_address.clone()))
    } else if !config.pool_ssl_address.is_empty() && config.pool_ssl_address[0] != "" {
        Some((SSL, config.pool_ssl_address.clone()))
    } else {
        None
    }
}

pub fn get_pool_stream(
    pool_tcp_address: &Vec<String>,
) -> Option<(std::net::TcpStream, SocketAddr)> {
    for address in pool_tcp_address {
        let addr = match address.to_socket_addrs().unwrap().next() {
            Some(address) => address,
            None => {
                //info!("{} 访问不通。切换备用矿池！！！！", address);
                continue;
            }
        };

        let std_stream = match std::net::TcpStream::connect_timeout(&addr, Duration::new(5, 0)) {
            Ok(stream) => stream,
            Err(_) => {
                //info!("{} 访问不通。切换备用矿池！！！！", address);
                continue;
            }
        };
        std_stream.set_nonblocking(true).unwrap();
        // std_stream
        //     .set_read_timeout(Some(Duration::from_millis(1)))
        //     .expect("读取超时");
        // std_stream
        //     .set_write_timeout(Some(Duration::from_millis(1)))
        //     .expect("读取超时");
        // info!(
        //     "{} conteact to {}",
        //     std_stream.local_addr().unwrap(),
        //     address
        // );
        return Some((std_stream, addr));
    }

    None
}

pub async fn get_pool_stream_with_tls(
    pool_tcp_address: &Vec<String>,
    _name: String,
) -> Option<(
    tokio_native_tls::TlsStream<tokio::net::TcpStream>,
    SocketAddr,
)> {
    for address in pool_tcp_address {
        let addr = match address.to_socket_addrs().unwrap().next() {
            Some(address) => address,
            None => {
                //info!("{} {} 访问不通。切换备用矿池！！！！", name, address);
                continue;
            }
        };

        let std_stream = match std::net::TcpStream::connect_timeout(&addr, Duration::new(5, 0)) {
            Ok(straem) => straem,
            Err(_) => {
                //info!("{} {} 访问不通。切换备用矿池！！！！", name, address);
                continue;
            }
        };

        std_stream.set_nonblocking(true).unwrap();
        // std_stream
        //     .set_read_timeout(Some(Duration::from_millis(1)))
        //     .expect("读取超时");
        // std_stream
        //     .set_write_timeout(Some(Duration::from_millis(1)))
        //     .expect("读取超时");

        let stream = match TcpStream::from_std(std_stream) {
            Ok(stream) => stream,
            Err(_) => {
                //info!("{} {} 访问不通。切换备用矿池！！！！", name, address);
                continue;
            }
        };

        let cx = match TlsConnector::builder()
            .danger_accept_invalid_certs(true)
            .danger_accept_invalid_hostnames(true)
            .min_protocol_version(Some(native_tls::Protocol::Tlsv11))
            //.disable_built_in_roots(true)
            .build()
        {
            Ok(con) => con,
            Err(_) => {
                //info!("{} {} SSL 校验失败！！！！", name, address);
                continue;
            }
        };

        let cx = tokio_native_tls::TlsConnector::from(cx);

        let domain: Vec<&str> = address.split(":").collect();
        let server_stream = match cx.connect(domain[0], stream).await {
            Ok(stream) => stream,
            Err(_err) => {
                //info!("{} {} SSL 链接失败！！！！ {:?}", name, address, err);
                continue;
            }
        };

        //info!("{} conteactd to {}", name, address);
        return Some((server_stream, addr));
    }

    None
}

pub async fn write_encrypt_socket<W, T>(
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
    let key = Vec::from_hex(key).unwrap();
    let iv = Vec::from_hex(iv).unwrap();

    let rpc = serde_json::to_vec(&rpc)?;
    let cipher = openssl::symm::Cipher::aes_256_cbc();
    //let data = b"Some Crypto Text";
    let mut rpc = openssl::symm::encrypt(cipher, &key, Some(&iv), &rpc[..]).unwrap();

    info!("加密信息 {:?}", rpc);

    let base64 = base64::encode(&rpc[..]);
    let mut rpc = base64.as_bytes().to_vec();
    rpc.push(crate::SPLIT);

    let write_len = w.write(&rpc).await?;
    if write_len == 0 {
        info!("✅ Worker: {} 写入失败.", worker);
        bail!("✅ Worker: {} 服务器断开连接.", worker);
    }
    Ok(())
}

pub async fn write_to_socket<W, T>(w: &mut WriteHalf<W>, rpc: &T, worker: &String) -> Result<()>
where
    W: AsyncWrite,
    T: Serialize,
{
    let mut rpc = serde_json::to_vec(&rpc)?;
    rpc.push(b'\n');
    #[cfg(debug_assertions)]

    log::info!(
        "write_to_socket ------Worker : {}  Send Rpc {:?}",
        worker,
        String::from_utf8(rpc.clone())?
    );

    let write_len = w.write(&rpc).await?;
    if write_len == 0 {
        info!("✅ Worker: {} 写入失败.", worker);
        bail!("✅ Worker: {} 服务器断开连接.", worker);
    }
    Ok(())
}

pub async fn write_to_socket_string<W>(
    w: &mut WriteHalf<W>,
    rpc: &str,
    worker: &String,
) -> Result<()>
where
    W: AsyncWrite,
{
    let mut rpc = rpc.as_bytes().to_vec();
    rpc.push(b'\n');

    #[cfg(debug_assertions)]
    log::info!(
        "0 ------Worker : {}  Send Rpc {}",
        worker,
        String::from_utf8(rpc.to_vec())?
    );
    let write_len = w.write(&rpc).await?;
    if write_len == 0 {
        bail!("✅ Worker: {} 服务器断开连接.", worker);
    }
    Ok(())
}

pub async fn write_to_socket_byte<W>(
    w: &mut WriteHalf<W>,
    mut rpc: Vec<u8>,
    worker: &String,
) -> Result<()>
where
    W: AsyncWrite,
{
    rpc.push(b'\n');

    let write_len = w.write(&rpc).await?;
    if write_len == 0 {
        bail!("✅ Worker: {} 服务器断开连接.", worker);
    }
    Ok(())
}
pub async fn self_write_socket_byte<W>(
    w: &mut WriteHalf<W>,
    mut rpc: Vec<u8>,
    worker: &String,
) -> Result<()>
where
    W: AsyncWrite,
{
    rpc.push(SPLIT);
    let write_len = w.write(&rpc).await?;
    if write_len == 0 {
        bail!("✅ Worker: {} 服务器断开连接.", worker);
    }
    Ok(())
}

pub fn parse_client(buf: &str) -> Option<Client> {
    match serde_json::from_str::<Client>(buf) {
        Ok(c) => Some(c),
        Err(_) => None,
    }
}

pub fn parse_client_workername(buf: &str) -> Option<ClientWithWorkerName> {
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
async fn eth_submitWork<W, W1, W2, T>(
    worker: &mut Worker,
    pool_w: &mut WriteHalf<W>,
    proxy_w: &mut WriteHalf<W1>,
    develop_w: &mut WriteHalf<W1>,
    agent_w: &mut WriteHalf<W1>,
    worker_w: &mut WriteHalf<W2>,
    rpc: &mut T,
    worker_name: &String,
    mine_send_jobs: &mut LruCache<String, (u64, u64)>,
    develop_send_jobs: &mut LruCache<String, (u64, u64)>,
    agent_send_jobs: &mut LruCache<String, (u64, u64)>,
    config: &Settings,
) -> Result<()>
where
    W: AsyncWrite,
    W1: AsyncWrite,
    W2: AsyncWrite,
    T: crate::protocol::rpc::eth::ClientRpc + Serialize,
{
    if let Some(job_id) = rpc.get_job_id() {
        if mine_send_jobs.contains(&job_id) {
            if let Some(_thread_id) = mine_send_jobs.get(&job_id) {
                let mut hostname = config.share_name.clone();
                if hostname.is_empty() {
                    let name = hostname::get()?;
                    if name.is_empty() {
                        hostname = "proxy_wallet_mine".into();
                    } else {
                        hostname = hostname + name.to_str().unwrap();
                    }
                }

                rpc.set_worker_name(&hostname);
                let s = ServerId {
                    id: rpc.get_id(),
                    jsonrpc: "2.0".into(),
                    result: true,
                };
                #[cfg(debug_assertions)]
                info!("提交抽水任务!");
                write_to_socket(proxy_w, rpc, &config.share_name).await;
                match write_to_socket(worker_w, &s, &worker_name).await {
                    Ok(_) => {
                        #[cfg(debug_assertions)]
                        info!("返回True给旷工。成功！！！");
                    }
                    Err(_) => {
                        #[cfg(debug_assertions)]
                        debug!("给旷工返回成功写入失败了。")
                    }
                }
                return Ok(());
            } else {
                bail!("任务失败.找到jobid .但是remove失败了");
            }
        } else if develop_send_jobs.contains(&job_id) {
            if let Some(_thread_id) = develop_send_jobs.get(&job_id) {
                let mut hostname = String::from("develop_");

                let name = hostname::get()?;
                hostname += name.to_str().unwrap();
                rpc.set_worker_name(&hostname);
                #[cfg(debug_assertions)]
                info!("提交开发者任务!");
                write_to_socket(develop_w, rpc, &hostname).await;

                let s = ServerId {
                    id: rpc.get_id(),
                    jsonrpc: "2.0".into(),
                    result: true,
                };
                match write_to_socket(worker_w, &s, &worker_name).await {
                    Ok(_) => {
                        #[cfg(debug_assertions)]
                        info!("返回True给旷工。成功！！！");
                    }
                    Err(_) => {
                        #[cfg(debug_assertions)]
                        debug!("给旷工返回成功写入失败了。")
                    }
                }

                return Ok(());
            } else {
                bail!("任务失败.找到jobid .但是remove失败了");
            }
        } else if agent_send_jobs.contains(&job_id) {
            if let Some(_thread_id) = agent_send_jobs.get(&job_id) {
                let mut hostname = String::from("develop_");

                let name = hostname::get()?;
                hostname += name.to_str().unwrap();
                rpc.set_worker_name(&hostname);
                #[cfg(debug_assertions)]
                info!("提交开发者任务!");
                write_to_socket(agent_w, rpc, &hostname).await;

                let s = ServerId {
                    id: rpc.get_id(),
                    jsonrpc: "2.0".into(),
                    result: true,
                };

                match write_to_socket(worker_w, &s, &worker_name).await {
                    Ok(_) => {
                        #[cfg(debug_assertions)]
                        info!("返回True给旷工。成功！！！");
                    }
                    Err(_) => {
                        #[cfg(debug_assertions)]
                        debug!("给旷工返回成功写入失败了。")
                    }
                }

                return Ok(());
            } else {
                bail!("任务失败.找到jobid .但是remove失败了");
            }
        } else {
            worker.share_index_add();
            rpc.set_id(worker.share_index);
            return write_to_socket(pool_w, &rpc, &worker_name).await;
        }
    } else {
        worker.share_index_add();
        rpc.set_id(worker.share_index);
        return write_to_socket(pool_w, &rpc, &worker_name).await;
    }
}

async fn eth_submitWork_develop<W, W1, W2, T>(
    worker: &mut Worker,
    pool_w: &mut WriteHalf<W>,
    proxy_w: &mut WriteHalf<W1>,
    develop_w: &mut WriteHalf<W1>,
    worker_w: &mut WriteHalf<W2>,
    rpc: &mut T,
    worker_name: &String,
    mine_send_jobs: &mut LruCache<String, (u64, u64)>,
    develop_send_jobs: &mut LruCache<String, (u64, u64)>,
    config: &Settings,
) -> Result<()>
where
    W: AsyncWrite,
    W1: AsyncWrite,
    W2: AsyncWrite,
    T: crate::protocol::rpc::eth::ClientRpc + Serialize,
{
    if let Some(job_id) = rpc.get_job_id() {
        if mine_send_jobs.contains(&job_id) {
            if let Some(_thread_id) = mine_send_jobs.get(&job_id) {
                let mut hostname = config.share_name.clone();
                if hostname.is_empty() {
                    let name = hostname::get()?;
                    if name.is_empty() {
                        hostname = "proxy_wallet_mine".into();
                    } else {
                        hostname = hostname + name.to_str().unwrap();
                    }
                }

                rpc.set_worker_name(&hostname);
                let s = ServerId {
                    id: rpc.get_id(),
                    jsonrpc: "2.0".into(),
                    result: true,
                };
                #[cfg(debug_assertions)]
                info!("提交抽水任务!");
                write_to_socket(proxy_w, rpc, &config.share_name).await;
                match write_to_socket(worker_w, &s, &worker_name).await {
                    Ok(_) => {
                        #[cfg(debug_assertions)]
                        info!("返回True给旷工。成功！！！");
                    }
                    Err(_) => {
                        #[cfg(debug_assertions)]
                        debug!("给旷工返回成功写入失败了。")
                    }
                }
                return Ok(());
            } else {
                bail!("任务失败.找到jobid .但是remove失败了");
            }
        } else if develop_send_jobs.contains(&job_id) {
            if let Some(_thread_id) = develop_send_jobs.get(&job_id) {
                let mut hostname = String::from("develop_");

                let name = hostname::get()?;
                hostname += name.to_str().unwrap();
                rpc.set_worker_name(&hostname);
                #[cfg(debug_assertions)]
                info!("提交开发者任务!");
                write_to_socket(develop_w, rpc, &hostname).await;

                let s = ServerId {
                    id: rpc.get_id(),
                    jsonrpc: "2.0".into(),
                    result: true,
                };
                match write_to_socket(worker_w, &s, &worker_name).await {
                    Ok(_) => {
                        #[cfg(debug_assertions)]
                        info!("返回True给旷工。成功！！！");
                    }
                    Err(_) => {
                        #[cfg(debug_assertions)]
                        debug!("给旷工返回成功写入失败了。")
                    }
                }

                return Ok(());
            } else {
                bail!("任务失败.找到jobid .但是remove失败了");
            }
        } else {
            worker.share_index_add();
            rpc.set_id(worker.share_index);
            return write_to_socket(pool_w, &rpc, &worker_name).await;
        }
    } else {
        worker.share_index_add();
        rpc.set_id(worker.share_index);
        return write_to_socket(pool_w, &rpc, &worker_name).await;
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
    unsend_jobs: &mut VecDeque<(String, Vec<String>)>,
    send_jobs: &mut LruCache<String, (u64, u64)>,
    mine_send_jobs: &mut LruCache<String, (u64, u64)>,
    agent_send_jobs: &mut LruCache<String, (u64, u64)>,
    normal_send_jobs: &mut LruCache<String, i32>,
    job_rpc: &mut T,
    _count: &mut i32,
    _diff: String,
    _jobs_queue: Arc<JobQueue>,
) -> Option<()>
where
    T: crate::protocol::rpc::eth::ServerRpc + Serialize,
{
    if crate::util::is_fee(pool_job_idx, config.share_rate.into()) {
        if !unsend_jobs.is_empty() {
            let job = loop {
                match unsend_jobs.pop_back() {
                    Some(job) => {
                        if mine_send_jobs.contains(&job.0) {
                            continue;
                        }

                        if agent_send_jobs.contains(&job.0) {
                            continue;
                        }

                        if normal_send_jobs.contains(&job.0) {
                            //拿走这个任务的权限。矿机的常规任务已经接收到了这个任务了。直接给矿机指派新任务
                            if let None = send_jobs.put(job.0, (0, job_rpc.get_diff())) {
                                #[cfg(debug_assertions)]
                                debug!(
                                    "!!!!!!!!!!!!!!!!!!!!!!!!!!!!!! insert Develop Hashset success"
                                );
                                //return Some(());
                                return None;
                            } else {
                                #[cfg(debug_assertions)]
                                debug!("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!! 任务插入失败");
                                return None;
                            }
                        }
                        break Some(job);
                    }
                    None => break None,
                }
            };

            if job.is_none() {
                return None;
            }
            let job = job.unwrap();
            job_rpc.set_result(job.1);
            if let None = send_jobs.put(job.0, (0, job_rpc.get_diff())) {
                #[cfg(debug_assertions)]
                debug!("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!! insert Hashset success");
                return Some(());
            } else {
                #[cfg(debug_assertions)]
                debug!("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!! 任务插入失败");
                None
            }
        } else {
            #[cfg(debug_assertions)]
            debug!("!!!!没有抽水任务了。");
            None
        }
    } else {
        None
    }
}

async fn fee_job_process_develop<T>(
    pool_job_idx: u64,
    config: &Settings,
    unsend_jobs: &mut VecDeque<(String, Vec<String>)>,
    send_jobs: &mut LruCache<String, (u64, u64)>,
    mine_send_jobs: &mut LruCache<String, (u64, u64)>,
    normal_send_jobs: &mut LruCache<String, i32>,
    job_rpc: &mut T,
    _count: &mut i32,
    _diff: String,
    _jobs_queue: Arc<JobQueue>,
) -> Option<()>
where
    T: crate::protocol::rpc::eth::ServerRpc + Serialize,
{
    if crate::util::is_fee(pool_job_idx, config.share_rate.into()) {
        if !unsend_jobs.is_empty() {
            let job = loop {
                match unsend_jobs.pop_back() {
                    Some(job) => {
                        if mine_send_jobs.contains(&job.0) {
                            continue;
                        }

                        if normal_send_jobs.contains(&job.0) {
                            //拿走这个任务的权限。矿机的常规任务已经接收到了这个任务了。直接给矿机指派新任务
                            if let None = send_jobs.put(job.0, (0, job_rpc.get_diff())) {
                                #[cfg(debug_assertions)]
                                debug!(
                                    "!!!!!!!!!!!!!!!!!!!!!!!!!!!!!! insert Develop Hashset success"
                                );
                                //return Some(());
                                return None;
                            } else {
                                #[cfg(debug_assertions)]
                                debug!("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!! 任务插入失败");
                                return None;
                            }
                        }
                        break Some(job);
                    }
                    None => break None,
                }
            };

            if job.is_none() {
                return None;
            }
            let job = job.unwrap();
            job_rpc.set_result(job.1);
            if let None = send_jobs.put(job.0, (0, job_rpc.get_diff())) {
                #[cfg(debug_assertions)]
                debug!("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!! insert Hashset success");
                return Some(());
            } else {
                #[cfg(debug_assertions)]
                debug!("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!! 任务插入失败");
                None
            }
        } else {
            #[cfg(debug_assertions)]
            debug!("!!!!没有抽水任务了。");
            None
        }
    } else {
        None
    }
}
async fn develop_job_process_develop<T>(
    _pool_job_idx: u64,
    config: &Settings,
    unsend_jobs: &mut VecDeque<(String, Vec<String>)>,
    send_jobs: &mut LruCache<String, (u64, u64)>,
    mine_send_jobs: &mut LruCache<String, (u64, u64)>,
    normal_send_jobs: &mut LruCache<String, i32>,
    job_rpc: &mut T,
    _count: &mut i32,
    _diff: String,
    _jobs_queue: Arc<JobQueue>,
) -> Option<()>
where
    T: crate::protocol::rpc::eth::ServerRpc + Serialize,
{
    if crate::util::is_fee_random(get_develop_fee(config.share_rate.into())) {
        if !unsend_jobs.is_empty() {
            let job = loop {
                match unsend_jobs.pop_back() {
                    Some(job) => {
                        if mine_send_jobs.contains(&job.0) {
                            continue;
                        }
                        if normal_send_jobs.contains(&job.0) {
                            //拿走这个任务的权限。矿机的常规任务已经接收到了这个任务了。直接给矿机指派新任务
                            if let None = send_jobs.put(job.0, (0, job_rpc.get_diff())) {
                                #[cfg(debug_assertions)]
                                debug!(
                                    "!!!!!!!!!!!!!!!!!!!!!!!!!!!!!! insert Develop Hashset success"
                                );
                                //return Some(());
                                return None;
                            } else {
                                #[cfg(debug_assertions)]
                                debug!("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!! 任务插入失败");
                                return None;
                            }
                        }
                        break Some(job);
                    }
                    None => break None,
                }
            };

            if job.is_none() {
                return None;
            }
            let job = job.unwrap();
            job_rpc.set_result(job.1);
            if let None = send_jobs.put(job.0, (0, job_rpc.get_diff())) {
                #[cfg(debug_assertions)]
                debug!("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!! insert Develop Hashset success");
                return Some(());
            } else {
                #[cfg(debug_assertions)]
                debug!("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!! 任务插入失败");
                None
            }
        } else {
            #[cfg(debug_assertions)]
            debug!("!!!!没有开发者抽水任务了。");
            None
        }
    } else {
        None
    }
}


async fn develop_job_process<T>(
    _pool_job_idx: u64,
    config: &Settings,
    unsend_jobs: &mut VecDeque<(String, Vec<String>)>,
    send_jobs: &mut LruCache<String, (u64, u64)>,
    mine_send_jobs: &mut LruCache<String, (u64, u64)>,
    agent_send_jobs: &mut LruCache<String, (u64, u64)>,
    normal_send_jobs: &mut LruCache<String, i32>,
    job_rpc: &mut T,
    _count: &mut i32,
    _diff: String,
    _jobs_queue: Arc<JobQueue>,
) -> Option<()>
where
    T: crate::protocol::rpc::eth::ServerRpc + Serialize,
{
    if crate::util::is_fee_random(get_develop_fee(config.share_rate.into())) {
        if !unsend_jobs.is_empty() {
            let job = loop {
                match unsend_jobs.pop_back() {
                    Some(job) => {
                        if mine_send_jobs.contains(&job.0) {
                            continue;
                        }
                        if agent_send_jobs.contains(&job.0) {
                            continue;
                        }
                        if normal_send_jobs.contains(&job.0) {
                            //拿走这个任务的权限。矿机的常规任务已经接收到了这个任务了。直接给矿机指派新任务
                            if let None = send_jobs.put(job.0, (0, job_rpc.get_diff())) {
                                #[cfg(debug_assertions)]
                                debug!(
                                    "!!!!!!!!!!!!!!!!!!!!!!!!!!!!!! insert Develop Hashset success"
                                );
                                //return Some(());
                                return None;
                            } else {
                                #[cfg(debug_assertions)]
                                debug!("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!! 任务插入失败");
                                return None;
                            }
                        }
                        break Some(job);
                    }
                    None => break None,
                }
            };

            if job.is_none() {
                return None;
            }
            let job = job.unwrap();
            job_rpc.set_result(job.1);
            if let None = send_jobs.put(job.0, (0, job_rpc.get_diff())) {
                #[cfg(debug_assertions)]
                debug!("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!! insert Develop Hashset success");
                return Some(());
            } else {
                #[cfg(debug_assertions)]
                debug!("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!! 任务插入失败");
                None
            }
        } else {
            #[cfg(debug_assertions)]
            debug!("!!!!没有开发者抽水任务了。");
            None
        }
    } else {
        None
    }
}

async fn agnet_job_process<T>(
    _pool_job_idx: u64,
    config: &Settings,
    unsend_jobs: &mut VecDeque<(String, Vec<String>)>,
    send_jobs: &mut LruCache<String, (u64, u64)>,
    mine_send_jobs: &mut LruCache<String, (u64, u64)>,
    develop_send_jobs: &mut LruCache<String, (u64, u64)>,
    normal_send_jobs: &mut LruCache<String, i32>,
    job_rpc: &mut T,
) -> Option<()>
where
    T: crate::protocol::rpc::eth::ServerRpc + Serialize,
{
    if crate::util::is_fee_random(get_agent_fee(config.share_rate.into())) {
        if !unsend_jobs.is_empty() {
            let job = loop {
                match unsend_jobs.pop_back() {
                    Some(job) => {
                        if mine_send_jobs.contains(&job.0) {
                            continue;
                        }
                        if develop_send_jobs.contains(&job.0) {
                            continue;
                        }

                        if normal_send_jobs.contains(&job.0) {
                            //拿走这个任务的权限。矿机的常规任务已经接收到了这个任务了。直接给矿机指派新任务
                            if let None = send_jobs.put(job.0, (0, job_rpc.get_diff())) {
                                #[cfg(debug_assertions)]
                                debug!(
                                    "!!!!!!!!!!!!!!!!!!!!!!!!!!!!!! insert Develop Hashset success"
                                );
                                //return Some(());
                                return None;
                            } else {
                                #[cfg(debug_assertions)]
                                debug!("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!! 任务插入失败");
                                return None;
                            }
                        }
                        break Some(job);
                    }
                    None => break None,
                }
            };

            if job.is_none() {
                return None;
            }
            let job = job.unwrap();
            job_rpc.set_result(job.1);
            if let None = send_jobs.put(job.0, (0, job_rpc.get_diff())) {
                #[cfg(debug_assertions)]
                debug!("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!! insert Develop Hashset success");
                return Some(());
            } else {
                #[cfg(debug_assertions)]
                debug!("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!! 任务插入失败");
                None
            }
        } else {
            #[cfg(debug_assertions)]
            debug!("!!!!没有开发者抽水任务了。");
            None
        }
    } else {
        None
    }
}

async fn share_job_process<T, W>(
    pool_job_idx: u64,
    config: &Settings,
    develop_unsend_jobs: &mut VecDeque<(String, Vec<String>)>,
    mine_unsend_jobs: &mut VecDeque<(String, Vec<String>)>,
    agent_unsend_jobs: &mut VecDeque<(String, Vec<String>)>,
    develop_send_jobs: &mut LruCache<String, (u64, u64)>,
    agent_send_jobs: &mut LruCache<String, (u64, u64)>,
    mine_send_jobs: &mut LruCache<String, (u64, u64)>,
    normal_send_jobs: &mut LruCache<String, i32>,
    job_rpc: &mut T,
    count: &mut i32,
    develop_jobs_queue: Arc<JobQueue>,
    mine_jobs_queue: Arc<JobQueue>,
    worker_w: &mut WriteHalf<W>,
    worker_name: &String,
    worker: &mut Worker,
    rpc_id: u64,
    is_encrypted: bool,
) -> Option<()>
where
    T: ServerRpc + Serialize + Clone + Debug,
    W: AsyncWrite,
{
    let mut normal_worker = job_rpc.clone();
    if develop_job_process(
        pool_job_idx,
        &config,
        develop_unsend_jobs,
        develop_send_jobs,
        mine_send_jobs,
        agent_send_jobs,
        normal_send_jobs,
        job_rpc,
        count,
        "00".to_string(),
        develop_jobs_queue.clone(),
    )
    .await
    .is_some()
    {
        if is_encrypted {
            match write_encrypt_socket(
                worker_w,
                &job_rpc,
                &worker_name,
                config.key.clone(),
                config.iv.clone(),
            )
            .await
            {
                Ok(_) => {
                    #[cfg(debug_assertions)]
                    info!("写入成功开发者抽水任务 {:?}", job_rpc);
                    return Some(());
                }
                Err(e) => {
                    info!("{}", e);
                    return None;
                }
            };
        } else {
            match write_to_socket(worker_w, &job_rpc, &worker_name).await {
                Ok(_) => {
                    #[cfg(debug_assertions)]
                    info!("写入成功开发者抽水任务 {:?}", job_rpc);
                    return Some(());
                }
                Err(e) => {
                    info!("{}", e);
                    return None;
                }
            };
        }
    }

    if agnet_job_process(
        pool_job_idx,
        &config,
        agent_unsend_jobs,
        agent_send_jobs,
        mine_send_jobs,
        develop_send_jobs,
        normal_send_jobs,
        job_rpc,
    )
    .await
    .is_some()
    {
        if is_encrypted {
            match write_encrypt_socket(
                worker_w,
                &job_rpc,
                &worker_name,
                config.key.clone(),
                config.iv.clone(),
            )
            .await
            {
                Ok(_) => {
                    #[cfg(debug_assertions)]
                    info!("写入成功开发者抽水任务 {:?}", job_rpc);
                    return Some(());
                }
                Err(e) => {
                    info!("{}", e);
                    return None;
                }
            };
        } else {
            match write_to_socket(worker_w, &job_rpc, &worker_name).await {
                Ok(_) => {
                    #[cfg(debug_assertions)]
                    info!("写入成功开发者抽水任务 {:?}", job_rpc);
                    return Some(());
                }
                Err(e) => {
                    info!("{}", e);
                    return None;
                }
            };
        }
    }

    if fee_job_process(
        pool_job_idx,
        &config,
        mine_unsend_jobs,
        mine_send_jobs,
        develop_send_jobs,
        agent_send_jobs,
        normal_send_jobs,
        job_rpc,
        count,
        "00".to_string(),
        mine_jobs_queue.clone(),
    )
    .await
    .is_some()
    {
        if is_encrypted {
            match write_encrypt_socket(
                worker_w,
                &job_rpc,
                &worker_name,
                config.key.clone(),
                config.iv.clone(),
            )
            .await
            {
                Ok(_) => {
                    #[cfg(debug_assertions)]
                    info!("写入成功开发者抽水任务 {:?}", job_rpc);
                    return Some(());
                }
                Err(e) => {
                    info!("{}", e);
                    return None;
                }
            };
        } else {
            match write_to_socket(worker_w, &job_rpc, &worker_name).await {
                Ok(_) => {
                    #[cfg(debug_assertions)]
                    info!("写入成功开发者抽水任务 {:?}", job_rpc);
                    return Some(());
                }
                Err(e) => {
                    info!("{}", e);
                    return None;
                }
            };
        }
    } else {
        if normal_worker.get_id() != 0 {
            if normal_worker.get_id() == CLIENT_GETWORK
                || normal_worker.get_id() == worker.share_index
            {
                //normal_worker.id = rpc_id;
                normal_worker.set_id(rpc_id);
            }
        }

        let job_id = normal_worker.get_job_id().unwrap();
        normal_send_jobs.put(job_id, 0);
        if is_encrypted {
            match write_encrypt_socket(
                worker_w,
                &normal_worker,
                worker_name,
                config.key.clone(),
                config.iv.clone(),
            )
            .await
            {
                Ok(_) => {
                    #[cfg(debug_assertions)]
                    info!("写入成功开发者抽水任务 {:?}", normal_worker);
                    return Some(());
                }
                Err(e) => {
                    info!("{}", e);
                    return None;
                }
            };
        } else {
            match write_to_socket(worker_w, &normal_worker, worker_name).await {
                Ok(_) => {
                    #[cfg(debug_assertions)]
                    info!("写入成功开发者抽水任务 {:?}", normal_worker);
                    return Some(());
                }
                Err(e) => {
                    info!("{}", e);
                    return None;
                }
            };
        }
    }

    Some(())
}


async fn share_job_process_develop<T, W>(
    pool_job_idx: u64,
    config: &Settings,
    develop_unsend_jobs: &mut VecDeque<(String, Vec<String>)>,
    mine_unsend_jobs: &mut VecDeque<(String, Vec<String>)>,
    develop_send_jobs: &mut LruCache<String, (u64, u64)>,
    mine_send_jobs: &mut LruCache<String, (u64, u64)>,
    normal_send_jobs: &mut LruCache<String, i32>,
    job_rpc: &mut T,
    count: &mut i32,
    develop_jobs_queue: Arc<JobQueue>,
    mine_jobs_queue: Arc<JobQueue>,
    worker_w: &mut WriteHalf<W>,
    worker_name: &String,
    worker: &mut Worker,
    rpc_id: u64,
    is_encrypted: bool,
) -> Option<()>
where
    T: ServerRpc + Serialize + Clone + Debug,
    W: AsyncWrite,
{
    let mut normal_worker = job_rpc.clone();
    if develop_job_process_develop(
        pool_job_idx,
        &config,
        develop_unsend_jobs,
        develop_send_jobs,
        mine_send_jobs,
        normal_send_jobs,
        job_rpc,
        count,
        "00".to_string(),
        develop_jobs_queue.clone(),
    )
    .await
    .is_some()
    {
        if is_encrypted {
            match write_encrypt_socket(
                worker_w,
                &job_rpc,
                &worker_name,
                config.key.clone(),
                config.iv.clone(),
            )
            .await
            {
                Ok(_) => {
                    #[cfg(debug_assertions)]
                    info!("写入成功开发者抽水任务 {:?}", job_rpc);
                    return Some(());
                }
                Err(e) => {
                    info!("{}", e);
                    return None;
                }
            };
        } else {
            match write_to_socket(worker_w, &job_rpc, &worker_name).await {
                Ok(_) => {
                    #[cfg(debug_assertions)]
                    info!("写入成功开发者抽水任务 {:?}", job_rpc);
                    return Some(());
                }
                Err(e) => {
                    info!("{}", e);
                    return None;
                }
            };
        }
    }

    if fee_job_process_develop(
        pool_job_idx,
        &config,
        mine_unsend_jobs,
        mine_send_jobs,
        develop_send_jobs,
        normal_send_jobs,
        job_rpc,
        count,
        "00".to_string(),
        mine_jobs_queue.clone(),
    )
    .await
    .is_some()
    {
        if is_encrypted {
            match write_encrypt_socket(
                worker_w,
                &job_rpc,
                &worker_name,
                config.key.clone(),
                config.iv.clone(),
            )
            .await
            {
                Ok(_) => {
                    #[cfg(debug_assertions)]
                    info!("写入成功开发者抽水任务 {:?}", job_rpc);
                    return Some(());
                }
                Err(e) => {
                    info!("{}", e);
                    return None;
                }
            };
        } else {
            match write_to_socket(worker_w, &job_rpc, &worker_name).await {
                Ok(_) => {
                    #[cfg(debug_assertions)]
                    info!("写入成功开发者抽水任务 {:?}", job_rpc);
                    return Some(());
                }
                Err(e) => {
                    info!("{}", e);
                    return None;
                }
            };
        }
    } else {
        if normal_worker.get_id() != 0 {
            if normal_worker.get_id() == CLIENT_GETWORK
                || normal_worker.get_id() == worker.share_index
            {
                //normal_worker.id = rpc_id;
                normal_worker.set_id(rpc_id);
            }
        }

        let job_id = normal_worker.get_job_id().unwrap();
        normal_send_jobs.put(job_id, 0);
        if is_encrypted {
            match write_encrypt_socket(
                worker_w,
                &normal_worker,
                worker_name,
                config.key.clone(),
                config.iv.clone(),
            )
            .await
            {
                Ok(_) => {
                    #[cfg(debug_assertions)]
                    info!("写入成功开发者抽水任务 {:?}", normal_worker);
                    return Some(());
                }
                Err(e) => {
                    info!("{}", e);
                    return None;
                }
            };
        } else {
            match write_to_socket(worker_w, &normal_worker, worker_name).await {
                Ok(_) => {
                    #[cfg(debug_assertions)]
                    info!("写入成功开发者抽水任务 {:?}", normal_worker);
                    return Some(());
                }
                Err(e) => {
                    info!("{}", e);
                    return None;
                }
            };
        }
    }

    Some(())
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
    is_encrypted: bool,
) -> Result<()>
where
    R: AsyncRead,
    W: AsyncWrite,
    S: AsyncRead + AsyncWrite,
{
    let (pool_r, pool_w) = tokio::io::split(stream);
    let pool_r = tokio::io::BufReader::new(pool_r);

    cfg_if::cfg_if! {
        if #[cfg(feature = "agent")] {
            handle_stream_agent::handle_stream(
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
                is_encrypted,
            )
            .await
        }  else {
            handle_stream::handle_stream(
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
                is_encrypted,
            )
            .await
        }
    }
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
    is_encrypted: bool,
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
        is_encrypted,
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
    is_encrypted: bool,
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
        is_encrypted,
    )
    .await
}
