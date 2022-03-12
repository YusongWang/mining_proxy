#![allow(dead_code)]
mod version {
    include!(concat!(env!("OUT_DIR"), "/version.rs"));
}

use anyhow::Result;
use clap::{crate_name, crate_version, App, Arg, ArgMatches};
use std::net::ToSocketAddrs;
use tracing::info;
use tracing::Level;
use tracing_subscriber::fmt::{format::Writer, time::FormatTime};

#[tokio::main]
async fn main() -> Result<()> {
    let matches = get_command_matches().await?;
    //mining_proxy::util::logger::init("monitor", "./logs/".into(), 0)?;
    if std::fs::metadata("./logs/").is_err() {
        std::fs::create_dir("./logs/")?;
    }

    struct LocalTimer;
    impl FormatTime for LocalTimer {
        fn format_time(&self, w: &mut Writer<'_>) -> std::fmt::Result {
            write!(w, "{}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"))
        }
    }

    let file_appender =
        tracing_appender::rolling::daily("./logs/", "mining_proxy");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    // 设置日志输出时的格式，例如，是否包含日志级别、是否包含日志来源位置、
    // 设置日志的时间格式 参考: https://docs.rs/tracing-subscriber/0.3.3/tracing_subscriber/fmt/struct.SubscriberBuilder.html#method.with_timer
    let format = tracing_subscriber::fmt::format()
        .with_level(true)
        .with_target(false)
        .with_line_number(true)
        .with_source_location(true)
        .with_timer(LocalTimer);

    // 初始化并设置日志格式(定制和筛选日志)
    tracing_subscriber::fmt()
        .with_max_level(Level::DEBUG)
        //.with_writer(io::stdout) // 写入标准输出
        .with_writer(non_blocking) // 写入文件，将覆盖上面的标准输出
        .with_ansi(false) // 如果日志是写入文件，应将ansi的颜色输出功能关掉
        .event_format(format)
        .init();
    info!(
        "✅ {}, 版本: {} commit: {} {}",
        crate_name!(),
        crate_version!(),
        version::commit_date(),
        version::short_sha()
    );

    let port = matches.value_of("port").unwrap_or_else(|| {
        info!("请正确填写本地监听端口 例如: -p 8888");
        std::process::exit(1);
    });

    let server = matches.value_of("server").unwrap_or_else(|| {
        info!("请正确填写服务器地址 例如: -s 127.0.0.0:8888");
        std::process::exit(1);
    });

    let addr = match server.to_socket_addrs().unwrap().next() {
        Some(address) => address,
        None => {
            info!("请正确填写服务器地址 例如: -s 127.0.0.0:8888");
            std::process::exit(1);
        }
    };

    let port: i32 = port.parse().unwrap_or_else(|_| {
        info!("请正确填写本地监听端口 例如: -p 8888");
        std::process::exit(1);
    });

    let res =
        tokio::try_join!(core::client::monitor::accept_monitor_tcp(port, addr));

    if let Err(err) = res {
        tracing::warn!("加密服务断开: {}", err);
    }

    Ok(())
}

pub async fn get_command_matches() -> Result<ArgMatches<'static>> {
    let matches = App::new(format!(
        "{}, 版本: {} commit {} {}",
        crate_name!(),
        crate_version!(),
        version::commit_date(),
        version::short_sha()
    ))
    .version(crate_version!())
    //.author(crate_authors!("\n"))
    //.about(crate_description!())
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
