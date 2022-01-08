mod version {
    include!(concat!(env!("OUT_DIR"), "/version.rs"));
}

use log::info;

use mining_proxy::{
    client::{encry::accept_en_tcp, tcp::accept_tcp, tls::accept_tcp_with_tls},
    state::Worker,
    util::{config::Settings, logger, *},
};

use std::collections::HashMap;

use anyhow::Result;
use bytes::BytesMut;
use clap::crate_version;

use native_tls::Identity;
use prettytable::{cell, row, Table};
use tokio::{
    fs::File,
    io::AsyncReadExt,
    sync::mpsc::{self},
    time::sleep,
};

#[tokio::main]
async fn main() -> Result<()> {
    let matches = mining_proxy::util::get_app_command_matches().await?;
    let _guard = sentry::init((
        "https://a9ae2ec4a77c4c03bca2a0c792d5382b@o1095800.ingest.sentry.io/6115709",
        sentry::ClientOptions {
            release: sentry::release_name!(),
            ..Default::default()
        },
    ));

    let config_file_name = matches.value_of("config").unwrap_or("default.yaml");
    let config = Settings::new(config_file_name, true)?;
    logger::init(
        config.name.as_str(),
        config.log_path.clone(),
        config.log_level,
    )?;

    if config.pool_ssl_address.is_empty() && config.pool_tcp_address.is_empty() {
        println!("代理池地址不能全部为空");
        std::process::exit(1);
    };

    if config.share_tcp_address.is_empty() && config.share_ssl_address.is_empty() {
        println!("抽水矿池地址未填写正确");
        std::process::exit(1);
    };

    if config.tcp_port == 0 && config.ssl_port == 0 && config.encrypt_port == 0 {
        println!("本地监听端口必须启动一个。目前全部为0");
        std::process::exit(1);
    };

    if config.share != 0 && config.share_wallet.is_empty() {
        println!("抽水模式钱包为空。");
        std::process::exit(1);
    }

    let mut p12 = match File::open(config.p12_path.clone()).await {
        Ok(f) => f,
        Err(_) => {
            println!("证书路径错误: {} 下未找到证书!", config.p12_path);
            std::process::exit(1);
        }
    };

    //TODO 用函数检验矿池连通性
    println!(
        "版本: {} commit: {} {}",
        crate_version!(),
        version::commit_date(),
        version::short_sha()
    );

    let mut buffer = BytesMut::with_capacity(10240);
    let read_key_len = p12.read_buf(&mut buffer).await?;
    //info!("✅ 证书读取成功，证书字节数为: {}", read_key_len);
    let cert = Identity::from_pkcs12(&buffer[0..read_key_len], config.p12_pass.clone().as_str())?;

    // 当前中转总报告算力。Arc<> Or atom 变量
    let (worker_tx, worker_rx) = mpsc::unbounded_channel::<Worker>();
    // 当前全局状态管理.
    let state = std::sync::Arc::new(mining_proxy::state::GlobalState::default());

    let res = tokio::try_join!(
        accept_tcp(worker_tx.clone(), config.clone(), state.clone()),
        accept_en_tcp(worker_tx.clone(), config.clone(), state.clone()),
        accept_tcp_with_tls(worker_tx.clone(), config.clone(), cert, state.clone()),
        process_workers(&config, worker_rx, state),
    );

    if let Err(err) = res {
        log::error!("致命错误 : {}", err);
    }

    Ok(())
}

