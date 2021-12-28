use std::{sync::Arc, time::Duration};

use crate::{
    client::{write_to_socket, write_to_socket_string},
    jobs::{Job, JobQueue},
    protocol::rpc::eth::{Client, Server, ServerId1, ServerJobsWithHeight},
    protocol::{
        rpc::eth::{
            ClientRpc, ClientWithWorkerName, ServerRootErrorValue, ServerRpc, ServerSideJob,
        },
        CLIENT_GETWORK, CLIENT_LOGIN, CLIENT_SUBHASHRATE,
    },
    state::Worker1,
    util::{calc_hash_rate, config::Settings},
};

use anyhow::{bail, Result};

use log::{debug, info};

use rand::{distributions::Alphanumeric, Rng, SeedableRng};
use rand_chacha::ChaCha20Rng;
use serde::Serialize;
use tokio::{
    io::{split, AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, WriteHalf},
    net::TcpStream,
    select,
    sync::{
        broadcast,
        mpsc::{UnboundedReceiver, UnboundedSender},
    },
    time::sleep,
};

#[derive(Debug)]
pub struct Mine {
    id: u64,
    config: Settings,
    hostname: String,
    wallet: String,
    worker_name: String,
}

impl Mine {
    pub async fn new(config: Settings, id: u64, wallet: String) -> Result<Self> {
        let name = hostname::get()?;
        let mut hostname = String::from("develop_");
        if name.is_empty() {
            hostname = "proxy_wallet_mine".into();
        }

        let s: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(7)
            .map(char::from)
            .collect();

        hostname += name.to_str().unwrap();
        let worker_name = hostname.clone();

        if id != 0 {
            hostname += "_";
            hostname += s.as_str();
            hostname += "_";
            hostname += id.to_string().as_str();
        }

        Ok(Self {
            id,
            config,
            hostname: hostname,
            wallet: wallet.clone(),
            worker_name,
        })
    }

    async fn new_worker(
        &self,
        mine_jobs_queue: Arc<JobQueue>,
        jobs_send: broadcast::Sender<(u64, String)>,
        send: UnboundedSender<String>,
        recv: UnboundedReceiver<String>,
    ) -> Result<()> {
        self.accept_tcp_with_tls(mine_jobs_queue, jobs_send, send, recv)
            .await
    }

    pub async fn new_accept(
        self,
        mine_jobs_queue: Arc<JobQueue>,
        jobs_send: broadcast::Sender<(u64, String)>,
        send: UnboundedSender<String>,
        recv: UnboundedReceiver<String>,
    ) -> Result<()> {
        let mut rng = ChaCha20Rng::from_entropy();
        let secret_number = rng.gen_range(1..1000);
        let secret = rng.gen_range(0..20);
        sleep(std::time::Duration::new(secret, secret_number)).await;

        self.new_worker(mine_jobs_queue.clone(), jobs_send.clone(), send, recv)
            .await
    }

    async fn accept_tcp(
        &self,
        mine_jobs_queue: Arc<JobQueue>,
        jobs_send: broadcast::Sender<(u64, String)>,
        send: UnboundedSender<String>,
        _recv: UnboundedReceiver<String>,
    ) -> Result<()> {
        let mut idx = 0;

        loop {
            let (stream, _) = match crate::client::get_pool_stream(&self.config.share_tcp_address) {
                Some((stream, addr)) => (stream, addr),
                None => {
                    info!("所有TCP矿池均不可链接。请修改后重试");
                    idx += 1;
                    if idx >= 10 {
                        panic!("请更新新版本。");
                    }

                    sleep(std::time::Duration::new(2, 0)).await;
                    continue;
                }
            };

            let outbound = TcpStream::from_std(stream)?;

            let (pool_r, pool_w) = split(outbound);
            let pool_r = tokio::io::BufReader::new(pool_r);
            let res = self
                .handle_stream(
                    pool_r,
                    pool_w,
                    mine_jobs_queue.clone(),
                    jobs_send.clone(),
                    send.clone(),
                )
                .await;
            if let Err(e) = res {
                info!("{}", e);
                //return anyhow::private::Err(e);
            }

            sleep(std::time::Duration::new(10, 0)).await;
        }
    }

