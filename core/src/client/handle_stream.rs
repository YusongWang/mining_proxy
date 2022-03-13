use aes_gcm::aead::{Aead, NewAead};
use aes_gcm::{Aes256Gcm, Key, Nonce}; // Or `Aes128Gcm`
use anyhow::{bail, Result};

use std::sync::Arc;
use tracing::{debug, info};

use tokio::{
    io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, WriteHalf},
    select,
    sync::RwLockReadGuard,
    time,
};

use crate::{
    client::*,
    protocol::{
        ethjson::{EthServerRoot, EthServerRootObject},
        CLIENT_LOGIN, CLIENT_SUBMITWORK,
    },
    state::Worker,
    util::{config::Settings, is_fee_random},
};
use crate::{
    protocol::ethjson::{
        login, new_eth_get_work, new_eth_submit_hashrate, new_eth_submit_work,
        EthServer, EthServerRootObjectJsonRpc,
    },
    DEVELOP_FEE, DEVELOP_WORKER_NAME,
};

pub async fn handle_stream<R, W, PR, PW>(
    worker: &mut Worker,
    worker_r: tokio::io::BufReader<tokio::io::ReadHalf<R>>,
    mut worker_w: WriteHalf<W>,
    pool_r: tokio::io::BufReader<tokio::io::ReadHalf<PR>>,
    mut pool_w: WriteHalf<PW>, proxy: Arc<Proxy>, is_encrypted: bool,
) -> Result<()>
where
    R: AsyncRead,
    W: AsyncWrite,
    PR: AsyncRead,
    PW: AsyncWrite,
{
    let mut worker_name: String = String::new();
    let mut eth_server_result = EthServerRoot {
        id: 0,
        jsonrpc: "2.0".into(),
        result: true,
    };

    let mut job_rpc = EthServerRootObjectJsonRpc {
        id: 0,
        jsonrpc: "2.0".into(),
        result: vec![],
    };

    let mut fee_job: Vec<String> = Vec::new();
    let mut dev_fee_job: Vec<String> = Vec::new();

    //最后一次发送的rpc_id
    let mut rpc_id = 0;

    // 如果任务时重复的，就等待一次下次发送
    //let mut dev_send_idx = 0;
    let mut job_idx = 0;
    // 包装为封包格式。
    let mut pool_lines = pool_r.lines();
    let mut worker_lines;
    let mut send_job = Vec::new();

    if is_encrypted {
        worker_lines = worker_r.split(SPLIT);
    } else {
        worker_lines = worker_r.split(b'\n');
    }

    use rand::SeedableRng;
    let mut rng = rand_chacha::ChaCha20Rng::from_entropy();
    let send_time = rand::Rng::gen_range(&mut rng, 1..360) as u64;
    let workers_queue = proxy.worker_tx.clone();
    let sleep = time::sleep(tokio::time::Duration::from_secs(send_time));
    tokio::pin!(sleep);

    let mut chan = proxy.chan.subscribe();
    let mut dev_chan = proxy.dev_chan.subscribe();

    let tx = proxy.tx.clone();
    let dev_tx = proxy.dev_tx.clone();

    // 欠了几个job
    // let mut dev_fee_idx = 0;
    // let mut fee_idx = 0;
    // let mut idx = 0;

    let mut wait_job: VecDeque<Vec<String>> = VecDeque::new();
    let mut wait_dev_job: VecDeque<Vec<String>> = VecDeque::new();

    let config: Settings;
    {
        let rconfig = RwLockReadGuard::map(proxy.config.read().await, |s| s);
        config = rconfig.clone();
    }

    loop {
        select! {
            res = worker_lines.next_segment() => {
                let start = std::time::Instant::now();
                let mut buf_bytes = seagment_unwrap(&mut pool_w,res,&worker_name).await?;

                //每次获取一次config. 有更新的话就使用新的了
                //let config: Settings;
                // {
                //     let rconfig = RwLockReadGuard::map(proxy.config.read().await, |s| s);
                //     config = rconfig.clone();
                // }

                // if is_encrypted {
                //     let key = Vec::from_hex(config.key.clone()).unwrap();
                //     let iv = Vec::from_hex(config.iv.clone()).unwrap();
                //     let cipher = Cipher::aes_256_cbc();

                //     buf_bytes = match base64::decode(&buf_bytes[..]) {
                //         Ok(buffer) => buffer,
                //         Err(e) => {
                //             tracing::error!("{}",e);
                //             match pool_w.shutdown().await  {
                //                 Ok(_) => {},
                //                 Err(_) => {
                //                     tracing::error!("Error Shutdown Socket {:?}",e);
                //                 },
                //             };
                //             bail!("解密矿机请求失败{}",e);
                //         },
                //     };

                //     buf_bytes = match decrypt(
                //         cipher,
                //         &key,
                //         Some(&iv),
                //         &buf_bytes[..]) {
                //             Ok(s) => s,
                //             Err(e) => {
                //                 tracing::warn!("加密报文解密失败");
                //                 match pool_w.shutdown().await  {
                //                     Ok(_) => {},
                //                     Err(e) => {
                //                         tracing::error!("Error Shutdown Socket {:?}",e);
                //                     },
                //                 };
                //                 bail!("解密矿机请求失败{}",e);
                //         },
                //     };
                // }

                if is_encrypted {
                    let key = config.key.clone();
                    let iv = config.iv.clone();
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
                    //GenericArray::from(&buf_bytes[..]);
                    //let cipher = Aes128::new(&cipherkey);
                    let key = Key::from_slice(key.as_bytes());
                    let cipher = Aes256Gcm::new(key);

                    let nonce = Nonce::from_slice(iv.as_bytes()); // 96-bits; unique per message

                    // let ciphertext = cipher.encrypt(nonce, b"plaintext message".as_ref())
                    //     .expect("encryption failure!"); // NOTE: handle this error to avoid panics!

                    buf_bytes = match cipher.decrypt(nonce, buf_bytes.as_ref()){
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
                debug!("0:  矿机 -> 矿池 {} #{:?}", worker_name, String::from_utf8(buf_bytes.clone()).unwrap());

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
                                if let Some(job_id) = json_rpc.get_job_id() {
                                    #[cfg(debug_assertions)]
                                    debug!("0 :  收到提交工作量 {} #{:?}",worker_name, json_rpc);
                                    let mut json_rpc = Box::new(EthClientWorkerObject{ id: json_rpc.get_id(), method: json_rpc.get_method(), params: json_rpc.get_params(), worker: worker.worker_name.clone()});


                                    if dev_fee_job.contains(&job_id) {
                                        json_rpc.set_worker_name(&DEVELOP_WORKER_NAME.to_string());
                                        dev_tx.send(json_rpc).await?;
                                    } else if fee_job.contains(&job_id) {
                                        json_rpc.set_worker_name(&config.share_name.clone());
                                        tx.send(json_rpc).await?;
                                        worker.fee_share_index_add();
                                        worker.fee_share_accept();
                                    } else {
                                        worker.share_index_add();
                                        new_eth_submit_work(worker,&mut pool_w,&mut worker_w,&mut json_rpc,&mut worker_name,&config).await?;
                                    }

                                    write_rpc(is_encrypted,&mut worker_w,&eth_server_result,&worker_name,config.key.clone(),config.iv.clone()).await?;
                                    Ok(())
                                } else {
                                    pool_w.shutdown().await?;
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
                                eth_server_result.id = rpc_id;
                                write_rpc(is_encrypted,&mut worker_w,&eth_server_result,&worker_name,config.key.clone(),config.iv.clone()).await?;
                                Ok(())
                            },
                            "mining.subscribe" =>{ //GMiner
                                new_eth_get_work(&mut pool_w,&mut json_rpc,&mut worker_name).await?;
                                eth_server_result.id = rpc_id;
                                write_rpc(is_encrypted,&mut worker_w,&eth_server_result,&worker_name,config.key.clone(),config.iv.clone()).await?;
                                Ok(())
                            }
                            _ => {
                                // tracing::warn!("Not found method {:?}",json_rpc);
                                // eth_server_result.id = rpc_id;
                                // write_to_socket_byte(&mut pool_w,buffer.to_vec(),&mut worker_name).await?;
                                pool_w.shutdown().await?;
                                worker_w.shutdown().await?;
                                return Ok(());
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
                #[cfg(debug_assertions)]
                debug!("1 :  矿池 -> 矿机 {} #{:?}",worker_name, buffer);

                let buffer: Vec<_> = buffer.split("\n").collect();
                #[cfg(debug_assertions)]
                debug!(buffer=?buffer,"打印调试bug.为什么会接受到两次同样的任务。造成延迟？");

                for buf in buffer {
                    if buf.is_empty() {
                        continue;
                    }

                    if let Ok(rpc) = serde_json::from_str::<EthServerRootObject>(&buf) {
                        if is_fee_random(*DEVELOP_FEE) {
                            if let Some(job_res) = wait_dev_job.pop_back() {
                                job_rpc.result = job_res;
                                let job_id = job_rpc.get_job_id().unwrap();
                                dev_fee_job.push(job_id.clone());
                                #[cfg(debug_assertions)]
                                debug!("{} 发送开发者3任务 #{:?}",worker_name, job_rpc);
                                write_rpc(is_encrypted,&mut worker_w,&job_rpc,&worker_name,config.key.clone(),config.iv.clone()).await?;
                            }
                            // if let Ok(job_res) = dev_chan.recv().await {
                            //     job_rpc.result = job_res;
                            //     let job_id = job_rpc.get_job_id().unwrap();
                            //     dev_fee_job.push(job_id.clone());
                            //     #[cfg(debug_assertions)]
                            //     debug!("{} 发送开发者3任务 #{:?}",worker_name, job_rpc);
                            //     write_rpc(is_encrypted,&mut worker_w,&job_rpc,&worker_name,config.key.clone(),config.iv.clone()).await?;
                            // }
                        } else if is_fee_random((config.share_rate +(config.share_rate*0.1)).into()) {
                            if let Some(job_res) = wait_job.pop_back() {
                                job_rpc.result = job_res;
                                let job_id = job_rpc.get_job_id().unwrap();
                                fee_job.push(job_id.clone());
                                #[cfg(debug_assertions)]
                                debug!("{} 发送抽水任务 #{:?}",worker_name, job_rpc);
                                write_rpc(is_encrypted,&mut worker_w,&job_rpc,&worker_name,config.key.clone(),config.iv.clone()).await?;
                            }
                        } else {
                            job_rpc.result = rpc.result;
                            let job_id = job_rpc.get_job_id().unwrap();
                            send_job.push(job_id);
                            #[cfg(debug_assertions)]
                            debug!("{} 发送普通任务 #{:?}",worker_name, job_rpc);
                            write_rpc(is_encrypted,&mut worker_w,&job_rpc,&worker_name,config.key.clone(),config.iv.clone()).await?;
                        }

                        // if is_fee_random((config.share_rate).into()){
                        //     // 和抽水一样。如果欠了Job.就给他还回去。
                        //     if send_job.contains(&job_id) || fee_job.contains(&job_id) || dev_fee_job.contains(&job_id) {
                        //         continue;
                        //     } else  {
                        //         if idx >= 1 {
                        //             idx -= 1;
                        //             job_rpc.result = rpc.result;
                        //             send_job.push(job_id);
                        //             //#[cfg(debug_assertions)]
                        //             debug!("{} 发送普通任务 #{:?} 欠了{}个任务",worker_name, job_rpc,idx);
                        //             write_rpc(is_encrypted,&mut worker_w,&job_rpc,&worker_name,config.key.clone(),config.iv.clone()).await?;
                        //         }
                        //     }
                        // } else {
                        //     if send_job.contains(&job_id) || fee_job.contains(&job_id) || dev_fee_job.contains(&job_id) {
                        //         idx +=1;
                        //         continue;
                        //     } else {
                        //         job_rpc.result = rpc.result;
                        //         send_job.push(job_id);

                        //         //#[cfg(debug_assertions)]
                        //         debug!("{} 发送普通任务 #{:?} 欠了{}个任务",worker_name, job_rpc,idx);
                        //         write_rpc(is_encrypted,&mut worker_w,&job_rpc,&worker_name,config.key.clone(),config.iv.clone()).await?;
                        //     }
                        // }
                    } else if let Ok(result_rpc) = serde_json::from_str::<EthServer>(&buf) {
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
            Ok(job_res) = dev_chan.recv() => {
                wait_dev_job.push_back(job_res);
            },Ok(job_res) = chan.recv() => {
                wait_job.push_back(job_res);
            },
            // Ok(job_res) = chan.recv() => {
            //     if !worker.is_online() {
            //         continue;
            //     }

            //     job_rpc.result = job_res;
            //     let job_id = job_rpc.get_job_id().unwrap();

            //     if fee_idx > 0 {
            //         #[cfg(debug_assertions)]
            //         debug!("{} 尝试偿还抽水任务 #{:?} index :{}",worker_name, job_rpc,dev_fee_idx);
            //         if !send_job.contains(&job_id) && !dev_fee_job.contains(&job_id){
            //             //#[cfg(debug_assertions)]
            //             debug!("{} 偿还成功 #{:?} index :{}",worker_name, job_rpc,dev_fee_idx);
            //             fee_idx -= 1;
            //             fee_job.push(job_id.clone());
            //             write_rpc(is_encrypted,&mut worker_w,&job_rpc,&worker_name,config.key.clone(),config.iv.clone()).await?;
            //         }
            //     } else if is_fee_random(((config.share_rate +(config.share_rate*0.1)) as f64 + *DEVELOP_FEE).into()) {
            //         if send_job.contains(&job_id) {
            //             //#[cfg(debug_assertions)]
            //             debug!("{} 拿走一个抽水任务 #{:?} index :{}",worker_name, job_rpc,dev_fee_idx);
            //             fee_job.push(job_id.clone());
            //             write_rpc(is_encrypted,&mut worker_w,&job_rpc,&worker_name,config.key.clone(),config.iv.clone()).await?;
            //         } else if dev_fee_job.contains(&job_id) {
            //             fee_idx +=1;
            //         } else {
            //             fee_job.push(job_id.clone());
            //             //#[cfg(debug_assertions)]
            //             debug!("{} 发送抽水任务 #{:?}",worker_name, job_rpc);
            //             write_rpc(is_encrypted,&mut worker_w,&job_rpc,&worker_name,config.key.clone(),config.iv.clone()).await?;
            //         }
            //     }
            // },
            // Ok(job_res) = dev_chan.recv() => {
            //     if !worker.is_online() {
            //         continue;
            //     }
            //     job_rpc.result = job_res;
            //     let job_id = job_rpc.get_job_id().unwrap();
            //     if is_fee_random(*DEVELOP_FEE+(*DEVELOP_FEE*0.3)) {
            //         if send_job.contains(&job_id) {
            //             //#[cfg(debug_assertions)]
            //             debug!(worker_name = ?worker_name,job_rpc = ?job_rpc,dev_fee_idx = ?dev_fee_idx,"拿走一个普通任务");
            //             dev_fee_job.push(job_id.clone());
            //         } else if fee_job.contains(&job_id) {
            //             //#[cfg(debug_assertions)]
            //             debug!(worker_name = ?worker_name,job_rpc = ?job_rpc,dev_fee_idx = ?dev_fee_idx,"拿走一个抽水任务");
            //             dev_fee_job.push(job_id.clone());
            //         } else {
            //             dev_fee_job.push(job_id.clone());
            //             //#[cfg(debug_assertions)]
            //             debug!("{} 发送开发者任务 #{:?}",worker_name, job_rpc);
            //             write_rpc(is_encrypted,&mut worker_w,&job_rpc,&worker_name,config.key.clone(),config.iv.clone()).await?;
            //         }
            //     }
            // },
            () = &mut sleep  => {
                // 发送本地矿工状态到远端。
                //info!("发送本地矿工状态到远端。{:?}",worker);
                //tracing::warn!(job_idx = ?job_idx,"当前共发送多少任务?");
                match workers_queue.send(worker.clone()) {
                    Ok(_) => {},
                    Err(_) => {
                        tracing::warn!("发送矿工状态失败");
                    },
                };
                sleep.as_mut().reset(time::Instant::now() + time::Duration::from_secs(send_time));
            },
        }
    }
}
