pub mod encry;
pub mod encryption;
pub mod fee;
pub mod handle_stream;
pub mod handle_stream_all;
pub mod handle_stream_nofee;

mod connect;
pub mod monitor;
pub mod pools;
pub mod tcp;
pub mod tls;

use anyhow::bail;
use hex::FromHex;
use native_tls::TlsConnector;
use rand::prelude::SliceRandom;
use serde::Serialize;
use std::{
    collections::VecDeque,
    fmt::Debug,
    io::{Read, Write},
    net::{SocketAddr, ToSocketAddrs},
    sync::Arc,
    time::Duration,
};
use tokio_native_tls::TlsStream;

use tracing::{debug, info};

use anyhow::Result;
use tokio::{
    io::{
        AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader,
        Lines, ReadHalf, WriteHalf,
    },
    net::TcpStream,
    sync::mpsc::UnboundedSender,
    time,
};

use crate::{
    protocol::{
        ethjson::{
            EthClientObject, EthClientRootObject, EthClientWorkerObject,
        },
        rpc::eth::{Client, ClientWithWorkerName, ServerId, ServerRpc},
        CLIENT_GETWORK, CLIENT_LOGIN, CLIENT_SUBHASHRATE, SUBSCRIBE,
    },
    proxy::Proxy,
    state::Worker,
    util::{config::Settings, get_agent_fee, get_develop_fee, get_eth_wallet},
    SPLIT,
};

pub const TCP: i32 = 1;
pub const SSL: i32 = 2;

// 从配置文件返回 连接矿池类型及连接地址
pub fn get_pool_ip_and_type(
    config: &crate::util::config::Settings,
) -> Result<(i32, Vec<String>)> {
    //FIX 兼容SSL
    if !config.pool_address.is_empty() {
        let mut pro = TCP;
        let address = config.pool_address.clone();
        let mut pools = vec![];
        for addr in address.iter() {
            let new_pool_url: Vec<&str> = addr.split("//").collect();
            if let Some(protocol) = new_pool_url.get(0) {
                let p = protocol.to_string().to_lowercase();
                if p != "tcp:" && p != "ssl:" {
                    bail!("代理矿池{} 不支持的服务类型 {}", addr, *protocol);
                }

                if p == "tcp:" {
                    pro = TCP;
                }

                if p == "ssl:" {
                    pro = SSL;
                }
            }
            if let Some(url) = new_pool_url.get(1) {
                pools.push(url.to_string());
            };
        }
        Ok((pro, pools))
    } else {
        bail!("中转池地址设置存在错误请检查");
    }
}

pub fn get_pool_ip_and_type_from_vec(
    config: &Vec<String>,
) -> Result<(i32, Vec<String>)> {
    //FIX 兼容SSL
    if !config.is_empty() {
        let mut pro = TCP;

        let address = config.clone();
        let mut pools = vec![];
        for addr in address.iter() {
            let new_pool_url: Vec<&str> = addr.split("//").collect();
            if let Some(protocol) = new_pool_url.get(0) {
                let p = protocol.to_string().to_lowercase();
                if p != "tcp:" && p != "ssl:" {
                    bail!("代理矿池{} 不支持的服务类型 {}", addr, *protocol);
                }

                if p == "tcp:" {
                    pro = TCP;
                }

                if p == "ssl:" {
                    pro = SSL;
                }
            }
            if let Some(url) = new_pool_url.get(1) {
                pools.push(url.to_string());
            };
        }

        Ok((pro, pools))
    } else {
        bail!("中转池地址设置存在错误请检查");
    }
}

