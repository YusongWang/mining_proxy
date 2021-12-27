pub mod handle_stream;
pub mod tcp;
pub mod tls;

use anyhow::bail;
use native_tls::TlsConnector;
use serde::Serialize;
use std::{
    net::{SocketAddr, ToSocketAddrs},
    time::Duration,
};

use anyhow::Result;
use tokio::{
    io::{AsyncWrite, AsyncWriteExt, WriteHalf},
    net::TcpStream,
};

use crate::protocol::rpc::eth::{Client, ClientWithWorkerName};

pub const TCP: i32 = 1;
pub const SSL: i32 = 2;

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
