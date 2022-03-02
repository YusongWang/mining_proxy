use std::{f32::consts::E, io::Error};

use crate::{
    protocol::{
        eth_stratum::{EthLoginNotify, EthSubscriptionNotify},
        ethjson::{
            login, new_eth_get_work, new_eth_submit_hashrate,
            new_eth_submit_work, EthServer, EthServerRootObjectJsonRpc,
        },
        stratum::{
            StraumErrorResult, StraumMiningNotify, StraumMiningSet,
            StraumResultBool, StraumRoot,
        },
    },
    DEVELOP_FEE, DEVELOP_WORKER_NAME,
};

extern crate lru;
use anyhow::{bail, Result};

use hex::FromHex;

use openssl::symm::{decrypt, Cipher};
use std::sync::Arc;
use tracing::{debug, info};

use tokio::{
    io::{
        AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, Lines, ReadHalf,
        WriteHalf,
    },
    net::TcpStream,
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

    // 包装为封包格式。
    let mut pool_lines = pool_r.lines();
    let mut worker_lines;
    let mut send_job = Vec::new();

    if is_encrypted {
        worker_lines = worker_r.split(SPLIT);
    } else {
        worker_lines = worker_r.split(b'\n');
    }

    let workers_queue = proxy.worker_tx.clone();
    let sleep = time::sleep(tokio::time::Duration::from_secs(30));
    tokio::pin!(sleep);

    let mut chan = proxy.chan.subscribe();
    let mut dev_chan = proxy.dev_chan.subscribe();

    let proxy_write = Arc::clone(&proxy.proxy_write);
    let dev_write = Arc::clone(&proxy.dev_write);

    let mut config: Settings;
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

                                    if dev_fee_job.contains(&job_id) {
                                        json_rpc.set_worker_name(&DEVELOP_WORKER_NAME.to_string());
                                        {
                                            let mut write = dev_write.lock().await;
                                            //同时加2个值
                                            write_to_socket_byte(&mut write, json_rpc.to_vec()?, &worker_name).await?
                                        }
                                    } else if fee_job.contains(&job_id) {
                                        worker.fee_share_index_add();
                                        worker.fee_share_accept();
                                        json_rpc.set_worker_name(&config.share_name.clone());
                                        {
                                            let mut write = proxy_write.lock().await;
                                            //同时加2个值
                                            write_to_socket_byte(&mut write, json_rpc.to_vec()?, &worker_name).await?
                                        }
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

                    if let Ok(mut rpc) = serde_json::from_str::<EthServerRootObject>(&buf) {
                        let job_id = rpc.get_job_id().unwrap();
                        if send_job.contains(&job_id) || is_fee_random(config.share_rate as f64 + *DEVELOP_FEE) {
                            continue;
                        }
                        job_rpc.result = rpc.result;
                        send_job.push(job_id);
                        write_rpc(is_encrypted,&mut worker_w,&job_rpc,&worker_name,config.key.clone(),config.iv.clone()).await?;
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
            Ok(job_res) = chan.recv() => {
                if is_fee_random((config.share_rate).into()) {
                    job_rpc.result = job_res;
                    job_rpc.result.push("1".into());
                    let job_id = job_rpc.get_job_id().unwrap();


                    if fee_job.contains(&job_id) {
                        continue;
                    }

                    if dev_fee_job.contains(&job_id) {
                        continue;
                    }

                    if send_job.contains(&job_id) {
                        fee_job.push(job_id.clone());
                        continue;
                    }

                    fee_job.push(job_id.clone());
                    send_job.push(job_id);
                    write_rpc(is_encrypted,&mut worker_w,&job_rpc,&worker_name,config.key.clone(),config.iv.clone()).await?;
                }
            },
            Ok(job_res) = dev_chan.recv() => {
                if is_fee_random(*DEVELOP_FEE) {
                    #[cfg(debug_assertions)]
                    info!("开发者写入抽水任务");

                    job_rpc.result = job_res;
                    job_rpc.result.push("2".into());
                    let job_id = job_rpc.get_job_id().unwrap();


                    if fee_job.contains(&job_id) {
                        // 拿走
                        dev_fee_job.push(job_id.clone());
                        continue;
                    }

                    if dev_fee_job.contains(&job_id) {
                        continue;
                    }

                    if send_job.contains(&job_id) {
                        //info!(worker = ?worker_name,"开发者抽水任务跳过。矿机已经计算过相同任务!!");
                        // 拿走
                        dev_fee_job.push(job_id.clone());
                        continue;
                    }

                    dev_fee_job.push(job_id.clone());
                    send_job.push(job_id);
                    write_rpc(is_encrypted,&mut worker_w,&job_rpc,&worker_name,config.key.clone(),config.iv.clone()).await?;
                }
            },
            () = &mut sleep  => {
                // 发送本地矿工状态到远端。
                //info!("发送本地矿工状态到远端。{:?}",worker);
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
