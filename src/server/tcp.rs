use std::{collections::VecDeque, io::Error, ops::BitAnd, pin::Pin};

use anyhow::{bail, Result};
use tracing::{debug, info};
use lru::LruCache;
use rand::{distributions::Alphanumeric, thread_rng, Rng};
use serde::Serialize;
use tokio::{
    io::{split, AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader},
    net::{
        tcp::{ReadHalf, WriteHalf},
        TcpListener, TcpStream,
    },
    select,
    sync::mpsc::UnboundedSender,
    time,
};

use crate::{
    client::{handle_tcp_pool, parse, pools},
    protocol::{
        ethjson::{
            EthClientObject, EthClientWorkerObject, EthServer, EthServerRoot, EthServerRootObject,
            EthServerRootObjectJsonRpc,
        },
        rpc::eth::ClientWithWorkerName,
        CLIENT_GETWORK, CLIENT_LOGIN, CLIENT_SUBHASHRATE, CLIENT_SUBMITWORK, SUBSCRIBE,
    },
    state::{State, Worker},
    util::{config::Settings, get_develop_fee, get_eth_wallet, is_fee_random},
};

pub struct Tcp {
    config: Settings,
    sender: UnboundedSender<Worker>,
    state: State,
    listener: TcpListener,
}

impl Tcp {
    pub async fn new(
        config: Settings,
        state: State,
        sender: UnboundedSender<Worker>,
    ) -> Result<Self> {
        //检查本地端口。
        let address = format!("0.0.0.0:{}", config.tcp_port);
        let listener = match TcpListener::bind(address.clone()).await {
            Ok(listener) => listener,
            Err(_) => {
                bail!("本地端口被占用 {}", address);
            }
        };

        println!("本地TCP端口{} 启动成功!!!", &address);

        Ok(Self {
            config,
            sender,
            state,
            listener,
        })
    }

    pub async fn accept(&mut self) -> Result<()> {
        loop {
            let (stream, addr) = self.listener.accept().await?;

            let config = self.config.clone();
            let workers = self.sender.clone();
            let state = self.state.clone();

            state
                .online
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

            tokio::spawn(async move {
                // 矿工状态管理
                let mut worker: Worker = Worker::default();
                match transfer(&mut worker, workers.clone(), stream, &config, state.clone()).await {
                    Ok(_) => {
                        state
                            .online
                            .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
                        if worker.is_online() {
                            worker.offline();
                            workers.send(worker);
                        } else {
                            info!("IP: {} 断开", addr);
                        }
                    }
                    Err(e) => {
                        if worker.is_online() {
                            worker.offline();
                            workers.send(worker);
                        } else {
                            debug!("IP: {} 恶意链接断开: {}", addr, e);
                        }
                        
                        state
                            .online
                            .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
                    }
                }
            });
        }
    }
}

