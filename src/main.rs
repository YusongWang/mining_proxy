use std::sync::Arc;

use anyhow::Result;
use bytes::BytesMut;
use clap::{crate_name, crate_version};
use log::info;
use native_tls::Identity;
use prettytable::{cell, row, Table};
use tokio::{
    fs::File,
    io::AsyncReadExt,
    sync::{
        broadcast,
        mpsc::{self, UnboundedReceiver},
        RwLock, RwLockReadGuard, RwLockWriteGuard,
    },
    time::sleep,
};

mod client;
mod mine;
mod protocol;
mod state;
mod util;

use util::{
    calc_hash_rate,
    config::{self, Settings},
    get_app_command_matches, logger,
};

use crate::{
    client::{pool, tcp::accept_tcp, tls::accept_tcp_with_tls},
    mine::Mine,
    protocol::rpc::eth::Server,
    state::State,
};

const FEE: f32 = 0.005;

#[tokio::main]
async fn main() -> Result<()> {
    let matches = get_app_command_matches().await?;
    let config_file_name = matches.value_of("config").unwrap_or("default.yaml");
    let config = config::Settings::new(config_file_name)?;
    logger::init(
        config.name.as_str(),
        config.log_path.clone(),
        config.log_level,
    )?;

    info!("✅ {}, 版本:{}", crate_name!(), crate_version!());
    // 分配任务给矿机channel
    let (state_send, state_recv) = mpsc::unbounded_channel::<String>();
    // 分配dev任务给矿机channel
    let (dev_state_send, dev_state_recv) = mpsc::unbounded_channel::<String>();

    // let pool = pool::Pool::new(config.clone(), state_send.clone(), dev_state_send.clone());

    // let _ = tokio::join!(
    //     pool.serve(),
    // );

    if config.pool_ssl_address.is_empty() && config.pool_tcp_address.is_empty() {
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

    let mut buffer = BytesMut::with_capacity(10240);
    let read_key_len = p12.read_buf(&mut buffer).await?;
    info!("✅ 证书读取成功，证书字节数为: {}", read_key_len);
    let cert = Identity::from_pkcs12(&buffer[0..read_key_len], config.p12_pass.clone().as_str())?;

    info!("✅ config init success!");

    // 分配任务给矿机channel
    let (job_send, _) = broadcast::channel::<String>(100);

    // 中转抽水费用
    let mine = Mine::new(config.clone()).await?;
    let (proxy_fee_sender, proxy_fee_recver) = mpsc::unbounded_channel::<String>();

    // 开发者费用
    let (fee_tx, fee_rx) = mpsc::unbounded_channel::<String>();
    let develop_account = "0x98be5c44d574b96b320dffb0ccff116bda433b8e".to_string();
    let develop_mine = mine::develop::Mine::new(config.clone(), develop_account).await?;

    // 当前中转总报告算力。Arc<> Or atom 变量
    let state = Arc::new(RwLock::new(State::new()));

    let res = tokio::try_join!(
        accept_tcp(
            state.clone(),
            config.clone(),
            job_send.clone(),
            proxy_fee_sender.clone(),
            fee_tx.clone(),
            state_send.clone(),
            dev_state_send.clone(),
        ),
        accept_tcp_with_tls(
            state.clone(),
            config.clone(),
            job_send.clone(),
            proxy_fee_sender.clone(),
            fee_tx.clone(),
            state_send.clone(),
            dev_state_send.clone(),
            cert
        ),
        mine.accept(
            state.clone(),
            job_send.clone(),
            proxy_fee_sender.clone(),
            proxy_fee_recver
        ),
        process_mine_state(state.clone(), state_recv),
        develop_mine.accept_tcp_with_tls(state.clone(), job_send, fee_tx.clone(), fee_rx),
        process_dev_state(state.clone(), dev_state_recv),
        print_state(state.clone(), config.clone()),
        clear_state(state.clone(), config.clone()),
    );

    if let Err(err) = res {
        info!("processing failed; error = {}", err);
    }

    Ok(())
}

async fn process_mine_state(
    state: Arc<RwLock<State>>,
    mut state_recv: UnboundedReceiver<String>,
) -> Result<()> {
    //debug!("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!! 从队列获得任务 开启");
    loop {
        let job = state_recv.recv().await.expect("从队列获得任务失败.");
        //debug!("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!! 从队列获得任务 {:?}",job);
        let job = serde_json::from_str::<Server>(&*job)?;
        let job_id = job.result.get(0).expect("封包格式错误");
        {
            let mut mine_jobs = RwLockWriteGuard::map(state.write().await, |s| &mut s.mine_jobs);
            if mine_jobs.insert(job_id.clone()) {
                //debug!("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!! insert Hashset success");
            }
            //debug!("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!! {:?}", job_id);
        }

        //debug!("Job_id {} 写入成功", job_id);
        // let job_str = serde_json::to_string(&job)?;
        // {
        //     let mut mine_queue =
        //         RwLockWriteGuard::map(state.write().await, |s| &mut s.mine_jobs_queue);
        //     if mine_queue.remove(&job_str) {
        //         //debug!("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!! remove set success");
        //     }
        //     //debug!("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!! {:?}", job_id);
        // }
    }
}

async fn process_dev_state(
    state: Arc<RwLock<State>>,
    mut state_recv: UnboundedReceiver<String>,
) -> Result<()> {
    //debug!("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!! 从队列获得任务 开启");
    loop {
        let job = state_recv.recv().await.expect("从队列获得任务失败.");
        //debug!("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!! 从队列获得任务 {:?}",job);
        let job = serde_json::from_str::<Server>(&*job)?;
        let job_id = job.result.get(0).expect("封包格式错误");
        {
            let mut develop_jobs =
                RwLockWriteGuard::map(state.write().await, |s| &mut s.develop_jobs);
            if develop_jobs.insert(job_id.clone()) {
                //debug!("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!! insert Hashset success");
            }
            //debug!("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!! {:?}", job_id);
        }

        //debug!("Job_id {} 写入成功", job_id);
        // let job_str = serde_json::to_string(&job)?;
        // {
        //     let mut mine_queue =
        //         RwLockWriteGuard::map(state.write().await, |s| &mut s.develop_jobs_queue);
        //     if mine_queue.remove(&job_str) {
        //         //debug!("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!! remove set success");
        //     }
        //     //debug!("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!! {:?}", job_id);
        // }
    }
}

async fn print_state(state: Arc<RwLock<State>>, config: Settings) -> Result<()> {
    loop {
        sleep(std::time::Duration::new(60, 0)).await;
        // 创建表格
        let mut table = Table::new();
        table.add_row(row![
            "旷工",
            "报告算力",
            "抽水算力",
            "总工作量(份额)",
            "有效份额",
            "无效份额"
        ]);
        let mut total_hash: u64 = 0;
        let mut total_share: u128 = 0;
        let mut total_accept: u128 = 0;
        let mut total_invalid: u128 = 0;

        let workers = RwLockReadGuard::map(state.read().await, |s| &s.workers);

        for w in &*workers {
            // 添加行
            table.add_row(row![
                w.worker,
                w.hash.to_string() + " Mb",
                calc_hash_rate(w.hash, config.share_rate).to_string() + " Mb",
                w.share_index,
                w.accept_index,
                w.invalid_index
            ]);
            total_hash = total_hash + w.hash;
            total_share = total_share + w.share_index;
            total_accept = total_accept + w.accept_index;
            total_invalid = total_invalid + w.invalid_index;
        }

        // 添加行
        table.add_row(row![
            "汇总",
            total_hash.to_string() + " Mb",
            calc_hash_rate(total_hash, config.share_rate).to_string() + " Mb",
            total_share,
            total_accept,
            total_invalid
        ]);

        table.printstd();
    }
}

async fn clear_state(state: Arc<RwLock<State>>, _: Settings) -> Result<()> {
    loop {
        sleep(std::time::Duration::new(60 * 10, 0)).await;
        {
            let workers = RwLockReadGuard::map(state.read().await, |s| &s.workers);
            for _ in &*workers {
                // if w.worker == *rw_worker {
                //     w.share_index = w.share_index + 1;
                // }
                //TODO Send workd to channel . channel send HttpApi to WebViewServer
            }
        }

        // 重新计算每十分钟效率情况
        {
            let mut workers = RwLockWriteGuard::map(state.write().await, |s| &mut s.workers);
            for w in &mut *workers {
                w.share_index = 0;
                w.accept_index = 0;
                w.invalid_index = 0;
            }
        }
    }
}
