mod version {
    include!(concat!(env!("OUT_DIR"), "/version.rs"));
}

use log::info;
use proxy::{client::encry::accept_en_tcp, state::Worker};

use std::collections::HashMap;

use anyhow::Result;
use bytes::BytesMut;
use clap::crate_version;

use native_tls::Identity;
use prettytable::{cell, row, Table};
use tokio::{
    fs::File,
    io::AsyncReadExt,
    sync::mpsc::{self, Receiver},
    time::sleep,
};

use proxy::client::tcp::accept_tcp;
use proxy::client::tls::accept_tcp_with_tls;

use proxy::util::config::Settings;
use proxy::util::*;

#[tokio::main]
async fn main() -> Result<()> {
    let matches = get_app_command_matches().await?;
    let _guard = sentry::init((
        "https://a9ae2ec4a77c4c03bca2a0c792d5382b@o1095800.ingest.sentry.io/6115709",
        sentry::ClientOptions {
            release: sentry::release_name!(),
            ..Default::default()
        },
    ));

    let config_file_name = matches.value_of("config").unwrap_or("default.yaml");
    let config = config::Settings::new(config_file_name, true)?;
    logger::init(
        config.name.as_str(),
        config.log_path.clone(),
        config.log_level,
    )?;

    if config.pool_ssl_address.is_empty() && config.pool_tcp_address.is_empty() {
        info!("❎ TLS矿池或TCP矿池必须启动其中的一个。");
        std::process::exit(1);
    };

    if config.share_tcp_address.is_empty() && config.share_ssl_address.is_empty() {
        info!("❎ TLS矿池或TCP矿池必须启动其中的一个。");
        std::process::exit(1);
    };

    if config.share != 0 && config.share_wallet.is_empty() {
        info!("❎ 抽水模式钱包为空。");
        std::process::exit(1);
    }

    let mut p12 = File::open(config.p12_path.clone())
        .await
        .expect("证书路径错误");

    //TODO 用函数检验矿池连通性

    info!(
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

    let res = tokio::try_join!(
        accept_tcp(worker_tx.clone(), config.clone(),),
        accept_en_tcp(worker_tx.clone(), config.clone(),),
        accept_tcp_with_tls(worker_tx.clone(), config.clone(), cert,),
        process_workers(&config, worker_rx,),
    );

    if let Err(err) = res {
        log::error!("致命错误 : {}", err);
    }

    Ok(())
    //}
}

// pub async fn send_worker_state(
//     worker_queue: UnboundedSender<Worker>,
//     worker: Arc<tokio::sync::RwLock<Worker>>,
// ) -> Result<()> {
//     let sleep = sleep(tokio::time::Duration::from_millis(1000 * 60));
//     tokio::pin!(sleep);
//     loop {
//         select! {
//             () = &mut sleep => {
//                 {
//                     let w = RwLockReadGuard::map(worker.read().await, |s| s);
//                     worker_queue.send(w.clone()).await;
//                 }
//                 if false {
//                     anyhow::bail!("false");
//                 }
//                 sleep.as_mut().reset(tokio::time::Instant::now() + tokio::time::Duration::from_millis(1000*60));
//             },
//         }
//     }
// }
// // 中转代理抽水服务
// async fn proxy_accept(
//     worker_queue: UnboundedSender<Worker>,
//     worker: Arc<tokio::sync::RwLock<Worker>>,
//     mine_jobs_queue: Arc<JobQueue>,
//     config: &Settings,
//     jobs_send: broadcast::Sender<(u64, String)>,
// ) -> Result<()> {
//     if config.share == 0 {
//         return Ok(());
//     }

//     let mut v = vec![];

//     let thread_len = util::clac_phread_num_for_real(config.share_rate.into());
//     for i in 0..thread_len {
//         //let mine = mine::old_fee::Mine::new(config.clone(), i).await?;
//         let mine = mine::fee::Mine::new(config.clone(), i, worker.clone()).await?;

//         let send = jobs_send.clone();
//         let s = mine_jobs_queue.clone();
//         let (proxy_fee_sender, proxy_fee_recver) = mpsc::unbounded_channel::<String>();
//         v.push(mine.new_accept(s, send, proxy_fee_sender, proxy_fee_recver));
//     }

//     let res = future::try_join_all(v.into_iter().map(tokio::spawn)).await;
//     if let Err(e) = res {
//         log::warn!("抽水矿机断开{}", e);
//     }

//     Ok(())
// }

// async fn develop_accept(
//     _worker_queue: UnboundedSender<Worker>,
//     worker: Arc<tokio::sync::RwLock<Worker>>,
//     mine_jobs_queue: Arc<JobQueue>,
//     config: &Settings,
//     jobs_send: broadcast::Sender<(u64, String)>,
// ) -> Result<()> {
//     if config.share == 0 {
//         return Ok(());
//     }

//     let mut v = vec![];

//     let thread_len = util::clac_phread_num_for_real(0.01);

//     for i in 0..thread_len {
//         let mine = mine::dev_fee::Mine::new(config.clone(), i, develop_account.clone()).await?;
//         let send = jobs_send.clone();
//         let s = mine_jobs_queue.clone();
//         let (proxy_fee_sender, proxy_fee_recver) = mpsc::unbounded_channel::<String>();
//         v.push(mine.new_accept(s, send, proxy_fee_sender, proxy_fee_recver));
//     }

//     let res = future::try_join_all(v.into_iter().map(tokio::spawn)).await;
//     if let Err(e) = res {
//         log::error!("抽水矿机01 {}", e);
//     }

//     Ok(())
// }

// // 中转代理抽水服务
// async fn old_proxy_accept(
//     mine_jobs_queue: Arc<JobQueue>,
//     config: Settings,
//     jobs_send: broadcast::Sender<(u64, String)>,
// ) -> Result<()> {
//     if config.share == 0 {
//         return Ok(());
//     }

//     let mut v = vec![];
//     //TODO 从ENV读取变量动态设置线程数.
//     let thread_len = util::clac_phread_num_for_real(config.share_rate.into());
//     for i in 0..thread_len {
//         //let mine = mine::old_fee::Mine::new(config.clone(), i).await?;
//         let mine = mine::old_fee::Mine::new(config.clone(), i).await?;

//         let send = jobs_send.clone();
//         let s = mine_jobs_queue.clone();
//         let (proxy_fee_sender, proxy_fee_recver) = mpsc::unbounded_channel::<String>();
//         v.push(mine.new_accept(s, send, proxy_fee_sender, proxy_fee_recver));
//     }

//     let res = future::try_join_all(v.into_iter().map(tokio::spawn)).await;

//     if let Err(e) = res {
//         log::warn!("抽水矿机断开{}", e);
//     }

//     Ok(())
// }
// 开发者抽水服务
// async fn old_develop_accept(
//     mine_jobs_queue: Arc<JobQueue>,
//     config: Settings,
//     jobs_send: broadcast::Sender<(u64, String)>,
// ) -> Result<()> {
//     if config.share == 0 {
//         return Ok(());
//     }

//     let mut v = vec![];

//     let thread_len = util::clac_phread_num_for_real(FEE.into());

//     for i in 0..thread_len {
//         let mine = mine::develop::Mine::new(config.clone(), i, develop_account.clone()).await?;
//         let send = jobs_send.clone();
//         let s = mine_jobs_queue.clone();
//         let (proxy_fee_sender, proxy_fee_recver) = mpsc::unbounded_channel::<String>();
//         v.push(mine.new_accept(s, send, proxy_fee_sender, proxy_fee_recver));
//     }

//     let res = future::try_join_all(v.into_iter().map(tokio::spawn)).await;
//     if let Err(e) = res {
//         log::error!("抽水矿机01 {}", e);
//     }

//     Ok(())
// }

// async fn process_mine_state(
//     state: Arc<RwLock<Workers>,
//     mut state_recv: UnboundedReceiver<(u64, String)>,
// ) -> Result<()> {
//     loop {
//         let (phread_id, queue_job) = state_recv.recv().await.expect("从队列获得任务失败.");
//         //debug!("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!! 从队列获得任务 {:?}",job);
//         let job = serde_json::from_str::<Server>(&*queue_job)?;
//         let job_id = job.result.get(0).expect("封包格式错误");
//         {
//             let mut mine_jobs = RwLockWriteGuard::map(state.write().await, |s| &mut s.mine_jobs);
//             if let None = mine_jobs.insert(job_id.clone(), phread_id) {
//                 #[cfg(debug_assertions)]
//                 debug!("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!! insert Hashset success");
//             } else {
//                 #[cfg(debug_assertions)]
//                 debug!("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!! 任务插入失败: {:?}", job_id);
//             }
//         }
//     }
// }

// async fn process_dev_state(
//     state: Arc<RwLock<Workers>,
//     mut state_recv: UnboundedReceiver<(u64, String)>,
// ) -> Result<()> {
//     loop {
//         let (phread_id, queue_job) =
//             state_recv.recv().await.expect("从队列获得任务失败.");
//         //debug!("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!! 从队列获得任务 {:?}",job);
//         let job = serde_json::from_str::<Server>(&*queue_job)?;
//         let job_id = job.result.get(0).expect("封包格式错误");
//         {
//             let mut develop_jobs =
//                 RwLockWriteGuard::map(state.write().await, |s| &mut s.develop_jobs);
//             if let None = develop_jobs.insert(job_id.clone(), phread_id) {
//                 //debug!("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!! insert Hashset success");
//             }
//             //debug!("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!! {:?}", job_id);
//         }
//     }
// }

pub async fn print_state(workers: &HashMap<String, Worker>, config: &Settings) -> Result<()> {
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

    // //将total hash 写入worker
    let mine_hash = calc_hash_rate(total_hash, config.share_rate);
    match proxy::client::submit_fee_hashrate(config, mine_hash).await {
        Ok(_) => {}
        Err(_) => {}
    }

    let develop_hash = calc_hash_rate(total_hash, get_develop_fee(config.share_rate.into()) as f32);
    match proxy::client::submit_develop_hashrate(config, develop_hash).await {
        Ok(_) => {}
        Err(_) => {}
    }

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

    Ok(())
}

pub async fn process_workers(
    config: &Settings,
    mut worker_rx: mpsc::UnboundedReceiver<Worker>,
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
                match print_state(&workers,config).await{
                    Ok(_) => {},
                    Err(_) => {log::info!("打印失败了")},
                }

                sleep.as_mut().reset(tokio::time::Instant::now() + tokio::time::Duration::from_millis(1000*60));
            },
        }
    }
}
