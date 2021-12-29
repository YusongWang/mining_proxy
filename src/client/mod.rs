pub mod handle_stream;
pub mod mine;
pub mod tcp;
pub mod tls;

use anyhow::bail;
use log::{debug, info};
use native_tls::TlsConnector;
use serde::Serialize;
use std::{
    collections::{HashMap, VecDeque},
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
        rpc::eth::{Client, ClientWithWorkerName, Server, ServerId1, ServerRpc},
        CLIENT_GETWORK, CLIENT_LOGIN, CLIENT_SUBHASHRATE, SUBSCRIBE,
    },
    state::Worker,
    util::config::Settings,
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

        let std_stream = match std::net::TcpStream::connect_timeout(&addr, Duration::new(2, 0)) {
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

        let std_stream = match std::net::TcpStream::connect_timeout(&addr, Duration::new(2, 0)) {
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

pub async fn write_to_socket<W, T>(w: &mut WriteHalf<W>, rpc: &T, worker: &String) -> Result<()>
where
    W: AsyncWrite,
    T: Serialize,
{
    let mut rpc = serde_json::to_vec(&rpc)?;
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
    worker_w: &mut WriteHalf<W2>,
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
    W2: AsyncWrite,
    T: crate::protocol::rpc::eth::ClientRpc + Serialize,
{
    worker.share_index_add();

    if let Some(job_id) = rpc.get_job_id() {
        if mine_send_jobs.contains_key(&job_id) {
            if let Some(thread_id) = mine_send_jobs.remove(&job_id) {
                let rpc_string = serde_json::to_string(&rpc)?;
                //rpc.set_workname(config.share_);

                // proxy_fee_sender
                //     .send((thread_id.0, rpc_string))
                //     .expect("可以提交给矿池任务失败。通道异常了");
                write_to_socket(proxy_w, rpc, worker_name).await;
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

            // let mut res = mine_send_job.2.result.clone();
            // res[2] = "proxy".into();
            // job_rpc.set_result(res);
            job_rpc.set_result(mine_send_job.2.result);
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
                        // let mut res = rpc.result.clone();
                        // res[2] = "proxy".into();
                        // job_rpc.set_result(res);
                        job_rpc.set_result(rpc.result.clone());
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
    if crate::util::is_fee_random(crate::FEE.into()) {
        if !unsend_jobs.is_empty() {
            let mine_send_job = unsend_jobs.pop_back().unwrap();
            //let job_rpc = serde_json::from_str::<Server>(&*job.1)?;
            // let mut res = mine_send_job.2.result.clone();
            // res[2] = "develop".into();
            // job_rpc.set_result(res);
            job_rpc.set_result(mine_send_job.2.result);
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
                        // let mut res = rpc.result.clone();
                        // res[2] = "develop".into();
                        //job_rpc.set_result(res);
                        job_rpc.set_result(rpc.result.clone());
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