pub async fn m_write_to_socket<T>(w: &mut WriteHalf<'_>, rpc: &T, worker: &String) -> Result<()>
where
    T: Serialize,
{
    let mut rpc = serde_json::to_vec(&rpc)?;
    rpc.push(b'\n');
    #[cfg(debug_assertions)]
    tracing::debug!(
        "write_to_socket ------Worker : {}  Send Rpc {:?}",
        worker,
        String::from_utf8(rpc.clone())?
    );

    let write_len = w.write(&rpc).await?;
    if write_len == 0 {
        bail!("旷工: {} 服务器断开连接. 写入成功0字节", worker);
    }
    Ok(())
}

pub async fn m_write_to_socket_string(
    w: &mut WriteHalf<'_>,
    rpc: &str,
    worker: &String,
) -> Result<()> {
    let mut rpc = rpc.as_bytes().to_vec();
    rpc.push(b'\n');

    #[cfg(debug_assertions)]
    tracing::debug!(
        "0 ------Worker : {}  Send Rpc {}",
        worker,
        String::from_utf8(rpc.to_vec())?
    );
    let write_len = w.write(&rpc).await?;
    if write_len == 0 {
        bail!("旷工: {} 服务器断开连接. 写入成功0字节", worker);
    }
    Ok(())
}

pub async fn m_write_to_socket_byte(
    w: &mut WriteHalf<'_>,
    mut rpc: Vec<u8>,
    worker: &String,
) -> Result<()> {
    rpc.push(b'\n');
    let write_len = w.write(&rpc).await?;
    if write_len == 0 {
        bail!("旷工: {} 服务器断开连接. 写入成功0字节", worker);
    }
    Ok(())
}

async fn lines_unwrap(
    w: &mut WriteHalf<'_>,
    res: Result<Option<String>, Error>,
    worker_name: &String,
    form_name: &str,
) -> Result<String> {
    let buffer = match res {
        Ok(res) => match res {
            Some(buf) => Ok(buf),
            None => {
                match w.shutdown().await {
                    Ok(_) => {}
                    Err(e) => {
                        tracing::error!("Error Worker Shutdown Socket {:?}", e);
                    }
                };
                return bail!("{}：{}  读取到字节0. 矿池主动断开 ", form_name, worker_name);
            }
        },
        Err(e) => {
            return bail!("{}：{} 读取错误:", form_name, worker_name);
        }
    };

    buffer
}
pub async fn u_write_to_socket_byte(
    w: &mut WriteHalf<'_>,
    mut rpc: Vec<u8>,
    worker: &String,
) -> Result<()> {
    rpc.push(b'\n');
    let write_len = w.write(&rpc).await?;
    if write_len == 0 {
        bail!("旷工: {} 服务器断开连接. 写入成功0字节", worker);
    }
    Ok(())
}
async fn new_eth_submit_login(
    worker: &mut Worker,
    w: &mut WriteHalf<'_>,
    rpc: &mut Box<dyn EthClientObject + Send + Sync>,
    worker_name: &mut String,
) -> Result<()> {
    if let Some(wallet) = rpc.get_eth_wallet() {
        rpc.set_id(CLIENT_LOGIN);
        let mut temp_worker = wallet.clone();
        let mut split = wallet.split(".").collect::<Vec<&str>>();
        if split.len() > 1 {
            worker.login(
                temp_worker.clone(),
                split.get(1).unwrap().to_string(),
                wallet.clone(),
            );
            *worker_name = temp_worker;
        } else {
            temp_worker.push_str(".");
            temp_worker = temp_worker + rpc.get_worker_name().as_str();
            worker.login(temp_worker.clone(), rpc.get_worker_name(), wallet.clone());
            *worker_name = temp_worker;
        }

        u_write_to_socket_byte(w, rpc.to_vec()?, &worker_name).await
    } else {
        bail!("请求登录出错。可能收到暴力攻击");
    }
}

async fn new_eth_submit_work(
    worker: &mut Worker,
    pool_w: &mut WriteHalf<'_>,
    proxy_w: &mut WriteHalf<'_>,
    develop_w: &mut WriteHalf<'_>,
    worker_w: &mut WriteHalf<'_>,
    rpc: &mut Box<dyn EthClientObject + Send + Sync>,
    worker_name: &String,
    mine_send_jobs: &mut LruCache<std::string::String, Vec<std::string::String>>,
    develop_send_jobs: &mut LruCache<std::string::String, Vec<std::string::String>>,
    config: &Settings,
    state: &mut State,
) -> Result<()> {
    rpc.set_id(CLIENT_SUBMITWORK);
    if let Some(job_id) = rpc.get_job_id() {
        #[cfg(debug_assertions)]
        debug!("提交的JobID {}", job_id);
        if mine_send_jobs.contains(&job_id) {
            let hostname = config.get_share_name().unwrap();
            state
                .proxy_share
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            rpc.set_worker_name(&hostname);
            #[cfg(debug_assertions)]
            debug!("得到抽水任务。{:?}", rpc);

            m_write_to_socket_byte(proxy_w, rpc.to_vec()?, &config.share_name).await?;
            return Ok(());
        } else if develop_send_jobs.contains(&job_id) {
            let mut hostname = String::from("develop_");
            state
                .develop_share
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

            let name = hostname::get()?;
            hostname += name.to_str().unwrap();
            rpc.set_worker_name(&hostname);
            #[cfg(debug_assertions)]
            debug!("得到开发者抽水任务。{:?}", rpc);
            m_write_to_socket_byte(develop_w, rpc.to_vec()?, &config.share_name).await?;
            return Ok(());
        } else {
            worker.share_index_add();
            //rpc.set_id(worker.share_index);
            m_write_to_socket_byte(pool_w, rpc.to_vec()?, &worker_name).await
        }
    } else {
        worker.share_index_add();
        //rpc.set_id(worker.share_index);
        m_write_to_socket_byte(pool_w, rpc.to_vec()?, &worker_name).await
    }
}

async fn new_eth_submit_hashrate(
    worker: &mut Worker,
    w: &mut WriteHalf<'_>,
    rpc: &mut Box<dyn EthClientObject + Send + Sync>,
    worker_name: &String,
) -> Result<()> {
    worker.new_submit_hashrate(rpc);
    rpc.set_id(CLIENT_SUBHASHRATE);
    m_write_to_socket_byte(w, rpc.to_vec()?, &worker_name).await
}

async fn seagment_unwrap(
    pool_w: &mut WriteHalf<'_>,
    res: std::io::Result<Option<Vec<u8>>>,
    worker_name: &String,
) -> Result<Vec<u8>> {
    let byte_buffer = match res {
        Ok(buf) => match buf {
            Some(buf) => Ok(buf),
            None => {
                match pool_w.shutdown().await {
                    Ok(_) => {}
                    Err(e) => {
                        tracing::error!("Error Shutdown Socket {:?}", e);
                    }
                }
                bail!("矿工：{}  读取到字节0.矿工主动断开 ", worker_name);
            }
        },
        Err(e) => {
            match pool_w.shutdown().await {
                Ok(_) => {}
                Err(e) => {
                    tracing::error!("Error Shutdown Socket {:?}", e);
                }
            }
            bail!("矿工：{} {}", worker_name, e);
        }
    };

    byte_buffer
}

async fn new_eth_get_work(
    w: &mut WriteHalf<'_>,
    rpc: &mut Box<dyn EthClientObject + Send + Sync>,
    worker_name: &String,
) -> Result<()> {
    rpc.set_id(CLIENT_GETWORK);
    m_write_to_socket_byte(w, rpc.to_vec()?, &worker_name).await
}

async fn m_new_subscribe(
    w: &mut WriteHalf<'_>,
    rpc: &mut Box<dyn EthClientObject + Send + Sync>,
    worker_name: &String,
) -> Result<()> {
    rpc.set_id(SUBSCRIBE);
    m_write_to_socket_byte(w, rpc.to_vec()?, &worker_name).await
}

// pub fn new_job_diff_change(
//     diff: &mut u64,
//     rpc: &EthServerRootObject,
//     a: &mut VecDeque<(String, Vec<String>)>,
//     b: &mut VecDeque<(String, Vec<String>)>,
//     c: &mut VecDeque<(String, Vec<String>)>,
// ) -> bool
// {
//     let job_diff = rpc.get_diff();
//     if job_diff == 0 {
//         return true;
//     }

//     if job_diff > *diff {
//         // 写入新难度
//         *diff = job_diff;
//         // 清空已有任务队列
//         a.clear();
//         b.clear();
//         c.clear();
//     }

//     true
// }

async fn buf_parse_to_string(w: &mut WriteHalf<'_>, buffer: &[u8]) -> Result<String> {
    let buf = match String::from_utf8(buffer.to_vec()) {
        Ok(s) => Ok(s),
        Err(_) => {
            //tracing::warn!("无法解析的字符串{:?}", buffer);
            match w.shutdown().await {
                Ok(_) => {
                    //tracing::warn!("端口可能被恶意扫描: {}", buf);
                }
                Err(e) => {
                    tracing::error!("Error Shutdown Socket {:?}", e);
                }
            };
            bail!("端口可能被恶意扫描。也可能是协议被加密了。");
        }
    };

    buf
    // tracing::warn!("端口可能被恶意扫描: {}", buf);
    // bail!("端口可能被恶意扫描。");
}

async fn transfer(
    worker: &mut Worker,
    worker_queue: UnboundedSender<Worker>,
    mut tcp_stream: TcpStream,
    config: &Settings,
    state: State,
) -> Result<()> {
    let (worker_r, worker_w) = tcp_stream.split();
    //let worker_r = BufReader::new(worker_r);
    let (stream_type, pools) = match crate::client::get_pool_ip_and_type(&config) {
        Some(pool) => pool,
        None => {
            bail!("未匹配到矿池 或 均不可链接。请修改后重试");
        }
    };

    tcphandle_tcp_pool(
        worker,
        worker_queue,
        worker_r,
        worker_w,
        &pools,
        &config,
        state,
        false,
    )
    .await
}

async fn u_seagment_unwrap(
    pool_w: &mut WriteHalf<'_>,
    res: std::io::Result<Option<Vec<u8>>>,
    worker_name: &String,
) -> Result<Vec<u8>> {
    let byte_buffer = match res {
        Ok(buf) => match buf {
            Some(buf) => Ok(buf),
            None => {
                match pool_w.shutdown().await {
                    Ok(_) => {}
                    Err(e) => {
                        tracing::error!("Error Shutdown Socket {:?}", e);
                    }
                }
                bail!("矿工：{}  读取到字节0.矿工主动断开 ", worker_name);
            }
        },
        Err(e) => {
            match pool_w.shutdown().await {
                Ok(_) => {}
                Err(e) => {
                    tracing::error!("Error Shutdown Socket {:?}", e);
                }
            }
            bail!("矿工：{} {}", worker_name, e);
        }
    };

    byte_buffer
}

async fn tcphandle_tcp_pool(
    worker: &mut Worker,
    worker_queue: UnboundedSender<Worker>,
    worker_r: ReadHalf<'_>,
    worker_w: WriteHalf<'_>,
    pools: &Vec<String>,
    config: &Settings,
    state: State,
    is_encrypted: bool,
) -> Result<()> {
    let (outbound, _) = match crate::client::get_pool_stream(&pools) {
        Some((stream, addr)) => (stream, addr),
        None => {
            bail!("所有TCP矿池均不可链接。请修改后重试");
        }
    };

    let stream = TcpStream::from_std(outbound)?;
    // handle(
    //     worker,
    //     worker_queue,
    //     worker_r,
    //     worker_w,
    //     stream,
    //     config,
    //     state,
    //     is_encrypted,
    // )
    // .await

    Ok(())
}

// pub async fn handle<T>(
//     worker: &mut Worker,
//     worker_queue: UnboundedSender<Worker>,
//     worker_r: ReadHalf<'_>,
//     mut worker_w: WriteHalf<'_>,
//     mut pool_stream: Pin<Box<dyn T>>,
//     config: &Settings,
//     mut state: State,
//     is_encrypted: bool,
// ) -> Result<()>
// where
//     T: AsyncRead + AsyncWrite + Send + Sync + Sized,
// {
//     let mut worker_name: String = String::new();
//     let mut eth_server_result = EthServerRoot {
//         id: 0,
//         jsonrpc: "2.0".into(),
//         result: true,
//     };

//     //let mut pool_stream = MyStream::new(pool_stream)?;
//     // let (outbound, _) =
//     //     match crate::client::get_pool_stream_with_tls(&config.share_tcp_address, "proxy".into())
//     //         .await
//     //     {
//     //         Some((stream, addr)) => (stream, addr),
//     //         None => {
//     //             bail!("所有SSL矿池均不可链接。请修改后重试");
//     //         }
//     //     };

//     // //let mut outbound = TcpStream::from_std(stream)?;
//     // // let (proxy_r, mut proxy_w) = split(outbound);
//     // // let proxy_r = tokio::io::BufReader::new(proxy_r);
//     // let mut my_stream = MyStream::new(outbound)?;

//     // pool_stream = my_stream;
//     //pool_r = proxy_r;
//     loop {
//         select! {
//             buffer = pool_stream.next_line() => {
//                 //let start = std::time::Instant::now();
//                 let buffer = buffer?;

//                 //let buf_bytes = seagment_unwrap(&mut pool_stream,res,&worker_name).await?;

//                 // debug!("0:  矿机 -> 矿池 {} #{:?}", worker_name, buf_bytes);
//                 // let buf_bytes = buf_bytes.split(|c| *c == b'\n');
//                 // for buffer in buf_bytes {
//                 //     if buffer.is_empty() {
//                 //         continue;
//                 //     }
//                 // }
//             },
//             buffer = my_stream.next_line() => {
//                 let buffer = buffer?;
//                 //let start = std::time::Instant::now();
//                 //let buffer = lines_unwrap(&mut worker_w,res,&worker_name,"矿池").await?;
//                 #[cfg(debug_assertions)]
//                 debug!("1 :  矿池 -> 矿机 {} #{:?}",worker_name, buffer);
//                 let buffer: Vec<_> = buffer.split(|c| *c == b'\n').collect();
//                 for buf in buffer {
//                     if buf.is_empty() {
//                         continue;
//                     }
//                 }
//             }
//         }
//     }
//     Ok(())
// }
