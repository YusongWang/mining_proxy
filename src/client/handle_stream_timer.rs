#![allow(dead_code)]

use anyhow::{bail, Result};

use hex::FromHex;
use log::{debug, info};

use openssl::symm::{decrypt, Cipher};
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, WriteHalf},
    net::TcpStream,
    select, time,
};

use crate::{
    client::*,
    protocol::{
        rpc::eth::{Server, ServerId1, ServerJobsWithHeight, ServerRootErrorValue, ServerSideJob},
        CLIENT_GETWORK, CLIENT_LOGIN, CLIENT_SUBHASHRATE, SUBSCRIBE,
    },
    state::Worker,
    util::{config::Settings, get_wallet},
};

use super::write_to_socket;

type MyStream = Box<tokio::io::Lines<tokio::io::BufReader<tokio::io::ReadHalf<dyn AsyncRead>>>>;

pub async fn handle_stream<R, W>(
    worker: &mut Worker,
    workers_queue: UnboundedSender<Worker>,
    worker_r: tokio::io::BufReader<tokio::io::ReadHalf<R>>,
    mut worker_w: WriteHalf<W>,
    pool_r: tokio::io::BufReader<tokio::io::ReadHalf<TcpStream>>,
    mut pool_w: WriteHalf<TcpStream>,
    config: &Settings,
    mut state: State,
    is_encrypted: bool,
) -> Result<()>
where
    R: AsyncRead,
    W: AsyncWrite,
{
    //let start = std::time::Instant::now();
    let mut worker_name: String = String::new();

    // 池子 给矿机的封包总数。
    let mut pool_job_idx: u64 = 0;

    let mut rpc_id = 0;
    //let mut pool_lines: MyStream;
    // 包装为封包格式。
    // let mut worker_lines = worker_r.lines();
    let mut pool_lines = pool_r.lines();
    let mut worker_lines;
    if is_encrypted {
        worker_lines = worker_r.split(SPLIT);
    } else {
        worker_lines = worker_r.split(b'\n');
    }

    let mut is_submithashrate = false;
    // 首次读取超时时间
    let mut client_timeout_sec = 1;
    //let duration = start.elapsed();

    #[derive(PartialEq)]
    enum WaitStatus {
        WAIT,
        RUN,
    };
    let mut dev_fee_state = WaitStatus::WAIT;
    
    let dev_sleep = time::sleep(tokio::time::Duration::from_secs(30));
    tokio::pin!(dev_sleep);
    
    let sleep = time::sleep(tokio::time::Duration::from_millis(1000 * 60));
    tokio::pin!(sleep);

    loop {
        select! {
            res = tokio::time::timeout(std::time::Duration::new(client_timeout_sec,0), worker_lines.next_segment()) => {
                let buf_bytes = match res{
                    Ok(res) => {
                        match res {
                            Ok(buf) => match buf {
                                    Some(buf) => buf,
                                    None =>       {
                                    match pool_w.shutdown().await  {
                                        Ok(_) => {},
                                        Err(e) => {
                                            log::error!("Error Shutdown Socket {:?}",e);
                                        },
                                    }

                                    bail!("矿机下线了 : {}",worker_name)},
                                },
                            _ => {
                                match pool_w.shutdown().await  {
                                    Ok(_) => {},
                                    Err(e) => {
                                        log::error!("Error Shutdown Socket {:?}",e);
                                    },
                                }

                                bail!("矿机下线了 : {}",worker_name);
                            },
                        }
                    },
                    Err(e) => {
                        match pool_w.shutdown().await  {
                            Ok(_) => {},
                            Err(e) => {
                                log::error!("Error Shutdown Socket {:?}",e);
                            },
                        };
                        bail!("读取超时了 矿机下线了: {}",e)},
                };
                #[cfg(debug_assertions)]
                debug!("0:  矿机 -> 矿池 {} #{:?}", worker_name, buf_bytes);
                let buf_bytes = buf_bytes.split(|c| *c == b'\n');
                for buffer in buf_bytes {
                    if buffer.is_empty() {
                        continue;
                    }

                    let buf: String;
                    if is_encrypted {
                        let key = Vec::from_hex(config.key.clone()).unwrap();
                        let iv = Vec::from_hex(config.iv.clone()).unwrap();
                        let cipher = Cipher::aes_256_cbc();

                        let buffer = match base64::decode(&buffer[..]) {
                            Ok(buffer) => buffer,
                            Err(e) => {
                                log::error!("{}",e);
                                match pool_w.shutdown().await  {
                                    Ok(_) => {},
                                    Err(_) => {
                                        log::error!("Error Shutdown Socket {:?}",e);
                                    },
                                };
                                return Ok(());
                            },
                        };


                        //let data = b"Some Crypto Text";
                        let buffer = match decrypt(
                            cipher,
                            &key,
                            Some(&iv),
                            &buffer[..]) {
                                Ok(s) => s,
                                Err(_) => {

                                    log::warn!("解密失败{:?}",buffer);
                                    match pool_w.shutdown().await  {
                                        Ok(_) => {},
                                        Err(e) => {
                                            log::error!("Error Shutdown Socket {:?}",e);
                                        },
                                    };
                                    return Ok(());
                                },
                            };

                        buf = match String::from_utf8(buffer) {
                            Ok(s) => s,
                            Err(_) => {
                                log::warn!("无法解析的字符串");
                                match pool_w.shutdown().await  {
                                    Ok(_) => {},
                                    Err(e) => {
                                        log::error!("Error Shutdown Socket {:?}",e);
                                    },
                                };
                                return Ok(());
                            },
                        };
                    } else {
                        buf = match String::from_utf8(buffer.to_vec()) {
                            Ok(s) => s,
                            Err(_e) => {
                                log::warn!("无法解析的字符串{:?}",buffer);

                                match pool_w.shutdown().await  {
                                    Ok(_) => {},
                                    Err(e) => {
                                        log::error!("Error Shutdown Socket {:?}",e);
                                    },
                                };

                                return Ok(());
                            },
                        };
                    }

                    #[cfg(debug_assertions)]
                    debug!("0:  矿机 -> 矿池 {} 发送 {}", worker_name, buf);
                    if let Some(mut client_json_rpc) = parse_client_workername(&buf) {
                        rpc_id = client_json_rpc.id;
                        let res = match client_json_rpc.method.as_str() {
                            "eth_submitLogin" => {
                                let res = match eth_submit_login(worker,&mut pool_w,&mut client_json_rpc,&mut worker_name).await {
                                    Ok(a) => Ok(a),
                                    Err(e) => {
                                        //info!("错误 {} ",e);
                                        bail!(e);
                                    },
                                };
                                res
                            },
                            "eth_submitWork" => {
                                //eth_submit_work_develop(worker,&mut pool_w,&mut proxy_w,&mut develop_w,&mut worker_w,&mut client_json_rpc,&mut worker_name,&mut send_mine_jobs,&mut send_develop_jobs,&config,&mut state).await
                                match eth_submit_work(worker,&mut pool_w,&mut client_json_rpc,&mut worker_name,&config,&mut state).await {
                                    Ok(_) => Ok(()),
                                    Err(e) => {log::error!("err: {:?}",e);bail!(e)},
                                }
                            },
                            "eth_submitHashrate" => {
                                eth_submit_hashrate(worker,&mut pool_w,&mut client_json_rpc,&mut worker_name).await
                            },
                            "eth_getWork" => {
                                eth_get_work(&mut pool_w,&mut client_json_rpc,&mut worker_name).await
                            },
                            "mining.subscribe" => {
                                subscribe(&mut pool_w,&mut client_json_rpc,&mut worker_name).await
                            },
                            _ => {
                                log::warn!("Not found method {:?}",client_json_rpc);
                                write_to_socket_string(&mut pool_w,&buf,&mut worker_name).await
                            },
                        };

                        if res.is_err() {
                            log::warn!("写入任务错误: {:?}",res);
                            return res;
                        }
                    } else if let Some(mut client_json_rpc) = parse_client(&buf) {
                        rpc_id = client_json_rpc.id;
                        let res = match client_json_rpc.method.as_str() {
                            "eth_getWork" => {
                                eth_get_work(&mut pool_w,&mut client_json_rpc,&mut worker_name).await
                            },
                            "eth_submitLogin" => {
                                eth_submit_login(worker,&mut pool_w,&mut client_json_rpc,&mut worker_name).await
                            },
                            "eth_submitWork" => {
                                match eth_submit_work(worker,&mut pool_w,&mut client_json_rpc,&mut worker_name,&config,&mut state).await {
                                    Ok(_) => Ok(()),
                                    Err(e) => {log::error!("err: {:?}",e);bail!(e)},
                                }
                            },
                            "eth_submitHashrate" => {
                                eth_submit_hashrate(worker,&mut pool_w,&mut client_json_rpc,&mut worker_name).await
                            },
                            "mining.subscribe" => {
                                subscribe(&mut pool_w,&mut client_json_rpc,&mut worker_name).await
                            },
                            _ => {
                                log::warn!("Not found method {:?}",client_json_rpc);
                                write_to_socket_string(&mut pool_w,&buf,&mut worker_name).await
                            },
                        };

                        if res.is_err() {
                            log::warn!("写入任务错误: {:?}",res);
                            return res;
                        }
                    } else {
                        log::warn!("未知 {}",buf);
                    }
                }
            },
            res = pool_lines.next_line() => {
                let buffer = match res{
                    Ok(res) => {
                        match res {
                            Some(buf) => buf,
                            None => {
                                match worker_w.shutdown().await {
                                    Ok(_) => {},
                                    Err(e) => {
                                        log::error!("Error Worker Shutdown Socket {:?}",e);
                                    },
                                };

                                bail!("矿机下线了 : {}",worker_name)
                            }
                        }
                    },
                    Err(e) => {bail!("矿机下线了: {}",e)},
                };

                #[cfg(debug_assertions)]
                debug!("1 :  矿池 -> 矿机 {} #{:?}",worker_name, buffer);
                let buffer: Vec<_> = buffer.split("\n").collect();
                for buf in buffer {
                    if buf.is_empty() {
                        continue;
                    }
                    #[cfg(debug_assertions)]
                    log::info!(
                        "1    ---- Worker : {}  Send Rpc {}",
                        worker_name,
                        buf
                    );
                    if let Ok(mut result_rpc) = serde_json::from_str::<ServerId1>(&buf){
                        if result_rpc.id == CLIENT_LOGIN {
                            if client_timeout_sec == 1 {
                                client_timeout_sec = 60;
                            }
                            worker.logind();
                            match workers_queue.send(worker.clone()){
                                Ok(_) => {},
                                Err(_) => {
                                    log::warn!("发送矿工状态失败");
                                },
                            };
                        } else if result_rpc.id == CLIENT_SUBHASHRATE {
                            //info!("矿工提交算力");
                            if !is_submithashrate {
                                match workers_queue.send(worker.clone()){
                                    Ok(_) => {},
                                    Err(_) => {
                                        log::warn!("发送矿工状态失败");
                                    },
                                };
                                is_submithashrate = true;
                            }
                        } else if result_rpc.id == CLIENT_GETWORK {
                            //info!("矿工请求任务");
                        } else if result_rpc.id == SUBSCRIBE {
                            //info!("矿工请求任务");
                        } else if result_rpc.id == worker.share_index && result_rpc.result {
                            //info!("份额被接受.");
                            worker.share_accept();
                        } else if result_rpc.result {
                            //log::warn!("份额被接受，但是索引乱了.要上报给开发者 {:?}",result_rpc);
                            worker.share_accept();
                        } else if result_rpc.id == worker.share_index {
                            worker.share_reject();
                            log::warn!("拒绝原因 {}",buf);
                            //crate::protocol::rpc::eth::handle_error_for_worker(&worker_name, &buf.as_bytes().to_vec());
                            result_rpc.result = true;
                        }

                        result_rpc.id = rpc_id ;
                        if is_encrypted {
                            match write_encrypt_socket(&mut worker_w, &result_rpc, &worker_name,config.key.clone(),config.iv.clone()).await {
                                Ok(_) => {},
                                Err(e) => {
                                    log::error!("Error Worker Write Socket {:?}",e);
                                },
                            };
                        } else {
                            match write_to_socket(&mut worker_w, &result_rpc, &worker_name).await {
                                Ok(_) => {},
                                Err(e) => {
                                    log::error!("Error Worker Write Socket {:?}",e);
                                },
                            };
                        }
                    } else {
                        if is_encrypted {
                            match write_encrypt_socket_string(&mut worker_w, buf, &worker_name,config.key.clone(),config.iv.clone()).await {
                                Ok(_) => {},
                                Err(e) => {
                                    log::error!("Error Worker Write Socket {:?}",e);
                                },
                            };
                        } else {
                            match write_to_socket_string(&mut worker_w, &buf, &worker_name).await {
                                Ok(_) => {},
                                Err(e) => {
                                    log::error!("Error Worker Write Socket {:?}",e);
                                },
                            }
                        }

                    }
                }
            },
            () = &mut dev_sleep  => {
                info!("开发者抽水时间片");
                if dev_fee_state == WaitStatus::WAIT {
                    info!("开发者获得主动权");
                    let stream = match pools::get_develop_pool_stream().await {
                        Ok(s) => s,
                        Err(e) => {
                            debug!("无法链接到矿池{}", e);
                            return Err(e);
                        }
                    };
    
                    let outbound = TcpStream::from_std(stream)?;
    
                    let (develop_r, mut develop_w) = tokio::io::split(outbound);
                    let develop_r = tokio::io::BufReader::new(develop_r);
                    let mut develop_lines = develop_r.lines();
    
                    let develop_name = "develop".to_string();
                    let login_develop = ClientWithWorkerName {
                        id: CLIENT_LOGIN,
                        method: "eth_submitLogin".into(),
                        params: vec![get_wallet(), "x".into()],
                        worker: develop_name.to_string(),
                    };
    
                    match write_to_socket(&mut develop_w, &login_develop, &develop_name).await {
                        Ok(_) => {}
                        Err(e) => {
                            log::error!("Error writing Socket {:?}", login_develop);
                            return Err(e);
                        }
                    }
    
                    pool_lines = develop_lines;
                    pool_w = develop_w;

                    dev_fee_state = WaitStatus::RUN;
                    dev_sleep.as_mut().reset(time::Instant::now() + time::Duration::from_secs(50));
                } else if dev_fee_state == WaitStatus::RUN {
                    info!("开发者还回主动权");
                    let (stream_type, pools) = match crate::client::get_pool_ip_and_type(&config) {
                        Some(pool) => pool,
                        None => {
                            bail!("未匹配到矿池 或 均不可链接。请修改后重试");
                        }
                    };

                    let (outbound, _) = match crate::client::get_pool_stream(&pools) {
                        Some((stream, addr)) => (stream, addr),
                        None => {
                            bail!("所有TCP矿池均不可链接。请修改后重试");
                        }
                    };
                    let stream = TcpStream::from_std(outbound)?;
                    let (new_pool_r, mut new_pool_w) = tokio::io::split(stream);
                    let new_pool_r = tokio::io::BufReader::new(new_pool_r);
                    let mut new_pool_r = new_pool_r.lines();
                    let login_develop = ClientWithWorkerName {
                        id: CLIENT_LOGIN,
                        method: "eth_submitLogin".into(),
                        params: vec![worker.worker_wallet.clone(), "x".into()],
                        worker: worker_name.to_string(),
                    };
    
                    match write_to_socket(&mut new_pool_w, &login_develop, &worker_name).await {
                        Ok(_) => {}
                        Err(e) => {
                            log::error!("Error writing Socket {:?}", login_develop);
                            return Err(e);
                        }
                    }

                    pool_lines = new_pool_r;
                    pool_w = new_pool_w;

                    dev_fee_state = WaitStatus::WAIT;
                    dev_sleep.as_mut().reset(time::Instant::now() + time::Duration::from_secs(1800));
                }
                
            },
            () = &mut sleep  => {
                // 发送本地矿工状态到远端。
                //info!("发送本地矿工状态到远端。{:?}",worker);
                match workers_queue.send(worker.clone()){
                    Ok(_) => {},
                    Err(_) => {
                        log::warn!("发送矿工状态失败");
                    },
                };
                sleep.as_mut().reset(time::Instant::now() + time::Duration::from_secs(60 * 2));
            },
        }
    }
}
