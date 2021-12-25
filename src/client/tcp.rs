use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use anyhow::Result;
use log::info;

use tokio::io::{split, BufReader};
use tokio::net::{TcpListener, TcpStream};

use futures::FutureExt;
use tokio::sync::broadcast;

use tokio::sync::{mpsc::UnboundedSender, RwLock};

use crate::jobs::JobQueue;
use crate::protocol::rpc::eth::ServerId1;
use crate::state::State;
use crate::util::config::Settings;

use super::handle_stream;

pub async fn accept_tcp(
    state: Arc<RwLock<State>>,
    mine_jobs_queue: Arc<JobQueue>,
    develop_jobs_queue: Arc<JobQueue>,
    config: Settings,
    job_send: broadcast::Sender<String>,
    proxy_fee_sender: broadcast::Sender<(u64, String)>,
    develop_fee_sender: broadcast::Sender<(u64, String)>,
    state_send: UnboundedSender<(u64, String)>,
    dev_state_send: UnboundedSender<(u64, String)>,
) -> Result<()> {
    let address = format!("0.0.0.0:{}", config.tcp_port);
    let listener = TcpListener::bind(address.clone()).await?;
    info!("😄 Accepting Tcp On: {}", &address);

    loop {
        let (stream, addr) = listener.accept().await?;
        info!("😄 Accepting Tcp connection from {}", addr);

        let config = config.clone();

        let mine_jobs_queue = mine_jobs_queue.clone();
        let develop_jobs_queue = develop_jobs_queue.clone();
        let proxy_fee_sender = proxy_fee_sender.clone();
        let develop_fee_sender = develop_fee_sender.clone();

        tokio::spawn(async move {
            transfer(
                stream,
                &config,
                mine_jobs_queue,
                develop_jobs_queue,
                proxy_fee_sender,
                develop_fee_sender,
            )
            .await
        });
    }
}

async fn transfer(
    tcp_stream: TcpStream,
    config: &Settings,
    mine_jobs_queue: Arc<JobQueue>,
    develop_jobs_queue: Arc<JobQueue>,
    proxy_fee_sender: broadcast::Sender<(u64, String)>,
    develop_fee_sender: broadcast::Sender<(u64, String)>,
) -> Result<()> {
    let (worker_r, worker_w) = split(tcp_stream);
    let worker_r = BufReader::new(worker_r);
    let (stream_type, pools) = match crate::util::get_pool_ip_and_type(&config) {
        Some(pool) => pool,
        None => {
            info!("未匹配到矿池 或 均不可链接。请修改后重试");
            return Ok(());
        }
    };

    let (outbound, _) = match crate::util::get_pool_stream(&pools) {
        Some((stream, addr)) => (stream, addr),
        None => {
            info!("所有TCP矿池均不可链接。请修改后重试");
            return Ok(());
        }
    };

    let stream = TcpStream::from_std(outbound)?;

    let (pool_r, pool_w) = split(stream);
    let pool_r = BufReader::new(pool_r);

    //if stream_type == crate::util::TCP {

    // let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<ServerId1>();
    // let worker = Arc::new(RwLock::new(String::new()));
    // let client_rpc_id = Arc::new(RwLock::new(0u64));

    //let jobs = Arc::new(crate::state::InnerJobs { mine_jobs: Arc::new(RwLock::new(HashMap::new())) });

    // let res = tokio::try_join!(
    //     client_to_server(
    //         state.clone(),
    //         jobs.clone(),
    //         worker.clone(),
    //         client_rpc_id.clone(),
    //         config.clone(),
    //         r_client,
    //         w_server,
    //         proxy_fee_sender.clone(),
    //         //state_send.clone(),
    //         fee.clone(),
    //         tx.clone()
    //     ),
    //     server_to_client(
    //         state.clone(),
    //         jobs.clone(),
    //         mine_jobs_queue.clone(),
    //         worker,
    //         client_rpc_id,
    //         config.clone(),
    //         jobs_recv,
    //         r_server,
    //         w_client,
    //         proxy_fee_sender.clone(),
    //         state_send.clone(),
    //         dev_state_send.clone(),
    //         rx
    //     )
    // );

    // if let Err(err) = res {
    //     //info!("矿机错误或者代理池错误: {}", err);
    // }

    match handle_stream::handle_stream(
        worker_r,
        worker_w,
        pool_r,
        pool_w,
        &config,
        mine_jobs_queue,
        develop_jobs_queue,
        proxy_fee_sender,
        develop_fee_sender,
    )
    .await
    {
        Ok(_) => info!("正常退出"),
        Err(e) => {
            info!("TCP下线 :{:?}", e);
        }
    }

    Ok(())
}