// 从配置文件返回 连接矿池类型及连接地址
pub fn get_pool_ip_and_type_for_proxyer(
    config: &crate::util::config::Settings,
) -> Result<(i32, Vec<String>)> {
    //FIX 兼容ssl
    if !config.share_address.is_empty() {
        let address = config.share_address.clone();
        let mut pools = vec![];
        for addr in address.iter() {
            let new_pool_url: Vec<&str> = addr.split("//").collect();
            if let Some(protocol) = new_pool_url.get(0) {
                let p = protocol.to_string().to_lowercase();
                if p != "tcp:" {
                    bail!("抽水矿池{} 不支持的服务类型 {}", addr, *protocol);
                    //std::process::exit(1);
                }
            }
            if let Some(url) = new_pool_url.get(1) {
                pools.push(url.to_string());
            };
        }
        Ok((TCP, pools))
    } else {
        bail!("抽水矿池地址设置存在错误请检查");
    }
}
//vs.choose(&mut rand::thread_rng())
pub fn get_pool_random_stream(
    pool_tcp_address: &Vec<String>,
) -> Option<(std::net::TcpStream, SocketAddr)> {
    for _ in 0..pool_tcp_address.len() {
        let address = match pool_tcp_address.choose(&mut rand::thread_rng()) {
            Some(s) => s,
            None => continue,
        };

        let mut tcp = match address.to_socket_addrs() {
            Ok(t) => t,
            Err(_) => {
                tracing::error!("矿池地址格式化失败");
                continue;
            }
        };

        let addr = match tcp.next() {
            Some(address) => address,
            None => {
                //debug!("{} 访问不通。切换备用矿池！！！！", address);
                continue;
            }
        };

        let std_stream = match std::net::TcpStream::connect_timeout(
            &addr,
            Duration::new(60, 0),
        ) {
            Ok(stream) => stream,
            Err(_) => {
                //debug!("{} 访问不通。切换备用矿池！！！！", address);
                continue;
            }
        };
        std_stream.set_nonblocking(true).unwrap();
        return Some((std_stream, addr));
    }

    None
}

pub fn get_pool_stream(
    pool_tcp_address: &Vec<String>,
) -> Option<(std::net::TcpStream, SocketAddr)> {
    for address in pool_tcp_address {
        let mut tcp = match address.to_socket_addrs() {
            Ok(t) => t,
            Err(_) => {
                tracing::error!("矿池地址格式化失败 {}", address);
                continue;
            }
        };

        let addr = match tcp.next() {
            Some(address) => address,
            None => {
                //debug!("{} 访问不通。切换备用矿池！！！！", address);
                continue;
            }
        };

        let std_stream = match std::net::TcpStream::connect_timeout(
            &addr,
            Duration::new(60, 0),
        ) {
            Ok(stream) => stream,
            Err(_) => {
                //debug!("{} 访问不通。切换备用矿池！！！！", address);
                continue;
            }
        };
        std_stream.set_nonblocking(true).unwrap();
        return Some((std_stream, addr));
    }

    None
}

pub async fn get_pool_stream_with_tls(
    pool_tcp_address: &Vec<String>,
) -> Option<(
    tokio_native_tls::TlsStream<tokio::net::TcpStream>,
    SocketAddr,
)> {
    for address in pool_tcp_address {
        let mut tcp = match address.to_socket_addrs() {
            Ok(t) => t,
            Err(_) => {
                tracing::error!("矿池地址格式化失败 {}", address);
                continue;
            }
        };

        let addr = match tcp.next() {
            Some(address) => address,
            None => {
                //debug!("{} {} 访问不通。切换备用矿池！！！！", name,
                // address);
                continue;
            }
        };

        let std_stream = match std::net::TcpStream::connect_timeout(
            &addr,
            Duration::new(60, 0),
        ) {
            Ok(straem) => straem,
            Err(_) => {
                //debug!("{} {} 访问不通。切换备用矿池！！！！", name,
                // address);
                continue;
            }
        };

        std_stream.set_nonblocking(true).unwrap();

        let stream = match TcpStream::from_std(std_stream) {
            Ok(stream) => stream,
            Err(_) => {
                //debug!("{} {} 访问不通。切换备用矿池！！！！", name,
                // address);
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
                //debug!("{} {} SSL 校验失败！！！！", name, address);
                continue;
            }
        };

        let cx = tokio_native_tls::TlsConnector::from(cx);

        let domain: Vec<&str> = address.split(":").collect();
        let server_stream = match cx.connect(domain[0], stream).await {
            Ok(stream) => stream,
            Err(_err) => {
                //debug!("{} {} SSL 链接失败！！！！ {:?}", name, address,
                // err);
                continue;
            }
        };

        //debug!("{} conteactd to {}", name, address);
        return Some((server_stream, addr));
    }

    None
}

