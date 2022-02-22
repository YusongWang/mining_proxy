use std::{f32::consts::E, io::Error};

use crate::protocol::{
    eth_stratum::{EthLoginNotify, EthSubscriptionNotify},
    ethjson::{
        login, new_eth_get_work, new_eth_submit_hashrate, new_eth_submit_login,
        new_eth_submit_work,
    },
    stratum::{
        StraumErrorResult, StraumMiningNotify, StraumMiningSet,
        StraumResultBool, StraumRoot,
    },
};

extern crate lru;
use anyhow::{bail, Result};
use futures::StreamExt;
use hex::FromHex;
use lru::LruCache;
use openssl::symm::{decrypt, Cipher};
use std::sync::Arc;
use tracing::{debug, info};

use tokio::{
    io::{
        AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader,
        Lines, ReadHalf, WriteHalf,
    },
    net::TcpStream,
    select,
    sync::RwLockReadGuard,
    time,
};

use crate::{
    client::*,
    protocol::{
        ethjson::{
            EthServer, EthServerRoot, EthServerRootObject,
            EthServerRootObjectBool, EthServerRootObjectError,
            EthServerRootObjectJsonRpc,
        },
        rpc::eth::{
            Server, ServerId1, ServerJobsWithHeight, ServerRootErrorValue,
            ServerSideJob,
        },
        CLIENT_GETWORK, CLIENT_LOGIN, CLIENT_SUBHASHRATE, CLIENT_SUBMITWORK,
        SUBSCRIBE,
    },
    state::Worker,
    util::{config::Settings, is_fee_random},
};

