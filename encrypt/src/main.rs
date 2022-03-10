use std::net::ToSocketAddrs;

use anyhow::Result;
use clap::{
    crate_description, crate_name, crate_version, App, Arg, ArgMatches,
};
use encrypt::encrypt::accept_encrypt_tcp;
use tokio::try_join;

#[tokio::main]
async fn main() -> Result<()> {
    let matches = get_encrypt_command_matches().await?;

    println!("{}, 版本: {}", crate_name!(), crate_version!(),);

    let key = matches
        .value_of("key")
        .unwrap_or("cde65ec77c3aa0f737e0732848c0e95a");

    let iv = matches.value_of("iv").unwrap_or("123123123123");

    let port = matches.value_of("port").unwrap_or_else(|| {
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

    let port: i32 = port.parse().unwrap_or_else(|_| {
        println!("请正确填写本地监听端口 例如: -p 8888");
        std::process::exit(1);
    });

    let res = try_join!(accept_encrypt_tcp(
        port,
        addr,
        key.to_string().as_bytes().to_vec(),
        iv.to_string().as_bytes().to_vec()
    ));

    if let Err(err) = res {
        tracing::warn!("加密服务断开: {}", err);
    }

    Ok(())
}

pub async fn get_encrypt_command_matches() -> Result<ArgMatches<'static>> {
    let matches = App::new(format!(
        "{}, 版本: {}",
        crate_name!(),
        crate_version!() /* version::commit_date(),
                          * version::short_sha() */
    ))
    .version(crate_version!())
    //.author(crate_authors!("\n"))
    .about(crate_description!())
    .arg(
        Arg::with_name("key")
            .short("k")
            .long("key")
            .help("指定加密秘钥")
            .takes_value(true),
    )
    .arg(
        Arg::with_name("iv")
            .short("i")
            .long("iv")
            .help("指定向量")
            .takes_value(true),
    )
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
