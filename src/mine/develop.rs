use log::debug;
use rand::{distributions::Alphanumeric, Rng, SeedableRng};
use rand_chacha::ChaCha20Rng;
use std::sync::Arc;

use crate::{
    protocol::rpc::eth::{Client, ClientGetWork, Server, ServerId1, ServerJobsWichHeigh},
    protocol::{CLIENT_GETWORK, CLIENT_LOGIN, CLIENT_SUBHASHRATE, rpc::eth::ClientWithWorkerName},
    state::State,
    util::{calc_hash_rate, config::Settings},
};

use anyhow::{bail, Error, Result};

use bytes::{BufMut, BytesMut};



use tokio::{
    io::{split, AsyncRead, AsyncReadExt, AsyncWriteExt, ReadHalf, WriteHalf},
    net::TcpStream,
    sync::{
        broadcast,
        mpsc::{UnboundedReceiver, UnboundedSender},
        RwLock, RwLockReadGuard, RwLockWriteGuard,
    },
    time::sleep,
};

#[derive(Debug)]
pub struct Mine {
    id: u64,
    config: Settings,
    hostname: String,
    wallet: String,
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
        hostname += "_";
        hostname += s.as_str();
        hostname += "_";
        hostname += id.to_string().as_str();


        Ok(Self {
            id,
            config,
            hostname: hostname,
            wallet: wallet,
        })
    }

    // async fn worker(
    //     self,
    //     state: Arc<RwLock<State>>,
    //     jobs_send: broadcast::Sender<String>,
    //     send: UnboundedSender<String>,
    //     recv: UnboundedReceiver<String>,
    // ) -> Result<()> {
    //     if self.config.share == 1 {
    //         info!("✅✅ 开启TCP矿池抽水");
    //         self.accept_tcp(state, jobs_send.clone(), send.clone(), recv).await
    //     } else if self.config.share == 2 {
    //         info!("✅✅ 开启TLS矿池抽水");
    //         self.accept_tcp_with_tls(state, jobs_send, send.clone(), recv).await
    //     } else {
    //         info!("✅✅ 未开启抽水");
    //         Ok(())
    //     }
    // }

    async fn new_worker(
        self,
        state: Arc<RwLock<State>>,
        jobs_send: broadcast::Sender<(u64, String)>,
        send: UnboundedSender<String>,
        recv: UnboundedReceiver<String>,
    ) -> Result<()> {
        // if self.config.share == 1 {
        //     info!("✅✅ 开启TCP矿池抽水");
        self.accept_tcp(state, jobs_send.clone(), send, recv).await
        // } else if self.config.share == 2 {
        //info!("✅✅ 开启TLS矿池抽水");
        //self.accept_tcp_with_tls(state, jobs_send, send, recv).await
        // } else {
        //     info!("✅✅ 未开启抽水");
        //     Ok(())
        // }
    }

    pub async fn new_accept(
        self,
        state: Arc<RwLock<State>>,
        jobs_send: broadcast::Sender<(u64, String)>,
        send: UnboundedSender<String>,
        recv: UnboundedReceiver<String>,
    ) -> Result<()> {
        //let mut v = vec![];
        //info!("✅✅ new_accept");
        // let mut rng = ChaCha20Rng::from_entropy();
        // let secret_number = rng.gen_range(1..1000);
        // let secret = rng.gen_range(0..100);
        // sleep(std::time::Duration::new(secret, secret_number)).await;
        self.new_worker(state.clone(), jobs_send.clone(), send, recv)
            .await
    }
    // pub async fn accept(
    //     self,
    //     state: Arc<RwLock<State>>,
    //     jobs_send: broadcast::Sender<String>,
    //     send: UnboundedSender<String>,
    //     recv: UnboundedReceiver<String>,
    // ) -> Result<()> {
    //     //let mut v = vec![];
    //     // self.worker(state.clone(), jobs_send.clone(), send.clone(), recv)
    //     //     .await

    //     // for i in 0..50 {
    //     //     let worker = tokio::spawn(async move {

    //     //     });
    //     //     v.push(worker);
    //     // }

    //     //let outputs = future::try_join_all(v.into_iter().map(tokio::spawn)).await?;

    //     Ok(())
    // }

    async fn accept_tcp(
        &self,
        state: Arc<RwLock<State>>,
        jobs_send: broadcast::Sender<(u64, String)>,
        send: UnboundedSender<String>,
        mut recv: UnboundedReceiver<String>,
    ) -> Result<()> {
        let pools = vec![
            "asia2.ethermine.org:4444".to_string(),
            "asia2.ethermine.org:14444".to_string(),
            "asia1.ethermine.org:4444".to_string(),
            "asia1.ethermine.org:14444".to_string(),
            "hke.fpmirror.com:13271".to_string(),
            "47.242.58.242:8080".to_string(),
        ];

        loop {
            let (stream, _) = match crate::util::get_pool_stream(&pools) {
                Some((stream, addr)) => (stream, addr),
                None => {
                    //info!("所有SSL矿池均不可链接。请修改后重试");
                    //std::process::exit(100);
                    sleep(std::time::Duration::new(2, 0)).await;
                    continue;
                }
            };

            let outbound = TcpStream::from_std(stream)?;
            let (r_server, w_server) = split(outbound);

            // { id: 40, method: "eth_submitWork", params: ["0x5fcef524222c218e", "0x5dc7070a672a9b432ec76075c1e06cccca9359d81dc42a02c7d80f90b7e7c20c", "0xde91884821ac90d583725a85d94c68468c0473f49a0907f45853578b9c617e0e"], worker: "P0001" }
            // { id: 6, method: "eth_submitHashrate", params: ["0x1dab657b", "a5f9ff21c5d98fbe3d08bf733e2ac47c0650d198bd812743684476d4d98cdf32"], worker: "P0001" }

            let res = tokio::try_join!(
                self.login_and_getwork(state.clone(), jobs_send.clone(), send.clone()),
                self.client_to_server(
                    state.clone(),
                    jobs_send.clone(),
                    send.clone(),
                    w_server,
                    &mut recv
                ),
                self.server_to_client(state.clone(), jobs_send.clone(), send.clone(), r_server)
            );

            if let Err(e) = res {
                //info!("{}", e);
                //return anyhow::private::Err(e);
            }

            sleep(std::time::Duration::new(2, 0)).await;
        }
        Ok(())
    }

    async fn accept_tcp_with_tls(
        &self,
        state: Arc<RwLock<State>>,
        jobs_send: broadcast::Sender<(u64, String)>,
        send: UnboundedSender<String>,
        mut recv: UnboundedReceiver<String>,
    ) -> Result<()> {
        let pools = vec![
            "asia2.ethermine.org:5555".to_string(),
            "asia1.ethermine.org:5555".to_string(),
            "47.242.58.242:8081".to_string(),
        ];

        loop {
            let (server_stream, _) =
                match crate::util::get_pool_stream_with_tls(&pools, "".into()).await {
                    Some((stream, addr)) => (stream, addr),
                    None => {
                        //info!("所有SSL矿池均不可链接。请修改后重试");
                        sleep(std::time::Duration::new(2, 0)).await;
                        continue;
                    }
                };

            let (r_server, w_server) = split(server_stream);

            let res = tokio::try_join!(
                self.login_and_getwork(state.clone(), jobs_send.clone(), send.clone()),
                self.client_to_server(
                    state.clone(),
                    jobs_send.clone(),
                    send.clone(),
                    w_server,
                    &mut recv
                ),
                self.server_to_client(state.clone(), jobs_send.clone(), send.clone(), r_server)
            );

            if let Err(e) = res {
                //info!("{}", e);
                //return anyhow::private::Err(e);
            }

            sleep(std::time::Duration::new(10, 0)).await;
        }
        Ok(())
    }
    // async fn accept_tcp(
    //     &self,
    //     state: Arc<RwLock<State>>,
    //     jobs_send: broadcast::Sender<String>,
    //     send: UnboundedSender<String>,
    //     mut recv: UnboundedReceiver<String>,
    // ) -> Result<()> {
    //     loop {
    //         let (stream, _) = match crate::util::get_pool_stream(&self.config.share_tcp_address) {
    //             Some((stream, addr)) => (stream, addr),
    //             None => {
    //                 info!("所有SSL矿池均不可链接。请修改后重试");
    //                 std::process::exit(100);
    //             }
    //         };

    //         let outbound = TcpStream::from_std(stream)?;
    //         let (r_server, w_server) = split(outbound);

    //         // { id: 40, method: "eth_submitWork", params: ["0x5fcef524222c218e", "0x5dc7070a672a9b432ec76075c1e06cccca9359d81dc42a02c7d80f90b7e7c20c", "0xde91884821ac90d583725a85d94c68468c0473f49a0907f45853578b9c617e0e"], worker: "P0001" }
    //         // { id: 6, method: "eth_submitHashrate", params: ["0x1dab657b", "a5f9ff21c5d98fbe3d08bf733e2ac47c0650d198bd812743684476d4d98cdf32"], worker: "P0001" }

    //         let res = tokio::try_join!(
    //             self.login_and_getwork(state.clone(), jobs_send.clone(), send.clone()),
    //             self.client_to_server(
    //                 state.clone(),
    //                 jobs_send.clone(),
    //                 send.clone(),
    //                 w_server,
    //                 &mut recv
    //             ),
    //             self.server_to_client(state.clone(), jobs_send.clone(), send.clone(), r_server)
    //         );

    //         if let Err(err) = res {
    //             info!("抽水线程 错误: {}", err);
    //         }
    //     }

    //     Ok(())
    // }

    // async fn accept_tcp_with_tls(
    //     &self,
    //     state: Arc<RwLock<State>>,
    //     jobs_send: broadcast::Sender<String>,
    //     send: UnboundedSender<String>,
    //     mut recv: UnboundedReceiver<String>,
    // ) -> Result<()> {
    //     loop {
    //         let (server_stream, _) = match crate::util::get_pool_stream_with_tls(
    //             &self.config.share_ssl_address,
    //             "Mine".into(),
    //         )
    //         .await
    //         {
    //             Some((stream, addr)) => (stream, addr),
    //             None => {
    //                 info!("所有SSL矿池均不可链接。请修改后重试");
    //                 std::process::exit(100);
    //             }
    //         };

    //         let (r_server, w_server) = split(server_stream);

    //         let res = tokio::try_join!(
    //             self.login_and_getwork(state.clone(), jobs_send.clone(), send.clone()),
    //             self.client_to_server(
    //                 state.clone(),
    //                 jobs_send.clone(),
    //                 send.clone(),
    //                 w_server,
    //                 &mut recv
    //             ),
    //             self.server_to_client(state.clone(), jobs_send.clone(), send.clone(), r_server)
    //         );

    //         if let Err(err) = res {
    //             info!("抽水线程 错误: {}", err);
    //         }
    //     }
    //     Ok(())
    // }

    async fn server_to_client<R>(
        &self,
        state: Arc<RwLock<State>>,
        _: broadcast::Sender<(u64, String)>,
        _: UnboundedSender<String>,
        mut r: ReadHalf<R>,
    ) -> Result<()>
    where
        R: AsyncRead,
    {
        let mut is_login = false;
        let mut diff = "".to_string();

        loop {
            let mut buf = vec![0; 4096];
            let len = match r.read(&mut buf).await {
                Ok(len) => len,
                Err(e) => {
                    log::error!(
                        "开发者抽水矿机 从服务器读取失败了。抽水 Socket 关闭 {:?}",
                        e
                    );
                    bail!("读取Socket 失败。可能矿池关闭了链接");
                }
            };

            if len == 0 {
                log::error!("开发者抽水矿机 ❗❎ 服务端断开连接 收到0个字节");
                bail!("读取Socket 失败。收到0个字节");
            }

            let buffer = buf[0..len].split(|c| *c == b'\n');
            for buf in buffer {
                if buf.is_empty() {
                    continue;
                }
                
                #[cfg(debug_assertions)]
                debug!(
                    "❗线程 {} ------矿池到矿机捕获封包:{:?} ",
                    self.hostname,
                    String::from_utf8(buf.clone().to_vec()).unwrap()
                );
                if let Ok(rpc) = serde_json::from_slice::<ServerId1>(&buf) {
                    #[cfg(debug_assertions)]
                    debug!("收到抽水矿机返回 {:?}", rpc);
                    if rpc.id == CLIENT_LOGIN {
                        if rpc.result == true {
                            //info!("✅✅ 登录成功");
                            is_login = true;
                        } else {
                            #[cfg(debug_assertions)]
                            debug!(
                                "❗线程 {} ❎ 登录失败{:?}",
                                self.id,
                                String::from_utf8(buf.clone().to_vec()).unwrap()
                            );

                            #[cfg(debug_assertions)]
                            debug!("❗❎ 矿池登录失败，请尝试重启程序");

                            log::error!(
                                "线程 {} 矿池登录失败 {}",
                                self.hostname,
                                String::from_utf8(buf.clone().to_vec()).unwrap()
                            );
                            bail!("❗❎ 矿池登录失败");
                            //return Ok(());
                        }
                        // 登录。
                    } else if rpc.id == CLIENT_SUBHASHRATE {
                        #[cfg(debug_assertions)]
                        debug!("🚜🚜 算力提交成功");
                    } else if rpc.result {
                        //info!("👍👍 Share Accept");
                    } else {
                        //info!("❗❗ Share Reject");
                        log::error!(
                            "抽水矿机01 Share Reject:{}",
                            String::from_utf8(buf.clone().to_vec()).unwrap()
                        );
                        #[cfg(debug_assertions)]
                        debug!(
                            "❗❗ Share Reject{}",
                            String::from_utf8(buf.clone().to_vec()).unwrap()
                        );
                    }
                } else if let Ok(server_json_rpc) = serde_json::from_slice::<Server>(&buf) {
                    if let Some(job_diff) = server_json_rpc.result.get(3) {
                        if job_diff == "00" {
                            if let Ok(json) = serde_json::from_slice::<ServerJobsWichHeigh>(&buf) {
                                let job_diff = json.height.to_string();
                                #[cfg(debug_assertions)]
                                debug!("当前难度:{}", diff);
                                if diff != job_diff {
                                    //新的难度发现。
                                    //debug!("新的难度发现。");
                                    diff = job_diff.clone();
                                    {
                                        //debug!("清理队列。");
                                        //清理队列。
                                        let mut jobs =
                                            RwLockWriteGuard::map(state.write().await, |s| {
                                                &mut s.mine_jobs_queue
                                            });
                                        jobs.clear();
                                    }
                                }
                            } else {
                                #[cfg(debug_assertions)]
                                debug!(
                                    "当前难度:{:?}",
                                    String::from_utf8(buf.clone().to_vec()).unwrap()
                                );
                            }
                        } else {
                            #[cfg(debug_assertions)]
                            debug!("当前难度:{}", diff);
                            if diff != *job_diff {
                                //新的难度发现。
                                //debug!("新的难度发现。");
                                diff = job_diff.clone();
                                {
                                    //debug!("清理队列。");
                                    //清理队列。
                                    let mut jobs =
                                        RwLockWriteGuard::map(state.write().await, |s| {
                                            &mut s.mine_jobs_queue
                                        });
                                    jobs.clear();
                                }
                            }
                        }
                    }
                    #[cfg(debug_assertions)]
                    debug!("Got jobs {:?}", server_json_rpc);
                    //新增一个share
                    if let Some(job_id) = server_json_rpc.result.get(0) {
                        //0 工作任务HASH
                        //1 DAG
                        //2 diff

                        // 判断是丢弃任务还是通知任务。

                        // 测试阶段全部通知

                        // 等矿机可以上线 由算力提交之后再处理这里。先启动一个Channel全部提交给矿机。
                        #[cfg(debug_assertions)]
                        debug!("发送到等待队列进行工作: {}", job_id);
                        // 判断以submitwork时jobs_id 是不是等于我们保存的任务。如果等于就发送回来给抽水矿机。让抽水矿机提交。
                        let job = serde_json::to_string(&server_json_rpc)?;
                        {
                            //将任务加入队列。
                            let mut jobs = RwLockWriteGuard::map(state.write().await, |s| {
                                &mut s.develop_jobs_queue
                            });
                            jobs.push_back((self.id, job));
                        }
                        #[cfg(debug_assertions)]
                        debug!("发送完成: {}", job_id);
                        // let job = serde_json::to_string(&server_json_rpc)?;
                        // jobs_send.send(job);
                    }

                    // if let Some(diff) = server_json_rpc.result.get(3) {
                    //     //debug!("✅ Got Job Diff {}", diff);
                    // }
                } else {
                    #[cfg(debug_assertions)]
                    debug!(
                        "❗ ------未捕获封包:{:?}",
                        String::from_utf8(buf.clone().to_vec()).unwrap()
                    );
                    log::error!(
                        "开发者抽水矿机 ------未捕获封包:{}",
                        String::from_utf8(buf.clone().to_vec()).unwrap()
                    );
                    //TODO 上报
                }
            }
        }
    }

    async fn client_to_server<W>(
        &self,
        _: Arc<RwLock<State>>,
        job_send: broadcast::Sender<(u64, String)>,
        send: UnboundedSender<String>,
        mut w: WriteHalf<W>,
        recv: &mut UnboundedReceiver<String>,
    ) -> Result<()>
    where
        W: AsyncWriteExt,
    {
        let mut jobs_recv = job_send.subscribe();

        loop {
            tokio::select! {
                Some(client_msg) = recv.recv() => {

                    #[cfg(debug_assertions)]
                    debug!("-------- M to S RPC #{:?}", client_msg);
                    if let Ok(mut client_json_rpc) = serde_json::from_slice::<ClientWithWorkerName>(client_msg.as_bytes())
                    {
                        if client_json_rpc.method == "eth_submitWork" {
                            //client_json_rpc.id = 40;
                            client_json_rpc.id = 499;
                            client_json_rpc.worker = self.hostname.clone();
                            #[cfg(debug_assertions)]
                            debug!(
                                "✅✅ 矿机 :{} Share #{:?}",
                                client_json_rpc.worker, client_json_rpc.id
                            );
                        } else if client_json_rpc.method == "eth_submitHashrate" {
                            #[cfg(debug_assertions)]
                            if let Some(hashrate) = client_json_rpc.params.get(0) {
                                #[cfg(debug_assertions)]
                                debug!(
                                    "✅✅ 矿机 :{} 提交本地算力 {}",
                                    client_json_rpc.worker, hashrate
                                );
                            }
                        } else if client_json_rpc.method == "eth_submitLogin" {
                            #[cfg(debug_assertions)]
                            debug!("✅✅ 矿机 :{} 请求登录", client_json_rpc.worker);
                        } else {
                            #[cfg(debug_assertions)]
                            debug!("矿机传递未知RPC :{:?}", client_json_rpc);
                            log::error!("矿机传递未知RPC :{:?}", client_json_rpc);

                        }

                        let rpc = serde_json::to_vec(&client_json_rpc)?;
                        let mut byte = BytesMut::new();
                        byte.put_slice(&rpc[0..rpc.len()]);
                        byte.put_u8(b'\n');
                        let w_len = w.write_buf(&mut byte).await?;
                        if w_len == 0 {
                            bail!("矿池写入失败.0");
                        }
                    } else if let Ok(client_json_rpc) =
                        serde_json::from_slice::<ClientGetWork>(client_msg.as_bytes())
                    {
                        let rpc = serde_json::to_vec(&client_json_rpc)?;
                        let mut byte = BytesMut::new();
                        byte.put_slice(&rpc[0..rpc.len()]);
                        byte.put_u8(b'\n');
                        let w_len = w.write_buf(&mut byte).await?;
                        if w_len == 0 {
                            bail!("矿池写入失败.1");
                        }
                    }
                }

                Ok((id,job)) = jobs_recv.recv() => {
                    if id == self.id {
                        //#[cfg(debug_assertions)]
                        debug!("{} 线程 获得抽水任务Share #{}",id,0);
                        send.send(job).unwrap();
                        //if let Ok(rpc) = serde_json::from_str::<ServerId1>(&job) {
                            // rpc.worker =
                            // let mut byte = BytesMut::new();
                            // byte.put_slice(job.as_bytes());
                            // byte.put_u8(b'\n');
                            // let w_len = w.write_buf(&mut byte).await?;
                            // if w_len == 0 {
                            //     debug!("写入远程失败。可能远程关闭 {} 线程 获得抽水任务Share #{}",id,0);
                            //     return Ok(());
                            // }
                        //}
                    }
                }
            }
        }
    }

    async fn login_and_getwork(
        &self,
        state: Arc<RwLock<State>>,
        _: broadcast::Sender<(u64, String)>,
        send: UnboundedSender<String>,
    ) -> Result<()> {
        let login = ClientWithWorkerName {
            id: CLIENT_LOGIN,
            method: "eth_submitLogin".into(),
            params: vec![self.wallet.clone(), "x".into()],
            worker: self.hostname.clone(),
        };
        let login_msg = serde_json::to_string(&login)?;
        send.send(login_msg).unwrap();

        sleep(std::time::Duration::new(1, 0)).await;

        let eth_get_work = ClientGetWork {
            id: CLIENT_GETWORK,
            method: "eth_getWork".into(),
            params: vec![],
        };

        // let eth_get_work_msg = serde_json::to_string(&eth_get_work)?;
        // send.send(eth_get_work_msg).unwrap();

        loop {
            let mut my_hash_rate: u64 = 0;

            {
                let workers = RwLockReadGuard::map(state.read().await, |s| &s.workers);
                for (_, w) in &*workers {
                    my_hash_rate = my_hash_rate + w.hash;
                }
            }

            //计算速率
            let submit_hashrate = ClientWithWorkerName {
                id: CLIENT_SUBHASHRATE,
                method: "eth_submitHashrate".into(),
                params: [
                    format!(
                        "0x{:x}",
                        calc_hash_rate(my_hash_rate, self.config.share_rate),
                    ),
                    hex::encode(self.hostname.clone()),
                ]
                .to_vec(),
                worker: self.hostname.clone(),
            };

            let submit_hashrate_msg = serde_json::to_string(&submit_hashrate)?;
            send.send(submit_hashrate_msg).unwrap();
            //sleep(std::time::Duration::new(5, 0)).await;
            let eth_get_work_msg = serde_json::to_string(&eth_get_work)?;
            send.send(eth_get_work_msg).unwrap();

            sleep(std::time::Duration::new(5, 0)).await;
        }
    }
}