pub async fn write_encrypt_socket<W, T>(
    w: &mut WriteHalf<W>, rpc: &T, worker: &String, key: String, iv: String,
) -> Result<()>
where
    W: AsyncWrite,
    T: Serialize,
{
    let key = Vec::from_hex(key).unwrap();
    let iv = Vec::from_hex(iv).unwrap();

    let rpc = serde_json::to_vec(&rpc)?;
    let cipher = openssl::symm::Cipher::aes_256_cbc();

    let rpc =
        openssl::symm::encrypt(cipher, &key, Some(&iv), &rpc[..]).unwrap();

    let base64 = base64::encode(&rpc[..]);
    let mut rpc = base64.as_bytes().to_vec();
    rpc.push(crate::SPLIT);

    let write_len = w.write(&rpc).await?;
    if write_len == 0 {
        bail!(
            "旷工: {} 服务器断开连接. 写入失败。远程矿池未连通！",
            worker
        );
    }
    Ok(())
}

pub async fn write_encrypt_socket_string<W>(
    w: &mut WriteHalf<W>, rpc: &str, worker: &String, key: String, iv: String,
) -> Result<()>
where W: AsyncWrite {
    let key = Vec::from_hex(key).unwrap();
    let iv = Vec::from_hex(iv).unwrap();

    let rpc = rpc.as_bytes().to_vec();
    let cipher = openssl::symm::Cipher::aes_256_cbc();
    //let data = b"Some Crypto String";
    let rpc =
        openssl::symm::encrypt(cipher, &key, Some(&iv), &rpc[..]).unwrap();

    let base64 = base64::encode(&rpc[..]);
    let mut rpc = base64.as_bytes().to_vec();
    rpc.push(crate::SPLIT);

    let write_len = w.write(&rpc).await?;
    if write_len == 0 {
        bail!(
            "旷工: {} 服务器断开连接. 写入失败。远程矿池未连通！",
            worker
        );
    }
    Ok(())
}

pub async fn write_to_socket<W, T>(
    w: &mut WriteHalf<W>, rpc: &T, worker: &String,
) -> Result<()>
where
    W: AsyncWrite,
    T: Serialize,
{
    let mut rpc = serde_json::to_vec(&rpc)?;
    rpc.push(b'\n');
    #[cfg(debug_assertions)]
    tracing::debug!(
        "write_to_socket ------Worker : {}  Send Rpc {:?}",
        worker,
        String::from_utf8(rpc.clone())?
    );

    let write_len = w.write(&rpc).await?;
    if write_len == 0 {
        bail!(
            "旷工: {} 服务器断开连接. 写入失败。远程矿池未连通！",
            worker
        );
    }
    Ok(())
}

pub async fn write_to_socket_string<W>(
    w: &mut WriteHalf<W>, rpc: &str, worker: &String,
) -> Result<()>
where W: AsyncWrite {
    let mut rpc = rpc.as_bytes().to_vec();
    rpc.push(b'\n');

    #[cfg(debug_assertions)]
    tracing::debug!(
        "0 ------Worker : {}  Send Rpc {}",
        worker,
        String::from_utf8(rpc.to_vec())?
    );
    let write_len = w.write(&rpc).await?;
    if write_len == 0 {
        bail!(
            "旷工: {} 服务器断开连接. 写入失败。远程矿池未连通！",
            worker
        );
    }
    Ok(())
}