pub async fn handle_stream<R, W>(
    worker: &mut Worker,
    worker_r: tokio::io::BufReader<tokio::io::ReadHalf<R>>,
    mut worker_w: WriteHalf<W>,
    pool_r: tokio::io::BufReader<tokio::io::ReadHalf<TcpStream>>,
    mut pool_w: WriteHalf<TcpStream>, proxy: Arc<Proxy>, is_encrypted: bool,
) -> Result<()>
where
    R: AsyncRead,
    W: AsyncWrite,
{
    let mut worker_name: String = String::new();
    let mut eth_server_result = EthServerRoot {
        id: 0,
        jsonrpc: "2.0".into(),
        result: true,
    };

    // 中转服务器提供人抽水代码
    //let mut unsend_fee_job: LruCache<String, Vec<String>> = LruCache::new(3);
    let mut unsend_fee_job: VecDeque<Vec<String>> = VecDeque::new();
    let mut unsend_dev_job: VecDeque<Vec<String>> = VecDeque::new();

    let mut fee_job: Vec<String> = Vec::new();
    let mut dev_fee_job: Vec<String> = Vec::new();

    // TODO 开发者抽水代码

    //最后一次发送的rpc_id
    let mut rpc_id = 0;

    // 包装为封包格式。
    let mut pool_lines = pool_r.lines();
    let mut worker_lines;

    if is_encrypted {
        worker_lines = worker_r.split(SPLIT);
    } else {
        worker_lines = worker_r.split(b'\n');
    }

    let workers_queue = proxy.worker_tx.clone();
    let sleep = time::sleep(tokio::time::Duration::from_secs(30));
    tokio::pin!(sleep);

    let mut chan = proxy.chan.clone();
    let mut dev_chan = proxy.dev_chan.clone();

    let mut proxy_write = Arc::clone(&proxy.proxy_write);
    let mut dev_write = Arc::clone(&proxy.dev_write);

    loop {
        select! {
            res = worker_lines.next_segment() => {
                let start = std::time::Instant::now();
                let mut buf_bytes = seagment_unwrap(&mut pool_w,res,&worker_name).await?;

                //每次获取一次config. 有更新的话就使用新的了
                let config: Settings;
                {
                    let rconfig = RwLockReadGuard::map(proxy.config.read().await, |s| s);
                    config = rconfig.clone();
                }

                if is_encrypted {
                    let key = Vec::from_hex(config.key.clone()).unwrap();
                    let iv = Vec::from_hex(config.iv.clone()).unwrap();
                    let cipher = Cipher::aes_256_cbc();

                    buf_bytes = match base64::decode(&buf_bytes[..]) {
                        Ok(buffer) => buffer,
                        Err(e) => {
                            tracing::error!("{}",e);
                            match pool_w.shutdown().await  {
                                Ok(_) => {},
                                Err(_) => {
                                    tracing::error!("Error Shutdown Socket {:?}",e);
                                },
                            };
                            bail!("解密矿机请求失败{}",e);
                        },
                    };

                    buf_bytes = match decrypt(
                        cipher,
                        &key,
                        Some(&iv),
                        &buf_bytes[..]) {
                            Ok(s) => s,
                            Err(e) => {
                                tracing::warn!("加密报文解密失败");
                                match pool_w.shutdown().await  {
                                    Ok(_) => {},
                                    Err(e) => {
                                        tracing::error!("Error Shutdown Socket {:?}",e);
                                    },
                                };
                                bail!("解密矿机请求失败{}",e);
                        },
                    };
                }

                #[cfg(debug_assertions)]
                debug!("0:  矿机 -> 矿池 {} #{:?}", worker_name, buf_bytes);
                let buf_bytes = buf_bytes.split(|c| *c == b'\n');
                for buffer in buf_bytes {
                    if buffer.is_empty() {
                        continue;
                    }

                    if let Some(mut json_rpc) = parse(&buffer) {
                        #[cfg(debug_assertions)]
                        info!("接受矿工: {} 提交 RPC {:?}",worker.worker_name,json_rpc);

                        rpc_id = json_rpc.get_id();
                        let res = match json_rpc.get_method().as_str() {
                            "eth_submitLogin" => {
                                eth_server_result.id = rpc_id;
                                login(worker,&mut pool_w,&mut json_rpc,&mut worker_name,&config).await?;
                                write_rpc(is_encrypted,&mut worker_w,&eth_server_result,&worker_name,config.key.clone(),config.iv.clone()).await?;
                                Ok(())
                            },
                            "eth_submitWork" => {
                                eth_server_result.id = rpc_id;
                                if let Some(job_id) = json_rpc.get_job_id(){
                                    //tracing::debug!(job_id = ?job_id,"Get Job ID");
                                    if fee_job.contains(&job_id) {
                                        //Send to fee
                                        tracing::info!(worker_name = ? worker_name,"Got Fee Job");
                                        worker.fee_share_index_add();
                                        worker.fee_share_accept();
                                        json_rpc.set_worker_name(&config.share_name.clone());
                                        {
                                            let mut write = proxy_write.lock().await;
                                            //同时加2个值
                                            write_to_socket_byte(&mut write, json_rpc.to_vec()?, &worker_name).await?
                                        }

                                        //sender.try_send(crate::client::FEE::PROXYFEE(json_rpc))?;
                                    } else if dev_fee_job.contains(&job_id) {
                                        //Send to fee
                                        tracing::info!(worker_name = ? worker_name,"Got Fee Job");
                                        worker.fee_share_index_add();
                                        worker.fee_share_accept();
                                        json_rpc.set_worker_name(&"DevFee".to_string());
                                        {
                                            let mut write = dev_write.lock().await;
                                            //同时加2个值
                                            write_to_socket_byte(&mut write, json_rpc.to_vec()?, &worker_name).await?
                                        }
                                        //sender.try_send(crate::client::FEE::PROXYFEE(json_rpc))?;
                                    } else {
                                        worker.share_index_add();
                                        new_eth_submit_work(worker,&mut pool_w,&mut worker_w,&mut json_rpc,&mut worker_name,&config).await?;
                                    }

                                    write_rpc(is_encrypted,&mut worker_w,&eth_server_result,&worker_name,config.key.clone(),config.iv.clone()).await?;
                                    Ok(())
                                } else {
                                    worker_w.shutdown().await?;
                                    bail!("非法攻击");
                                }
                            },
                            "eth_submitHashrate" => {
                                eth_server_result.id = rpc_id;
                                new_eth_submit_hashrate(worker,&mut pool_w,&mut json_rpc,&mut worker_name).await?;
                                write_rpc(is_encrypted,&mut worker_w,&eth_server_result,&worker_name,config.key.clone(),config.iv.clone()).await?;
                                Ok(())
                            },
                            "eth_getWork" => {

                                new_eth_get_work(&mut pool_w,&mut json_rpc,&mut worker_name).await?;
                                //write_rpc(is_encrypted,&mut worker_w,eth_server_result,&worker_name,config.key.clone(),config.iv.clone()).await?;
                                Ok(())
                            },
                            _ => {
                                tracing::warn!("Not found method {:?}",json_rpc);
                                eth_server_result.id = rpc_id;
                                write_to_socket_byte(&mut pool_w,buffer.to_vec(),&mut worker_name).await?;
                                Ok(())
                            },
                        };

                        if res.is_err() {
                            tracing::warn!("写入任务错误: {:?}",res);
                            return res;
                        }
                    } else {
                        tracing::warn!("协议解析错误: {:?}",buffer);
                        bail!("未知的协议{}",buf_parse_to_string(&mut worker_w,&buffer).await?);
                    }
                }
                #[cfg(debug_assertions)]
                info!("接受矿工: {} 提交处理时间{:?}",worker.worker_name,start.elapsed());
            },
            res = pool_lines.next_line() => {
                let buffer = lines_unwrap(res,&worker_name,"矿池").await?;
                let config: Settings;
                {
                    let rconfig = RwLockReadGuard::map(proxy.config.read().await, |s| s);
                    config = rconfig.clone();
                }
                #[cfg(debug_assertions)]
                debug!("1 :  矿池 -> 矿机 {} #{:?}",worker_name, buffer);

                let buffer: Vec<_> = buffer.split("\n").collect();
                for buf in buffer {
                    if buf.is_empty() {
                        continue;
                    }

                    #[cfg(debug_assertions)]
                    tracing::info!(
                        "1    ---- Worker : {}  Send Rpc {}",
                        worker_name,
                        buf
                    );

                    if let Ok(mut job_rpc) = serde_json::from_str::<EthServerRootObject>(&buf) {
                        // 推送多少次任务？
                        if is_fee_random(0.01) {
                            #[cfg(debug_assertions)]
                            info!("开发者抽水回合");
                            //let job_res = RwLockReadGuard::map(proxy.fee_job.read().await, |s| s);
                            // if let Some(job_res) = unsend_dev_job.pop_back() {
                            //     job_rpc.result = job_res;
                            //     let job_id = job_rpc.get_job_id().unwrap();
                            //     tracing::debug!(job_id = ?job_id,"Set the devfee Job");
                            //     dev_fee_job.push(job_id);
                            // } else
                            if let Some(job_res) = dev_chan.next().await {
                                job_rpc.result = job_res;
                                let job_id = job_rpc.get_job_id().unwrap();
                                tracing::debug!(job_id = ?job_id,"Set the devfee Job");
                                dev_fee_job.push(job_id);
                            } else {
                                tracing::debug!(worker_name = ?worker_name,"开发者没有任务可以分配了");
                            }
                        } else if is_fee_random((config.share_rate + 0.01).into()) {
                            #[cfg(debug_assertions)]
                            info!("中转抽水回合");

                            //let job_res = RwLockReadGuard::map(proxy.fee_job.read().await, |s| s);
                            // if let Some(job_res) = unsend_fee_job.pop_back() {
                            //     job_rpc.result = job_res;
                            //     let job_id = job_rpc.get_job_id().unwrap();
                            //     tracing::debug!(job_id = ?job_id,"Set the devfee Job");
                            //     fee_job.push(job_id);
                            // }
                            if let Some(job_res) = chan.next().await {
                                job_rpc.result = job_res;
                                let job_id = job_rpc.get_job_id().unwrap();
                                tracing::debug!(job_id = ?job_id,"Set the devfee Job");
                                fee_job.push(job_id);
                            } else {
                                tracing::debug!(worker_name = ?worker_name,"没有任务可以分配了");
                            }
                        }

                        write_rpc(is_encrypted,&mut worker_w,&job_rpc,&worker_name,config.key.clone(),config.iv.clone()).await?;
                    } else if let Ok(result_rpc) = serde_json::from_str::<EthServerRoot>(&buf) {
                        if result_rpc.id == CLIENT_LOGIN {
                            worker.logind();
                        } else if result_rpc.id == CLIENT_SUBMITWORK && result_rpc.result {
                            worker.share_accept();
                        } else if result_rpc.id == CLIENT_SUBMITWORK {
                            worker.share_reject();
                        }
                    }
                }
            },
            Some(job) = chan.next() => {
                drop(job);
                //tracing::debug!(job= ?job,worker= ?worker_name,"worker thread Got job");
                // if unsend_fee_job.len() == 1 {
                //     unsend_fee_job.pop_front();
                // }
                // unsend_fee_job.push_back(job);
                // dbg!(&unsend_fee_job);
            },
            Some(job) = dev_chan.next() => {
                drop(job);
                //tracing::debug!(job= ?job,worker= ?worker_name,"worker thread Got job");
                // if unsend_dev_job.len() == 1 {
                //     unsend_dev_job.pop_front();
                // }
                // unsend_dev_job.push_back(job);
                // dbg!(&unsend_dev_job);
            },
            () = &mut sleep  => {
                // 发送本地矿工状态到远端。
                info!("发送本地矿工状态到远端。{:?}",worker);
                match workers_queue.send(worker.clone()){
                    Ok(_) => {},
                    Err(_) => {
                        tracing::warn!("发送矿工状态失败");
                    },
                };
                sleep.as_mut().reset(time::Instant::now() + time::Duration::from_secs(30));
            },
        }
    }
}
