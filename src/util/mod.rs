pub mod config;

extern crate clap;
use std::{
    net::{SocketAddr, ToSocketAddrs},
    time::Duration,
};

use anyhow::Result;
use clap::{crate_authors, crate_description, crate_name, crate_version, App, Arg, ArgMatches};
use log::info;
use native_tls::{TlsConnector, Protocol};
use tokio::net::TcpStream;
use tokio_native_tls::TlsStream;

use self::config::Settings;

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
pub mod logger {

    pub fn init(app_name: &str, path: String, log_level: u32) -> Result<(), fern::InitError> {
        // parse log_laver
        let lavel = match log_level {
            3 => log::LevelFilter::Error,
            2 => log::LevelFilter::Info,
            1 => log::LevelFilter::Debug,
            _ => log::LevelFilter::Info,
        };
        if log_level <= 1 {
            let log = fern::DateBased::new(path, format!("{}.log.%Y-%m-%d.%H", app_name))
                .utc_time()
                .local_time();
            fern::Dispatch::new()
                .format(move |out, message, record| {
                    out.finish(format_args!(
                        "[{}] [{}] [{}:{}] [{}] {}",
                        chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                        record.target(),
                        record.file().unwrap(),
                        record.line().unwrap(),
                        record.level(),
                        message
                    ))
                })
                .level(lavel)
                //.level_for("engine", log::LevelFilter::Debug)
                .chain(std::io::stdout())
                .chain(log)
                .apply()?;
        } else {
            let log = fern::DateBased::new(path, format!("{}.log.%Y-%m-%d.%H", app_name))
                .utc_time()
                .local_time();
            fern::Dispatch::new()
                .format(move |out, message, record| {
                    out.finish(format_args!(
                        "[{}] [{}] {}",
                        chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                        record.level(),
                        message
                    ))
                })
                .level(lavel)
                //.level_for("engine", log::LevelFilter::Debug)
                .chain(std::io::stdout())
                .chain(log)
                .apply()?;
        }

        Ok(())
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

        let std_stream = match std::net::TcpStream::connect_timeout(&addr, Duration::new(3, 0)) {
            Ok(straem) => straem,
            Err(_) => {
                info!("{} 访问不通。切换备用矿池！！！！", address);
                continue;
            }
        };
        std_stream.set_nonblocking(true).unwrap();

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
) -> Option<(TlsStream<TcpStream>, SocketAddr)> {
    for address in pool_tcp_address {
        let addr = match address.to_socket_addrs().unwrap().next() {
            Some(address) => address,
            None => {
                info!("{} 访问不通。切换备用矿池！！！！", address);
                continue;
            }
        };

        let std_stream = match std::net::TcpStream::connect_timeout(&addr, Duration::new(3, 0)) {
            Ok(straem) => straem,
            Err(_) => {
                info!("{} 访问不通。切换备用矿池！！！！", address);
                continue;
            }
        };
        std_stream.set_nonblocking(true).unwrap();
        let stream = match TcpStream::from_std(std_stream) {
            Ok(stream) => stream,
            Err(_) => {
                info!("{} 访问不通。切换备用矿池！！！！", address);
                continue;
            }
        };

        let cx = match TlsConnector::builder()
            .danger_accept_invalid_certs(true)
            .danger_accept_invalid_hostnames(true)
            .min_protocol_version(Some(Protocol::Tlsv12))
            //.max_protocol_version(Some(Protocol::Sslv3))
            .build()
        {
            Ok(con) => con,
            Err(_) => {
                info!("{} SSL 校验失败！！！！", address);
                continue;
            }
        };
        let cx = tokio_native_tls::TlsConnector::from(cx);
        let addr_str = addr.to_string();
        let domain: Vec<&str> = addr_str.split(":").collect();
        info!("{:?}",domain);
        let server_stream = match cx.connect(domain[0], stream).await {
            Ok(stream) => stream,
            Err(err) => {
                info!("{} SSL 链接失败！！！！ {:?}", address, err);
                continue;
            }
        };

        info!("conteactd to {}", address);
        return Some((server_stream, addr));
    }

    None
}