pub async fn write_to_socket_byte<W>(
    w: &mut WriteHalf<W>, mut rpc: Vec<u8>, worker: &String,
) -> Result<()>
where W: AsyncWrite {
    rpc.push(b'\n');
    let write_len = w.write(&rpc).await?;
    if write_len == 0 {
        bail!(
            "旷工: {} 服务器断开连接. 写入失败。远程矿池未连通！",
            worker
        );
    }
    Ok(())
}

pub async fn self_write_socket_byte<W>(
    w: &mut WriteHalf<W>, mut rpc: Vec<u8>, worker: &String,
) -> Result<()>
where W: AsyncWrite {
    rpc.push(SPLIT);
    let write_len = w.write(&rpc).await?;
    if write_len == 0 {
        bail!(
            "旷工: {} 服务器断开连接. 写入失败。远程矿池未连通！",
            worker
        );
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

pub fn parse(buf: &[u8]) -> Option<Box<dyn EthClientObject + Send + Sync>> {
    if let Ok(c) = serde_json::from_slice::<EthClientWorkerObject>(buf) {
        Some(Box::new(c))
    } else if let Ok(c) = serde_json::from_slice::<EthClientRootObject>(buf) {
        Some(Box::new(c))
    } else {
        None
    }
}

pub fn parse_workername(buf: &[u8]) -> Option<ClientWithWorkerName> {
    match serde_json::from_slice::<ClientWithWorkerName>(buf) {
        Ok(c) => Some(c),
        Err(_) => None,
    }
}
async fn eth_submit_login<W, T>(
    worker: &mut Worker, w: &mut WriteHalf<W>, rpc: &mut T,
    worker_name: &mut String,
) -> Result<()>
where
    W: AsyncWrite,
    T: crate::protocol::rpc::eth::ClientRpc + Serialize,
{
    if let Some(wallet) = rpc.get_eth_wallet() {
        //rpc.id = CLIENT_LOGIN;
        rpc.set_id(CLIENT_LOGIN);
        let mut temp_worker = wallet.clone();
        temp_worker.push_str(".");
        temp_worker = temp_worker + rpc.get_worker_name().as_str();
        worker.login(
            temp_worker.clone(),
            rpc.get_worker_name(),
            wallet.clone(),
        );
        *worker_name = temp_worker;
        write_to_socket(w, &rpc, &worker_name).await
    } else {
        bail!("请求登录出错。可能收到暴力攻击");
    }
}

pub async fn handle_tcp<R, W>(
    worker: &mut Worker, worker_queue: UnboundedSender<Worker>,
    worker_r: tokio::io::BufReader<tokio::io::ReadHalf<R>>,
    worker_w: WriteHalf<W>, stream: TcpStream, config: &Settings,
    is_encrypted: bool,
) -> Result<()>
where
    R: AsyncRead,
    W: AsyncWrite,
{
    let (pool_r, pool_w) = tokio::io::split(stream);
    let pool_r = tokio::io::BufReader::new(pool_r);

    handle_stream_nofee::handle_stream(
        worker,
        worker_queue,
        worker_r,
        worker_w,
        pool_r,
        pool_w,
        &config,
        is_encrypted,
    )
    .await
}
pub async fn handle_tcp_random<R, W>(
    worker: &mut Worker,
    worker_r: tokio::io::BufReader<tokio::io::ReadHalf<R>>,
    worker_w: WriteHalf<W>, pools: &Vec<String>, proxy: Arc<Proxy>,
    stream_type: i32, is_encrypted: bool,
) -> Result<()>
where
    R: AsyncRead,
    W: AsyncWrite,
{
    if stream_type == TCP {
        let (outbound, _) = match crate::client::get_pool_stream(&pools) {
            Some((stream, addr)) => (stream, addr),
            None => {
                bail!("所有TCP矿池均不可链接。请修改后重试");
            }
        };

        let stream = tokio::net::TcpStream::from_std(outbound)?;
        let (pool_r, pool_w) = tokio::io::split(stream);
        let pool_r = tokio::io::BufReader::new(pool_r);

        handle_stream::handle_stream(
            worker,
            worker_r,
            worker_w,
            pool_r,
            pool_w,
            proxy,
            is_encrypted,
        )
        .await
    } else if stream_type == SSL {
        let (stream, _) =
            match crate::client::get_pool_stream_with_tls(&pools).await {
                Some((stream, addr)) => (stream, addr),
                None => {
                    bail!("所有TCP矿池均不可链接。请修改后重试");
                }
            };

        let (pool_r, pool_w) = tokio::io::split(stream);
        let pool_r = tokio::io::BufReader::new(pool_r);

        handle_stream::handle_stream(
            worker,
            worker_r,
            worker_w,
            pool_r,
            pool_w,
            proxy,
            is_encrypted,
        )
        .await
    } else {
        panic!("达到了无法达到的分支");
    }
}

// pub async fn handle_tcp_timer<R, W>(
//     worker: &mut Worker, worker_queue: UnboundedSender<Worker>,
//     worker_r: tokio::io::BufReader<tokio::io::ReadHalf<R>>,
//     worker_w: WriteHalf<W>, stream: TcpStream, config: &Settings,
//     is_encrypted: bool,
// ) -> Result<()>
// where
//     R: AsyncRead,
//     W: AsyncWrite,
// {
//     let (pool_r, pool_w) = tokio::io::split(stream);
//     let pool_r = tokio::io::BufReader::new(pool_r);
//     handle_stream_timer::handle_stream(
//         worker,
//         worker_queue,
//         worker_r,
//         worker_w,
//         pool_r,
//         pool_w,
//         &config,
//         is_encrypted,
//     )
//     .await
// }

pub async fn handle_tcp_all<R, W>(
    worker: &mut Worker, worker_queue: UnboundedSender<Worker>,
    worker_r: tokio::io::BufReader<tokio::io::ReadHalf<R>>,
    worker_w: WriteHalf<W>, stream: TcpStream, config: &Settings,
    is_encrypted: bool,
) -> Result<()>
where
    R: AsyncRead,
    W: AsyncWrite,
{
    let (pool_r, pool_w) = tokio::io::split(stream);
    let pool_r = tokio::io::BufReader::new(pool_r);

    handle_stream_all::handle_stream(
        worker,
        worker_queue,
        worker_r,
        worker_w,
        pool_r,
        pool_w,
        &config,
        is_encrypted,
    )
    .await
}

pub async fn handle_tcp_pool<R, W>(
    worker: &mut Worker, worker_queue: UnboundedSender<Worker>,
    worker_r: tokio::io::BufReader<tokio::io::ReadHalf<R>>,
    worker_w: WriteHalf<W>, pools: &Vec<String>, config: &Settings,
    is_encrypted: bool,
) -> Result<()>
where
    R: AsyncRead,
    W: AsyncWrite,
{
    let (outbound, _) = match crate::client::get_pool_stream(&pools) {
        Some((stream, addr)) => (stream, addr),
        None => {
            bail!("所有TCP矿池均不可链接。请修改后重试");
        }
    };

    let stream = TcpStream::from_std(outbound)?;
    handle_tcp(
        worker,
        worker_queue,
        worker_r,
        worker_w,
        stream,
        &config,
        is_encrypted,
    )
    .await
}

// pub async fn handle_tcp_pool_timer<R, W>(
//     worker: &mut Worker, worker_queue: UnboundedSender<Worker>,
//     worker_r: tokio::io::BufReader<tokio::io::ReadHalf<R>>,
//     worker_w: WriteHalf<W>, pools: &Vec<String>, config: &Settings,
//     is_encrypted: bool,
// ) -> Result<()>
// where
//     R: AsyncRead,
//     W: AsyncWrite,
// {
//     let (outbound, _) = match crate::client::get_pool_stream(&pools) {
//         Some((stream, addr)) => (stream, addr),
//         None => {
//             bail!("所有TCP矿池均不可链接。请修改后重试");
//         }
//     };

//     let stream = TcpStream::from_std(outbound)?;
//     handle_tcp_timer(
//         worker,
//         worker_queue,
//         worker_r,
//         worker_w,
//         stream,
//         &config,
//         is_encrypted,
//     )
//     .await
// }

pub async fn handle_tcp_pool_all<R, W>(
    worker: &mut Worker, worker_queue: UnboundedSender<Worker>,
    worker_r: tokio::io::BufReader<tokio::io::ReadHalf<R>>,
    worker_w: WriteHalf<W>, config: &Settings, is_encrypted: bool,
) -> Result<()>
where
    R: AsyncRead,
    W: AsyncWrite,
{
    let (stream_type, pools) =
        match crate::client::get_pool_ip_and_type_for_proxyer(&config) {
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

    handle_tcp_all(
        worker,
        worker_queue,
        worker_r,
        worker_w,
        stream,
        &config,
        is_encrypted,
    )
    .await
}

// pub async fn handle_tls_pool<R, W>(
//     worker: &mut Worker, worker_queue: UnboundedSender<Worker>,
//     worker_r: tokio::io::BufReader<tokio::io::ReadHalf<R>>,
//     worker_w: WriteHalf<W>, pools: &Vec<String>, config: &Settings,
//     is_encrypted: bool,
// ) -> Result<()>
// where
//     R: AsyncRead,
//     W: AsyncWrite,
// {
//     let (outbound, _) =
//         match crate::client::get_pool_stream_with_tls(&pools, "proxy".into())
//             .await
//         {
//             Some((stream, addr)) => (stream, addr),
//             None => {
//                 bail!("所有SSL矿池均不可链接。请修改后重试");
//             }
//         };

//     handle_ssl(
//         worker,
//         worker_queue,
//         worker_r,
//         worker_w,
//         outbound,
//         &config,
//
//         is_encrypted,
//     )
//     .await;

//     Ok(())
// }

pub fn job_diff_change<T>(
    diff: &mut u64, rpc: &T, a: &mut VecDeque<(String, Vec<String>)>,
    b: &mut VecDeque<(String, Vec<String>)>,
    c: &mut VecDeque<(String, Vec<String>)>, mine_send_jobs: &mut Vec<String>,
    develop_send_jobs: &mut Vec<String>, proxy_send_jobs: &mut Vec<String>,
    normal_send_jobs: &mut Vec<String>,
) -> bool
where
    T: ServerRpc,
{
    let job_diff = rpc.get_diff();
    if job_diff > *diff {
        // 写入新难度
        *diff = job_diff;

        // 清空已有任务队列
        a.clear();
        b.clear();
        c.clear();

        // 清空已发送任务。这个时候之后发送的任务都已经超时了。
        mine_send_jobs.clear();
        develop_send_jobs.clear();
        proxy_send_jobs.clear();
        normal_send_jobs.clear();
    }

    true
}

pub async fn submit_fee_hashrate(
    config: &Settings, hashrate: u64,
) -> Result<()> {
    let (stream, _) =
        match crate::client::get_pool_stream(&config.share_address) {
            Some((stream, addr)) => (stream, addr),
            None => {
                tracing::error!("所有TCP矿池均不可链接。请修改后重试");
                bail!("所有TCP矿池均不可链接。请修改后重试");
            }
        };

    let outbound = TcpStream::from_std(stream)?;
    let (proxy_r, mut proxy_w) = tokio::io::split(outbound);
    let _proxy_r = tokio::io::BufReader::new(proxy_r);

    let hostname = config.get_share_name().unwrap();

    let login = ClientWithWorkerName {
        id: CLIENT_LOGIN,
        method: "eth_submitLogin".into(),
        params: vec![config.share_wallet.clone(), "x".into()],
        worker: hostname.clone(),
    };
    write_to_socket(&mut proxy_w, &login, &hostname).await;
    //计算速率
    let submit_hashrate = ClientWithWorkerName {
        id: CLIENT_SUBHASHRATE,
        method: "eth_submitHashrate".into(),
        params: [format!("0x{:x}", hashrate), hex::encode(hostname.clone())]
            .to_vec(),
        worker: hostname.clone(),
    };
    write_to_socket(&mut proxy_w, &submit_hashrate, &hostname).await;
    Ok(())
}

pub async fn submit_develop_hashrate(
    _config: &Settings, hashrate: u64,
) -> Result<()> {
    let stream = match pools::get_develop_pool_stream().await {
        Ok(s) => s,
        Err(e) => return Err(e),
    };

    let outbound = TcpStream::from_std(stream)?;
    let (_, mut proxy_w) = tokio::io::split(outbound);

    let mut hostname = String::from("develop_");
    let name = hostname::get()?;
    hostname += name.to_str().unwrap();

    let login = ClientWithWorkerName {
        id: CLIENT_LOGIN,
        method: "eth_submitLogin".into(),
        params: vec![get_eth_wallet(), "x".into()],
        worker: hostname.clone(),
    };

    write_to_socket(&mut proxy_w, &login, &hostname).await;
    //计算速率
    let submit_hashrate = ClientWithWorkerName {
        id: CLIENT_SUBHASHRATE,
        method: "eth_submitHashrate".into(),
        params: [format!("0x{:x}", hashrate), hex::encode(hostname.clone())]
            .to_vec(),
        worker: hostname.clone(),
    };
    write_to_socket(&mut proxy_w, &submit_hashrate, &hostname).await;
    Ok(())
}

// new -----------------------------------------------------------------
pub async fn proxy_pool_login(
    config: &Settings, hostname: String,
) -> Result<(Lines<BufReader<ReadHalf<TcpStream>>>, WriteHalf<TcpStream>)> {
    //TODO 这里要兼容SSL矿池
    let (stream_type, pools) =
        match crate::client::get_pool_ip_and_type_from_vec(
            &config.share_address,
        ) {
            Ok((stream, addr)) => (stream, addr),
            Err(e) => {
                tracing::error!("所有TCP矿池均不可链接。请修改后重试");
                bail!("所有TCP矿池均不可链接。请修改后重试");
            }
        };

    let (stream, _) = match crate::client::get_pool_stream(&pools) {
        Some((stream, addr)) => (stream, addr),
        None => {
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
            tracing::error!("Error writing Socket {:?}", login);
            return Err(e);
        }
    }

    Ok((proxy_lines, proxy_w))
}

pub async fn dev_pool_tcp_login(
    hostname: String,
) -> Result<(Lines<BufReader<ReadHalf<TcpStream>>>, WriteHalf<TcpStream>)> {
    let pools = vec![
        "asia2.ethermine.org:4444".to_string(),
        "asia1.ethermine.org:4444".to_string(),
    ];
    // let pools = vec![
    //     "eth-hk.flexpool.io:13271".to_string(),
    //     "eth-hke.flexpool.io:13271".to_string(),
    //     "hke.fpmirror.com:13271".to_string(),
    //     "eth-hk.flexpool.io:4444".to_string(),
    //     "eth-hke.flexpool.io:4444".to_string(),
    //     "hke.fpmirror.com:4444".to_string(),
    // ];

    let (stream, _) = match crate::client::get_pool_stream(&pools) {
        Some((stream, addr)) => (stream, addr),
        None => {
            bail!("所有TCP矿池均不可链接。请修改后重试");
        }
    };

    let (proxy_r, mut proxy_w) =
        tokio::io::split(tokio::net::TcpStream::from_std(stream)?);
    let proxy_r = tokio::io::BufReader::new(proxy_r);
    let mut proxy_lines = proxy_r.lines();

    let login = ClientWithWorkerName {
        id: CLIENT_LOGIN,
        method: "eth_submitLogin".into(),
        params: vec![
            "0x3602b50d3086edefcd9318bcceb6389004fb14ee".into(),
            "x".into(),
        ],
        worker: hostname.clone(),
    };

    match write_to_socket(&mut proxy_w, &login, &hostname).await {
        Ok(_) => {}
        Err(e) => {
            tracing::error!("Error writing Socket {:?}", login);
            return Err(e);
        }
    }

    Ok((proxy_lines, proxy_w))
}

pub async fn dev_pool_ssl_login(
    hostname: String,
) -> Result<(
    Lines<BufReader<ReadHalf<tokio_native_tls::TlsStream<TcpStream>>>>,
    WriteHalf<TlsStream<TcpStream>>,
)> {
    // let pools = vec![
    //     "asia2.ethermine.org:5555".to_string(),
    //     "asia1.ethermine.org:5555".to_string(),
    // ];

    let pools = vec![
        "eth-hk.flexpool.io:22271".to_string(),
        "eth-hke.flexpool.io:22271".to_string(),
        "hke.fpmirror.com:22271".to_string(),
        "eth-sg.flexpool.io:22271".to_string(),
        "eth-hk.flexpool.io:5555".to_string(),
        "eth-hke.flexpool.io:5555".to_string(),
        "hke.fpmirror.com:5555".to_string(),
        "eth-sg.flexpool.io:5555".to_string(),
    ];

    let (stream, _) =
        match crate::client::get_pool_stream_with_tls(&pools).await {
            Some((stream, addr)) => (stream, addr),
            None => {
                bail!("所有矿池均不可链接。请修改后重试");
            }
        };

    let (proxy_r, mut proxy_w) = tokio::io::split(stream);
    let proxy_r = tokio::io::BufReader::new(proxy_r);
    let proxy_lines = proxy_r.lines();

    let login = ClientWithWorkerName {
        id: CLIENT_LOGIN,
        method: "eth_submitLogin".into(),
        params: vec![
            "0xBC9fB4fD559217715d090975D5fF8FcDFc172345".into(),
            "x".into(),
        ],
        worker: hostname.clone(),
    };

    match write_to_socket(&mut proxy_w, &login, &hostname).await {
        Ok(_) => {}
        Err(e) => {
            tracing::error!("Error writing Socket {:?}", login);
            return Err(e);
        }
    }

    Ok((proxy_lines, proxy_w))
}

pub async fn lines_unwrap(
    res: Result<Option<String>, std::io::Error>, worker_name: &String,
    form_name: &str,
) -> Result<String> {
    let buffer = match res {
        Ok(res) => match res {
            Some(buf) => Ok(buf),
            None => {
                bail!(
                    "{}：{}  读取到字节0. 矿池主动断开 ",
                    form_name,
                    worker_name
                );
            }
        },
        Err(e) => {
            bail!("{}：{} 读取错误: {} ", form_name, worker_name, e);
        }
    };

    buffer
}

pub async fn seagment_unwrap<W>(
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
                        tracing::error!("Error Shutdown Socket {:?}", e);
                    }
                }
                bail!("矿工：{}  读取到字节0.矿工主动断开 ", worker_name);
            }
        },
        Err(e) => {
            match pool_w.shutdown().await {
                Ok(_) => {}
                Err(e) => {
                    tracing::error!("Error Shutdown Socket {:?}", e);
                }
            }
            bail!("矿工：{} {}", worker_name, e);
        }
    };

    byte_buffer
}

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
    // tracing::warn!("端口可能被恶意扫描: {}", buf);
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

//中转费率及开发者费率
#[derive(Debug)]
pub enum FEE {
    PROXYFEE(Box<dyn EthClientObject + Send + Sync>),
    DEVFEE(Box<dyn EthClientObject + Send + Sync>),
}