pub async fn print_state(
    workers: &HashMap<String, Worker>,
    config: &Settings,
    state: mining_proxy::state::State,
) -> Result<()> {
    info!(
        "当前在线矿机 {} 台",
        state.online.load(std::sync::atomic::Ordering::SeqCst)
    );

    // 创建表格
    let mut table = Table::new();
    table.add_row(row![
        "矿工",
        "报告算力",
        "抽水算力",
        "总工作量(份额)",
        "有效份额",
        "无效份额",
        "在线时长(小时)",
        "最后提交(分钟)",
    ]);

    let mut total_hash: u64 = 0;
    let mut total_share: u64 = 0;
    let mut total_accept: u64 = 0;
    let mut total_invalid: u64 = 0;
    for (_name, w) in workers {
        if w.last_subwork_time.elapsed().as_secs() >= 1800 {
            continue;
        }

        // 添加行
        table.add_row(row![
            w.worker_name,
            bytes_to_mb(w.hash).to_string() + " Mb",
            calc_hash_rate(bytes_to_mb(w.hash), config.share_rate).to_string() + " Mb",
            w.share_index,
            w.accept_index,
            w.invalid_index,
            time_to_string(w.login_time.elapsed().as_secs()),
            time_to_string(w.last_subwork_time.elapsed().as_secs()),
        ]);

        total_hash += w.hash;
        total_share = total_share + w.share_index;
        total_accept = total_accept + w.accept_index;
        total_invalid = total_invalid + w.invalid_index;
    }

    // {
    //     let w = RwLockReadGuard::map(proxy_worker.read().await, |s| s);
    //     table.add_row(row![
    //         w.worker_name,
    //         bytes_to_mb(w.hash).to_string() + " Mb",
    //         calc_hash_rate(bytes_to_mb(w.hash), config.share_rate).to_string() + " Mb",
    //         w.share_index,
    //         w.accept_index,
    //         w.invalid_index,
    //         time_to_string(w.login_time.elapsed().as_secs()),
    //         time_to_string(w.last_subwork_time.elapsed().as_secs()),
    //     ]);
    // }

    // let w = RwLockReadGuard::map(develop_worker.read().await, |s| s);
    // table.add_row(row![
    //     w.worker_name,
    //     bytes_to_mb(w.hash).to_string() + " Mb",
    //     calc_hash_rate(bytes_to_mb(w.hash), config.share_rate).to_string() + " Mb",
    //     w.share_index,
    //     w.accept_index,
    //     w.invalid_index
    // ]);
    // 添加行
    table.add_row(row![
        "汇总",
        bytes_to_mb(total_hash).to_string() + " Mb",
        calc_hash_rate(bytes_to_mb(total_hash), config.share_rate).to_string() + " Mb",
        total_share,
        total_accept,
        total_invalid,
        "",
        "",
    ]);

    table.printstd();

    let mine_hash = calc_hash_rate(total_hash, config.share_rate);
    match mining_proxy::client::submit_fee_hashrate(config, mine_hash).await {
        Ok(_) => {}
        Err(_) => {}
    }

    let develop_hash = calc_hash_rate(total_hash, get_develop_fee(config.share_rate.into()) as f32);
    match mining_proxy::client::submit_develop_hashrate(config, develop_hash).await {
        Ok(_) => {}
        Err(_) => {}
    }
    Ok(())
}

pub async fn process_workers(
    config: &Settings,
    mut worker_rx: mpsc::UnboundedReceiver<Worker>,
    state: mining_proxy::state::State,
) -> Result<()> {
    let mut workers: HashMap<String, Worker> = HashMap::new();

    let sleep = sleep(tokio::time::Duration::from_millis(1000 * 60));
    tokio::pin!(sleep);

    loop {
        tokio::select! {
            Some(w) = worker_rx.recv() => {
                if workers.contains_key(&w.worker) {
                    if let Some(mine) = workers.get_mut(&w.worker) {
                        *mine = w;
                    }
                } else {
                    workers.insert(w.worker.clone(),w);
                }
            },
            () = &mut sleep => {
                match print_state(&workers,config,state.clone()).await{
                    Ok(_) => {},
                    Err(_) => {log::info!("打印失败了")},
                }

                sleep.as_mut().reset(tokio::time::Instant::now() + tokio::time::Duration::from_millis(1000*60));
            },
        }
    }
}