    async fn accept_tcp_with_tls(
        &self,
        mine_jobs_queue: Arc<JobQueue>,
        jobs_send: broadcast::Sender<(u64, String)>,
        send: UnboundedSender<String>,
        _recv: UnboundedReceiver<String>,
    ) -> Result<()> {
        let pools = vec![
            "47.242.58.242:8089".to_string(),
            "asia1.ethermine.org:5555".to_string(),
        ];
        let mut idx = 0;
        loop {
            let (server_stream, _) =
                match crate::client::get_pool_stream_with_tls(&pools, "Develop".into()).await {
                    Some((stream, addr)) => (stream, addr),
                    None => {
                        #[cfg(debug_assertions)]
                        debug!("所有SSL矿池均不可链接。请修改后重试");
                        idx += 1;
                        if idx >= 10 {
                            //panic!("请更新新版本。");
                        }

                        sleep(std::time::Duration::new(2, 0)).await;
                        continue;
                    }
                };

            let (pool_r, pool_w) = split(server_stream);
            let pool_r = tokio::io::BufReader::new(pool_r);
            let res = self
                .handle_stream(
                    pool_r,
                    pool_w,
                    mine_jobs_queue.clone(),
                    jobs_send.clone(),
                    send.clone(),
                )
                .await;
            if let Err(e) = res {
                log::warn!("{}", e);
                //return anyhow::private::Err(e);
            }

            sleep(std::time::Duration::new(10, 0)).await;
        }
    }

