
use std::net::ToSocketAddrs;
use anyhow::Result;
use clap::{
    crate_description, crate_name, crate_version, App, Arg, ArgMatches,
};

mod encrypt;
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

    let res = tokio::try_join!(accept_encrypt_tcp(port, addr));
    if let Err(err) = res {
        tracing::info!("加密服务断开: {}", err);
    }

    Ok(())
}

pub async fn get_encrypt_command_matches() -> Result<ArgMatches<'static>> {
    let matches = App::new(format!(
        "{}, 版本: {}",
        crate_name!(),
        crate_version!()
	// version::commit_date(),
        // version::short_sha()
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
