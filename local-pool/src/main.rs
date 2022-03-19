use anyhow::{anyhow, Result};
use clap::{
    crate_description, crate_name, crate_version, App, Arg, ArgMatches,
};
use std::net::ToSocketAddrs;
use tokio::{io::AsyncWriteExt, net::TcpSocket};

mod encrypt;
mod server;
use encrypt::accept_encrypt_tcp;

#[tokio::main]
async fn main() -> Result<()> {
    utils::initialize_logger(3);

    let matches = get_encrypt_command_matches().await?;
    println!("{}, 版本: {}", crate_name!(), crate_version!(),);
    let port = matches.value_of("port").unwrap_or_else(|| {
        println!("请正确填写本地监听端口 例如: -p 8888");
        std::process::exit(1);
    });

    let port: i32 = port.parse().unwrap_or_else(|_| {
        println!("请正确填写本地监听端口 例如: -p 8888");
        std::process::exit(1);
    });

    let server = matches.value_of("server").unwrap_or_else(|| {
        println!("请正确填写服务器地址 例如: -s 8.0.0.0:8888");
        std::process::exit(1);
    });

    let addr = match server.to_socket_addrs().unwrap().next() {
        Some(address) => address,
        None => {
            println!("请正确填写服务器地址 例如: -s 8.0.0.0:8888");
            std::process::exit(1);
        }
    };

    let res =
        tokio::try_join!(accept_encrypt_tcp(port, addr), server_tcp(addr));

    if let Err(err) = res {
        tracing::info!("加密服务断开: {}", err);
    }

    Ok(())
}

async fn server_tcp(addr: std::net::SocketAddr) -> Result<()> {
    let socket = TcpSocket::new_v4()?;
    let mut s = socket.connect(addr).await?;

    s.write_all(b"Hello world").await?;
    //发送当前钱包地址。后续的任务全部提交给一个钱包。
    //let (read,write) = s.split();

    return Err(anyhow!("非正常退出"));
}

pub async fn get_encrypt_command_matches() -> Result<ArgMatches<'static>> {
    let matches = App::new(format!(
        "{}, 版本: {}",
        crate_name!(),
        crate_version!() /* version::commit_date(),
                          * version::short_sha() */
    ))
    .version(crate_version!())
    .about(crate_description!())
    .arg(
        Arg::with_name("port")
            .short("p")
            .long("port")
            .help("本地监听端口")
            .takes_value(true),
    )
    .arg(
        Arg::with_name("server")
            .short("s")
            .long("server")
            .help("服务器监听端口")
            .takes_value(true),
    )
    .get_matches();
    Ok(matches)
}