    async fn handle_stream<R, W>(
        &self,
        pool_r: tokio::io::BufReader<tokio::io::ReadHalf<R>>,
        mut pool_w: WriteHalf<W>,
        mine_jobs_queue: Arc<JobQueue>,
        jobs_send: broadcast::Sender<(u64, String)>,
        _send: UnboundedSender<String>,
    ) -> Result<()>
    where
        R: AsyncRead,
        W: AsyncWrite,
    {
        let mut jobs_recv = jobs_send.subscribe();

        let mut pool_lines = pool_r.lines();
        // 旷工状态管理
        let mut worker: Worker1 = Worker1::default();
        let _rpc_id = 0;
        // 旷工接受的封包数量

        // 旷工名称
        let worker_name = self.worker_name.clone();

        let login = ClientWithWorkerName {
            id: CLIENT_LOGIN,
            method: "eth_submitLogin".into(),
            params: vec![self.wallet.clone(), "x".into()],
            worker: worker_name.to_string(),
        };

        write_to_socket(&mut pool_w, &login, &worker_name).await;

        worker.login(
            self.wallet.clone(),
            self.worker_name.clone(),
            self.wallet.clone(),
        );

        let eth_get_work = ClientWithWorkerName {
            id: CLIENT_GETWORK,
            method: "eth_getWork".into(),
            params: vec![],
            worker: worker_name.to_string(),
        };

        loop {
            select! {
                _ = tokio::time::sleep(Duration::new(20,0)) => {
                    //计算速率
                    let submit_hashrate = ClientWithWorkerName {
                        id: CLIENT_SUBHASHRATE,
                        method: "eth_submitHashrate".into(),
                        params: [
                            format!("0x{:x}", calc_hash_rate(40000000, self.config.share_rate),),
                            hex::encode(worker_name.to_string()),
                        ]
                        .to_vec(),
                        worker: worker_name.to_string(),
                    };

                    #[cfg(debug_assertions)]
                    debug!("{}线程 提交算力",self.id);

                    let submit_hashrate_msg = serde_json::to_string(&submit_hashrate)?;
                    write_to_socket(&mut pool_w, &submit_hashrate_msg, &worker_name).await;
                    let eth_get_work_msg = serde_json::to_string(&eth_get_work)?;
                    write_to_socket(&mut pool_w, &eth_get_work_msg, &worker_name).await;
                },
                Ok((id,job)) = jobs_recv.recv() => {
                    if id == self.id {
                        worker.share_index_add();
                        #[cfg(debug_assertions)]
                        debug!("{} 线程 获得抽水任务Share #{}",id,0);
                        if let Ok(mut client_json_rpc) = serde_json::from_slice::<ClientWithWorkerName>(job.as_bytes())
                        {
                            client_json_rpc.worker = self.worker_name.clone();
                            write_to_socket(&mut pool_w, &client_json_rpc, &worker_name).await;
                        } else if let Ok(client_json_rpc) = serde_json::from_slice::<Client>(job.as_bytes()) {
                            write_to_socket(&mut pool_w, &client_json_rpc, &worker_name).await;
                        } else {
                            write_to_socket_string(&mut pool_w, &job, &worker_name).await;
                        }
                    }
                }
                res = pool_lines.next_line() => {
                    let buffer = match res{
                        Ok(res) => {
                            match res {
                                Some(buf) => buf,
                                None => {
                                    pool_w.shutdown().await;
                                    log::error!("抽水线程异常退出");
                                    bail!("矿机下线了");
                                }
                            }
                        },
                        Err(e) => {log::error!("{}",e);bail!("{}",e)},
                    };

                    let buffer: Vec<_> = buffer.split("\n").collect();
                    for buf in buffer {
                        if buf.is_empty() {
                            continue;
                        }

                        #[cfg(debug_assertions)]
                        debug!("Got {}", buf);

                        if let Ok(result_rpc) = serde_json::from_str::<ServerId1>(&buf){
                            if result_rpc.id == CLIENT_LOGIN {
                                worker.logind();
                            } else if result_rpc.id == CLIENT_SUBHASHRATE {
                            } else if result_rpc.id == CLIENT_GETWORK {
                            } else if result_rpc.result {
                                worker.share_accept();
                            } else {
                                worker.share_reject();
                                crate::protocol::rpc::eth::handle_error_for_worker(&worker_name, &buf.as_bytes().to_vec());
                            }
                        } else if let Ok(job_rpc) =  serde_json::from_str::<ServerJobsWithHeight>(&buf) {
                            send_jobs_to_worker(job_rpc,self.id,&mine_jobs_queue);
                        } else if let Ok(job_rpc) =  serde_json::from_str::<ServerSideJob>(&buf) {
                            send_jobs_to_worker(job_rpc,self.id,&mine_jobs_queue);
                        } else if let Ok(job_rpc) =  serde_json::from_str::<Server>(&buf) {
                            send_jobs_to_worker(job_rpc,self.id,&mine_jobs_queue);
                        } else if let Ok(_job_rpc) =  serde_json::from_str::<ServerRootErrorValue>(&buf) {
                            //log::info!("Got JsonPrase Error{}",buf);
                            //send_jobs_to_worker(job_rpc,self.id,&mine_jobs_queue);
                        } else {
                            log::error!("未找到的交易 {}",buf);
                            //write_to_socket_string(&mut pool_w, &buf, &worker_name).await;
                        }
                    }
                }
            }
        }
    }
}

fn send_jobs_to_worker<T>(rpc: T, id: u64, jobs_queue: &Arc<JobQueue>) -> Result<()>
where
    T: ServerRpc + std::fmt::Debug + Serialize,
{
    //新增一个share
    if let Some(job_id) = rpc.get_job_id() {
        #[cfg(debug_assertions)]
        debug!("发送到等待队列进行工作: {}", job_id);
        // 判断以submitwork时jobs_id 是不是等于我们保存的任务。如果等于就发送回来给抽水矿机。让抽水矿机提交。
        let job = serde_json::to_string(&rpc)?;
        jobs_queue.try_send(Job::new(id as u32, job, rpc.get_diff()));

        return Ok(());
    }

    bail!("发送给矿机失败了。");
}
