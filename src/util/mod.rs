pub mod config;
pub mod logger;
pub const TCP: i32 = 1;
pub const SSL: i32 = 2;
use log::info;

extern crate clap;
use std::{
    net::{SocketAddr, ToSocketAddrs},
    time::Duration,
};

use anyhow::Result;
use clap::{crate_authors, crate_description, crate_name, crate_version, App, Arg, ArgMatches};
use native_tls::TlsConnector;
use tokio::net::TcpStream;

pub async fn get_app_command_matches() -> Result<ArgMatches<'static>> {
    let matches = App::new(crate_name!())
        .version(crate_version!())
        .author(crate_authors!("\n"))
        .about(crate_description!())
        .arg(
            Arg::with_name("config")
                .short("c")
                .long("config")
                .value_name("FILE")
                .help("Sets a custom config file")
                .takes_value(true),
        )
        .get_matches();
    Ok(matches)
}

fn parse_hex_digit(c: char) -> Option<i64> {
    match c {
        '0' => Some(0),
        '1' => Some(1),
        '2' => Some(2),
        '3' => Some(3),
        '4' => Some(4),
        '5' => Some(5),
        '6' => Some(6),
        '7' => Some(7),
        '8' => Some(8),
        '9' => Some(9),
        'a' => Some(10),
        'b' => Some(11),
        'c' => Some(12),
        'd' => Some(13),
        'e' => Some(14),
        'f' => Some(15),
        _ => None,
    }
}

pub fn hex_to_int(string: &str) -> Option<i64> {
    let base: i64 = 16;

    string
        .chars()
        .rev()
        .enumerate()
        .fold(Some(0), |acc, (pos, c)| {
            parse_hex_digit(c).and_then(|n| acc.map(|acc| acc + n * base.pow(pos as u32)))
        })
}

pub fn calc_hash_rate(my_hash_rate: u64, share_rate: f32) -> u64 {
    ((my_hash_rate) as f32 * share_rate) as u64
}

// 从配置文件返回 连接矿池类型及连接地址
pub fn get_pool_ip_and_type(config: &config::Settings) -> Option<(i32, Vec<String>)> {
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
                info!("{} 访问不通。切换备用矿池！！！！", address);
                continue;
            }
        };

        let std_stream = match std::net::TcpStream::connect_timeout(&addr, Duration::new(2, 0)) {
            Ok(stream) => stream,
            Err(_) => {
                info!("{} 访问不通。切换备用矿池！！！！", address);
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
        info!(
            "{} conteact to {}",
            std_stream.local_addr().unwrap(),
            address
        );
        return Some((std_stream, addr));
    }

    None
}

pub async fn get_pool_stream_with_tls(
    pool_tcp_address: &Vec<String>,
    name: String,
) -> Option<(
    tokio_native_tls::TlsStream<tokio::net::TcpStream>,
    SocketAddr,
)> {
    for address in pool_tcp_address {
        let addr = match address.to_socket_addrs().unwrap().next() {
            Some(address) => address,
            None => {
                info!("{} {} 访问不通。切换备用矿池！！！！", name, address);
                continue;
            }
        };

        let std_stream = match std::net::TcpStream::connect_timeout(&addr, Duration::new(2, 0)) {
            Ok(straem) => straem,
            Err(_) => {
                info!("{} {} 访问不通。切换备用矿池！！！！", name, address);
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
                info!("{} {} 访问不通。切换备用矿池！！！！", name, address);
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
                info!("{} {} SSL 校验失败！！！！", name, address);
                continue;
            }
        };

        let cx = tokio_native_tls::TlsConnector::from(cx);

        let domain: Vec<&str> = address.split(":").collect();
        info!("{} {:?}", name, domain);
        let server_stream = match cx.connect(domain[0], stream).await {
            Ok(stream) => stream,
            Err(err) => {
                info!("{} {} SSL 链接失败！！！！ {:?}", name, address, err);
                continue;
            }
        };

        info!("{} conteactd to {}", name, address);
        return Some((server_stream, addr));
    }

    None
}
