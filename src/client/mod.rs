use rand_chacha::ChaCha20Rng;
use std::{cmp::Ordering, sync::Arc};

use anyhow::Result;

use bytes::{BufMut, BytesMut};
use log::{debug, info};
use rand::{Rng, SeedableRng};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadHalf, WriteHalf},
    sync::{
        broadcast,
        mpsc::{UnboundedReceiver, UnboundedSender},
        RwLock, RwLockReadGuard, RwLockWriteGuard,
    },
};

use crate::{
    protocol::{
        rpc::eth::{Client, ClientGetWork, Server, ServerError, ServerId1},
        CLIENT_GETWORK, CLIENT_LOGIN, CLIENT_SUBHASHRATE,
    },
    state::{State, Worker},
    util::{config::Settings, hex_to_int},
};

pub mod tcp;
pub mod tls;

async fn client_to_server<R, W>(
    state: Arc<RwLock<State>>,
    worker: Arc<RwLock<String>>,
    client_rpc_id: Arc<RwLock<u64>>,
    _: Settings,
    mut r: ReadHalf<R>,
    mut w: WriteHalf<W>,
    //state_send: UnboundedSender<String>,
    proxy_fee_sender: broadcast::Sender<(u64, String)>,
    dev_fee_send: broadcast::Sender<(u64, String)>,
    tx: UnboundedSender<ServerId1>,
) -> Result<(), std::io::Error>
where
    R: AsyncRead,
    W: AsyncWrite,
{
    // let mut w = tokio_io_timeout::TimeoutWriter::new(w);
    // tokio::pin!(w);
    let mut worker_name: String = String::new();
    loop {
        let mut buf = vec![0; 4096];

        let len = r.read(&mut buf).await?;

        #[cfg(debug_assertions)]
        info!("读取成功{} 字节", len);

        if len == 0 {
            match remove_worker(state.clone(), worker_name.clone()).await {
                Ok(_) => {}
                Err(_) => info!("❗清理全局变量失败 Code: {}", line!()),
            }

            info!("Worker {} 客户端断开连接.", worker_name);
            return Ok(());
        }

        #[cfg(debug_assertions)]
        match String::from_utf8(buf[0..len].to_vec()) {
            Ok(rpc) => {
                debug!("矿机 -> 矿池 #{:?}", rpc);
            }
            Err(_) => {
                info!("格式化为字符串失败。{:?}", buf[0..len].to_vec());
                return Ok(());
            }
        }

        let buffer_string = match String::from_utf8(buf[0..len].to_vec()) {
            Ok(s) => s,
            Err(_) => {
                info!("错误的封包格式。");
                return Ok(());
            }
        };

        let buffer: Vec<_> = buffer_string.split("\n").collect();
        //let buffer = buf[0..len].split(|c| *c == b'\n');
        //let buffer = buf[0..len].split();
        for buf in buffer {
            if buf.is_empty() {
                continue;
            }
            if len > 5 {
                if let Ok(mut client_json_rpc) = serde_json::from_str::<Client>(&buf) {
                    if client_json_rpc.method == "eth_submitWork" {
                        let mut submit_idx_id = 0;
                        {
                            //新增一个share
                            let mut workers =
                                RwLockWriteGuard::map(state.write().await, |s| &mut s.workers);

                            let rw_worker = RwLockReadGuard::map(worker.read().await, |s| s);
                            if let Some(w) = workers.get_mut(&rw_worker.clone()) {
                                w.share_index = w.share_index + 1;
                                //w.rpc_id = client_json_rpc.id as u64;
                                submit_idx_id = w.share_index;
                                //info!("rpc_id : {}", w.share_index);
                            }

                            //debug!("✅ Worker :{} Share #{}", client_json_rpc.worker, *mapped);
                        }

                        if let Some(job_id) = client_json_rpc.params.get(1) {
                            {
                                let mut mine = RwLockWriteGuard::map(state.write().await, |s| {
                                    &mut s.mine_jobs
                                });
                                if mine.contains_key(job_id) {
                                    if let Some(thread_id) = mine.remove(job_id) {
                                        let rpc = serde_json::to_string(&client_json_rpc)?;

                                        // debug!(
                                        //     "------- 收到 指派任务。可以提交给矿池了 {:?}",
                                        //     job_id
                                        // );

                                        proxy_fee_sender
                                            .send((thread_id, rpc))
                                            .expect("可以提交给矿池任务失败。通道异常了");

                                        let s = ServerId1 {
                                            id: client_json_rpc.id,
                                            //jsonrpc: "2.0".into(),
                                            result: true,
                                        };

                                        tx.send(s).expect("可以提交矿机结果失败。通道异常了");
                                        continue;
                                    }
                                }
                                //debug!("✅ Worker :{} Share #{}", client_json_rpc.worker, *mapped);
                            }

                            {
                                let mut mine = RwLockWriteGuard::map(state.write().await, |s| {
                                    &mut s.develop_jobs
                                });
                                if mine.contains_key(job_id) {
                                    if let Some(thread_id) = mine.remove(job_id) {
                                        let rpc = serde_json::to_string(&client_json_rpc)?;
                                        //debug!("------- 收到 指派任务。可以提交给矿池了 {:?}", job_id);
                                        dev_fee_send
                                            .send((thread_id, rpc))
                                            .expect("可以提交给矿池任务失败。通道异常了");

                                        let s = ServerId1 {
                                            id: client_json_rpc.id,
                                            //jsonrpc: "2.0".into(),
                                            result: true,
                                        };

                                        tx.send(s).expect("可以提交给矿机结果失败。通道异常了");
                                        continue;
                                    }
                                }
                                //debug!("✅ Worker :{} Share #{}", client_json_rpc.worker, *mapped);
                            }
                        }

                        //写入公共rpc_id
                        {
                            let mut rpc_id =
                                RwLockWriteGuard::map(client_rpc_id.write().await, |s| s);
                            *rpc_id = client_json_rpc.id;
                        }

                        client_json_rpc.id = submit_idx_id as u64;
                        {
                            let rw_worker = RwLockReadGuard::map(worker.read().await, |s| s);
                            info!("✅ Worker :{} Share #{}", rw_worker, submit_idx_id);
                        }
                    } else if client_json_rpc.method == "eth_submitHashrate" {
                        // {
                        //     //新增一个share
                        //     let mut workers =
                        //         RwLockWriteGuard::map(state.write().await, |s| &mut s.workers);

                        //     let rw_worker = RwLockReadGuard::map(worker.read().await, |s| s);
                        //     for w in &mut *workers {
                        //         if w.worker == *rw_worker {
                        //             w.rpc_id = client_json_rpc.id as u64;
                        //         }
                        //     }
                        // }
                        //写入公共rpc_id
                        {
                            let mut rpc_id =
                                RwLockWriteGuard::map(client_rpc_id.write().await, |s| s);
                            *rpc_id = client_json_rpc.id;
                        }
                        client_json_rpc.id = CLIENT_SUBHASHRATE;
                        if let Some(hashrate) = client_json_rpc.params.get(0) {
                            {
                                let mut workers =
                                    RwLockWriteGuard::map(state.write().await, |s| &mut s.workers);

                                let rw_worker = RwLockReadGuard::map(worker.read().await, |s| s);

                                if let Some(w) = workers.get_mut(&rw_worker.clone()) {
                                    if let Some(h) = hex_to_int(&hashrate[2..hashrate.len()]) {
                                        w.hash = (h as u64) / 1000 / 1000;
                                    }
                                }
                                // for w in &mut *workers {
                                //     if w.worker == *rw_worker {
                                //         if let Some(h) = hex_to_int(&hashrate[2..hashrate.len()]) {
                                //             w.hash = (h as u64) / 1000 / 1000;
                                //         }
                                //     }
                                // }
                                // let rw_worker = RwLockReadGuard::map(worker.read().await, |s| s);
                                // if hash.get(&*rw_worker).is_some() {
                                //     hash.remove(&*rw_worker);
                                //     hash.insert(rw_worker.clone(), hashrate.clone());
                                // } else {
                                //     hash.insert(rw_worker.clone(), hashrate.clone());
                                // }
                            }

                            // if let Some(h) = crate::util::hex_to_int(&hashrate[2..hashrate.len()]) {
                            //     info!("✅ Worker :{} 提交本地算力 {} MB", worker, h / 1000 / 1000);
                            // } else {
                            //     info!("✅ Worker :{} 提交本地算力 {} MB", worker, hashrate);
                            // }
                        }
                    } else if client_json_rpc.method == "eth_submitLogin" {
                        //写入公共rpc_id
                        {
                            let mut id = RwLockWriteGuard::map(client_rpc_id.write().await, |s| s);
                            *id = client_json_rpc.id;
                        }

                        // //新增一个share
                        // let mut workers =
                        //     RwLockWriteGuard::map(state.write().await, |s| &mut s.workers);

                        // let rw_worker = RwLockReadGuard::map(worker.read().await, |s| s);
                        // for w in &mut *workers {
                        //     if w.worker == *rw_worker {
                        //         w.rpc_id = client_json_rpc.id as u64;
                        //     }
                        // }
                        client_json_rpc.id = CLIENT_LOGIN;
                        if let Some(wallet) = client_json_rpc.params.get(0) {
                            let mut temp_worker = wallet.clone();
                            temp_worker.push_str(".");
                            temp_worker = temp_worker + client_json_rpc.worker.as_str();
                            let mut rw_worker = RwLockWriteGuard::map(worker.write().await, |s| s);
                            *rw_worker = temp_worker.clone();
                            worker_name = temp_worker.clone();
                            info!("✅ Worker :{} 请求登录", *rw_worker);
                        } else {
                            //debug!("❎ 登录错误，未找到登录参数");
                        }
                    } else {
                        //debug!("❎ Worker {} 传递未知RPC :{:?}", worker, client_json_rpc);
                    }

                    let mut rpc = serde_json::to_string(&client_json_rpc)?;
                    rpc.push_str("\r\n");
                    let write_len = w.write(rpc.as_bytes()).await?;
                    if write_len == 0 {
                        match remove_worker(state.clone(), worker_name.clone()).await {
                            Ok(_) => {}
                            Err(_) => info!("❗清理全局变量失败 Code: {}", line!()),
                        }

                        info!("✅ Worker: {} 服务器断开连接.", worker_name);
                        return Ok(());
                    }
                } else if let Ok(mut client_json_rpc) = serde_json::from_str::<ClientGetWork>(&buf)
                {
                    // {
                    //     //新增一个share
                    //     let mut workers =
                    //         RwLockWriteGuard::map(state.write().await, |s| &mut s.workers);

                    //     let rw_worker = RwLockReadGuard::map(worker.read().await, |s| s);
                    //     for w in &mut *workers {
                    //         if w.worker == *rw_worker {
                    //             w.rpc_id = client_json_rpc.id as u64;
                    //         }
                    //     }
                    // }
                    {
                        let mut rpc_id = RwLockWriteGuard::map(client_rpc_id.write().await, |s| s);
                        *rpc_id = client_json_rpc.id;
                    }
                    client_json_rpc.id = CLIENT_GETWORK;
                    //debug!("获得任务:{:?}", client_json_rpc);
                    let mut rpc = serde_json::to_string(&client_json_rpc)?;
                    rpc.push_str("\r\n");
                    let write_len = w.write(rpc.as_bytes()).await?;
                    info!("🚜 Worker: {} 请求计算任务", worker_name);
                    if write_len == 0 {
                        match remove_worker(state.clone(), worker_name.clone()).await {
                            Ok(_) => {}
                            Err(_) => info!("❗清理全局变量失败 Code: {}", line!()),
                        }

                        info!(
                        "✅ Worker: {} 服务器断开连接.安全离线。可能丢失算力。已经缓存本次操作。",
                        worker_name
                    );
                        return Ok(());
                    }
                }
            } else {
                return Ok(());
            }
        }
    }
}

async fn server_to_client<R, W>(
    state: Arc<RwLock<State>>,
    worker: Arc<RwLock<String>>,
    client_rpc_id: Arc<RwLock<u64>>,
    mut config: Settings,
    _: broadcast::Receiver<String>,
    mut r: ReadHalf<R>,
    mut w: WriteHalf<W>,
    _: broadcast::Sender<(u64, String)>,
    state_send: UnboundedSender<(u64, String)>,
    dev_state_send: UnboundedSender<(u64, String)>,
    mut rx: UnboundedReceiver<ServerId1>,
) -> Result<(), std::io::Error>
where
    R: AsyncRead,
    W: AsyncWrite,
{
    // let mut r = tokio_io_timeout::TimeoutReader::new(r);
    // r.set_timeout(Some(std::time::Duration::from_millis(1000)));
    // tokio::pin!(r);

    // let mut w = tokio_io_timeout::TimeoutWriter::new(w);
    // w.set_timeout(Some(std::time::Duration::from_millis(1000)));
    // tokio::pin!(w);
    let mut is_login = false;

    loop {
        let mut buf = vec![0; 4096];

        tokio::select! {
            len = r.read(&mut buf) => {
                let len = match len{
                    Ok(len) => len,
                    Err(e) => {
                        //info!("{:?}",e);
                        return anyhow::private::Err(e);
                    },
                };


                if len == 0 {
                    let worker_name: String;
                    {
                        let guard = worker.read().await;
                        let rw_worker = RwLockReadGuard::map(guard, |s| s);
                        worker_name = rw_worker.to_string();
                    }

                    info!("worker {} ",worker_name);
                    match remove_worker(state.clone(), worker_name.clone()).await {
                        Ok(_) => {}
                        Err(_) => info!("❗清理全局变量失败 Code: {}", line!()),
                    }
                    info!(
                        "✅ Worker: {} 读取失败。链接失效。",
                        worker_name
                    );
                    return Ok(());
                }

                // debug!(
                //     "S to C RPC #{:?}",
                //     String::from_utf8(buf[0..len].to_vec()).unwrap()
                // );

                let buffer_string =  String::from_utf8(buf[0..len].to_vec()).unwrap();
                let buffer:Vec<_> = buffer_string.split("\n").collect();
                //let buffer = buf[0..len].split(|c| *c == b'\n');
                //let buffer = buf[0..len].split();
                for buf in buffer {
                    if buf.is_empty() {
                        continue;
                    }

                    #[cfg(debug_assertions)]
                    debug!("Got jobs {}",buf);
                    if let Ok(mut server_json_rpc) = serde_json::from_str::<ServerId1>(&buf) {

                        let rw_worker = RwLockReadGuard::map(worker.read().await, |s| s);
                        // let mut workers =
                        // RwLockWriteGuard::map(state.write().await, |s| &mut s.workers);
                        // if let Some(w) = workers.get_mut(&rw_worker.clone()) {
                                let mut rpc_id = 0;
                            if server_json_rpc.id == CLIENT_LOGIN {
                                if server_json_rpc.result {

                                    let rw_worker = RwLockReadGuard::map(worker.read().await, |s| s);
                                    let wallet:Vec<_>= rw_worker.split(".").collect();
                                    let mut workers =
                                    RwLockWriteGuard::map(state.write().await, |s| &mut s.workers);

                                    workers.insert(rw_worker.clone(),Worker::new(
                                        rw_worker.clone(),
                                        wallet[1].clone().to_string(),
                                        wallet[0].clone().to_string(),
                                    ));
                                    is_login = true;
                                    info!("✅ {} 登录成功",rw_worker);

                                } else {
                                    #[cfg(debug_assertions)]
                                    debug!(
                                        " 登录失败{:?}",
                                        String::from_utf8(buf.as_bytes().to_vec()).unwrap()
                                    );
                                    info!("矿池登录失败");
                                    log::error!(
                                        "矿池登录失败 {}",
                                        String::from_utf8(buf.as_bytes().to_vec()).unwrap()
                                    );
                                    //return Ok(());
                                }
                                // 登录。
                            } else if server_json_rpc.id == CLIENT_SUBHASHRATE {
                                //info!("👍 Worker :{} 算力提交成功", rw_worker);
                            } else if server_json_rpc.id == CLIENT_GETWORK {

                            } else  {
                                let rw_worker = RwLockReadGuard::map(worker.read().await, |s| s);
                                let mut workers =
                                RwLockWriteGuard::map(state.write().await, |s| &mut s.workers);
                                if let Some(w) = workers.get_mut(&rw_worker.clone()) {
                                    rpc_id = w.share_index;
                                    if server_json_rpc.id as u128 == rpc_id{
                                        if server_json_rpc.result == true {

                                            w.accept_index = w.accept_index + 1;
                                            info!("👍 Worker :{} Share #{} Accept", rw_worker,rpc_id);
                                        } else {
                                            w.invalid_index = w.invalid_index + 1;
                                            info!("❗ Worker :{} Share #{} Reject", rw_worker,rpc_id);
                                            log::error!(
                                                " Worker :{} Share #{} Reject",
                                                rw_worker,rpc_id
                                            );
                                        }
                                    } else {
                                        info!("❗ Worker :{} Got Unpackage Idx {}", rw_worker,rpc_id);
                                        log::error!(
                                            "❗ Worker :{} Got Unpackage Idx {}",
                                            rw_worker,rpc_id
                                        );
                                    }
                                }
                            }

                        {
                            let rpc_id = RwLockReadGuard::map(client_rpc_id.read().await, |s| s);
                            server_json_rpc.id = *rpc_id;
                        }

                        let to_client_buf = serde_json::to_string(&server_json_rpc).expect("格式化RPC失败");
                        let mut byte = BytesMut::from(to_client_buf.as_str());
                        byte.put_u8(b'\n');
                        let len = w.write_buf(&mut byte).await?;
                        if len == 0 {
                            info!("❗ 服务端写入失败 断开连接.");
                            let worker_name: String;
                            {
                                let guard = worker.read().await;
                                let rw_worker = RwLockReadGuard::map(guard, |s| s);
                                worker_name = rw_worker.to_string();
                            }

                            info!("worker {} ",worker_name);
                            match remove_worker(state.clone(), worker_name).await {
                                Ok(_) => {}
                                Err(_) => info!("❗清理全局变量失败 Code: {}", line!()),
                            }
                            return Ok(());
                        }

                        continue;
                    } else if let Ok(_) = serde_json::from_str::<Server>(&buf) {

                            if config.share != 0 {
                                {
                                    let mut rng = ChaCha20Rng::from_entropy();
                                    let secret_number = rng.gen_range(1..1000);

                                    let max = (1000.0 * crate::FEE) as u32;
                                    let max = 1000 - max; //900
                                    match secret_number.cmp(&max) {
                                        Ordering::Less => {}
                                        _ => {
                                            let mut jobs_queue =
                                            RwLockWriteGuard::map(state.write().await, |s| &mut s.develop_jobs_queue);
                                            if jobs_queue.len() > 0 {
                                                let (phread_id,queue_job) = jobs_queue.pop_back().unwrap();
                                                let job = serde_json::from_str::<Server>(&*queue_job)?;

                                                let rpc = serde_json::to_vec(&job).expect("格式化RPC失败");
                                                let mut byte = BytesMut::new();
                                                byte.put_slice(&rpc[..]);
                                                byte.put_u8(b'\n');

                                                let w_len = w.write_buf(&mut byte).await?;
                                                if w_len == 0 {
                                                    let worker_name: String;
                                                    {
                                                        let rw_worker = RwLockReadGuard::map(worker.read().await, |s| s);
                                                        worker_name = rw_worker.clone();
                                                    }

                                                    match remove_worker(state.clone(), worker_name).await {
                                                        Ok(_) => {}
                                                        Err(_) => info!("❗清理全局变量失败 Code: {}", line!()),
                                                    }
                                                    //debug!("矿机任务写入失败 {:?}",job);
                                                    return Ok(());
                                                }

                                                dev_state_send.send((phread_id,queue_job)).expect("发送任务给开发者失败。");
                                                continue;
                                            } else {
                                                //几率不高。但是要打日志出来。
                                                //debug!("------------- 跳过本次抽水。没有任务处理了。。。3");
                                                log::error!(
                                                    "跳过本次抽水。没有任务处理了99"
                                                );
                                            }

                                        }
                                    }
                                }


                                let mut rng = ChaCha20Rng::from_entropy();
                                let secret_number = rng.gen_range(1..1000);

                                if config.share_rate <= 0.000 {
                                    config.share_rate = 0.005;
                                }
                                let max = (1000.0 * config.share_rate) as u32;
                                let max = 1000 - max; //900
                                match secret_number.cmp(&max) {
                                    Ordering::Less => {}
                                    _ => {
                                            {
                                                let mut jobs_queue =
                                                RwLockWriteGuard::map(state.write().await, |s| &mut s.mine_jobs_queue);
                                                if jobs_queue.len() > 0 {
                                                    let (phread_id,queue_job) = jobs_queue.pop_back().unwrap();
                                                    let job = serde_json::from_str::<Server>(&*queue_job)?;
                                                    //debug!("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!! {:?}",job);
                                                    let rpc = serde_json::to_vec(&job).expect("格式化RPC失败");
                                                    let mut byte = BytesMut::new();
                                                    byte.put_slice(&rpc[..]);
                                                    byte.put_u8(b'\n');

                                                    let w_len = w.write_buf(&mut byte).await?;
                                                    if w_len == 0 {
                                                        //debug!("矿机任务写入失败 {:?}",job);
                                                        let worker_name: String;
                                                        {
                                                            let rw_worker = RwLockReadGuard::map(worker.read().await, |s| s);
                                                            worker_name = rw_worker.clone();
                                                        }

                                                        match remove_worker(state.clone(), worker_name).await {
                                                            Ok(_) => {}
                                                            Err(_) => info!("❗清理全局变量失败 Code: {}", line!()),
                                                        }
                                                        return Ok(());
                                                    }
                                                    state_send.send((phread_id,queue_job)).expect("发送任务给抽水矿工失败。");

                                                    continue;
                                                } else {
                                                    //几率不高。但是要打日志出来。
                                                    //debug!("------------- 跳过本次抽水。没有任务处理了。。。3");
                                                    log::error!(
                                                        "跳过本次抽水。没有任务处理了88"
                                                    );
                                                }
                                            }
                                }
                            }
                        }

                        let mut byte = BytesMut::from(buf);
                        byte.put_u8(b'\n');
                        let len = w.write_buf(&mut byte).await?;
                        if len == 0 {
                            info!("❗ 服务端写入失败 断开连接.");
                            let worker_name: String;
                            {
                                let guard = worker.read().await;
                                let rw_worker = RwLockReadGuard::map(guard, |s| s);
                                worker_name = rw_worker.to_string();
                            }

                            info!("worker {} ",worker_name);
                            match remove_worker(state.clone(), worker_name).await {
                                Ok(_) => {}
                                Err(_) => info!("❗清理全局变量失败 Code: {}", line!()),
                            }
                            return Ok(());
                        }
                        continue;
                    } else {
                        log::error!(
                            "❗ ------未捕获封包:{:?}",
                            buf
                        );
                        // debug!(
                        //     "❗ ------未捕获封包:{:?}",
                        //     buf
                        // );

                        let mut byte = BytesMut::from(buf);

                        byte.put_u8(b'\n');
                        let len = w.write_buf(&mut byte).await?;
                        if len == 0 {
                            info!("❗ 服务端写入失败 断开连接.");
                            let worker_name: String;
                            {
                                let guard = worker.read().await;
                                let rw_worker = RwLockReadGuard::map(guard, |s| s);
                                worker_name = rw_worker.to_string();
                            }

                            info!("worker {} ",worker_name);
                            match remove_worker(state.clone(), worker_name).await {
                                Ok(_) => {}
                                Err(_) => info!("❗清理全局变量失败 Code: {}", line!()),
                            }
                            return Ok(());
                        }
                    }

                    // } else if let Ok(mut server_json_rpc) = serde_json::from_str::<ServerError>(&buf) {
                    //     let rw_worker = RwLockReadGuard::map(worker.read().await, |s| s);
                    //     let mut workers =
                    //     RwLockWriteGuard::map(state.write().await, |s| &mut s.workers);
                    //     let mut rpc_id = 0;
                    //     if let Some(w) = workers.get_mut(&rw_worker.clone()) {
                    //     // for w in &mut *workers {
                    //     //     if w.worker == *rw_worker {
                    //             w.invalid_index = w.invalid_index + 1;
                    //             rpc_id = w.share_index;
                    //     //     }
                    //     // }
                    //     }
                    //     // if let Ok(rpc) = serde_json::from_str::<BinanceError>(&rpc.error){
                    //     //     if rpc.code <=
                    //     // }

                    //     info!("❗ Worker :{} Share #{} Reject", rw_worker,rpc_id);
                    //     {
                    //         let rpc_id = RwLockReadGuard::map(client_rpc_id.read().await, |s| s);
                    //         server_json_rpc.id = *rpc_id;
                    //     }
                    //     let to_client_buf = serde_json::to_string(&server_json_rpc).expect("格式化RPC失败");
                    //     let mut byte = BytesMut::from(to_client_buf.as_str());
                    //     byte.put_u8(b'\n');
                    //     let len = w.write_buf(&mut byte).await?;
                    //     if len == 0 {
                    //         info!("❗ 服务端写入失败 断开连接.");
                    //         let worker_name: String;
                    //         {
                    //             let guard = worker.read().await;
                    //             let rw_worker = RwLockReadGuard::map(guard, |s| s);
                    //             worker_name = rw_worker.to_string();
                    //         }

                    //         info!("worker {} ",worker_name);
                    //         match remove_worker(state.clone(), worker_name).await {
                    //             Ok(_) => {}
                    //             Err(_) => info!("❗清理全局变量失败 Code: {}", line!()),
                    //         }
                    //         return Ok(());
                    //     }

                    //     continue;
                    //     // debug!(
                    //     //     "❗ ------未捕获封包:{:?}",
                    //     //     String::from_utf8(buf.clone().to_vec()).unwrap()
                    //     // );

                    }



            },
            id1 = rx.recv() => {
                let mut msg = id1.expect("解析Server封包错误");
                {
                    let rpc_id = RwLockReadGuard::map(client_rpc_id.read().await, |s| s);
                    msg.id = *rpc_id;
                }

                let rpc = serde_json::to_vec(&msg)?;
                let mut byte = BytesMut::new();
                byte.put_slice(&rpc[0..rpc.len()]);
                byte.put_u8(b'\n');
                let w_len = w.write_buf(&mut byte).await?;
                if w_len == 0 {
                    return Ok(());
                }
            }
        }
    }
}

async fn remove_worker(state: Arc<RwLock<State>>, worker: String) -> Result<()> {
    #[cfg(debug_assertions)]
    debug!("旷工下线 {}", worker);
    {
        let mut workers = RwLockWriteGuard::map(state.write().await, |s| &mut s.workers);
        if !worker.is_empty() && !workers.is_empty() {
            #[cfg(debug_assertions)]
            debug!("共有{}个旷工在线 ", workers.len());
            //let mut idx: usize = 0;
            if let Some(w) = workers.remove(&worker.clone()) {
                return Ok(());
            }
            // for idx in 0..workers.len() {
            //     #[cfg(debug_assertions)]
            //     info!("index {}, {:?}", idx, workers[idx]);
            //     if workers[idx].worker == worker {
            //         workers.remove(idx);
            //         return Ok(());
            //     }
            //     //idx = idx + 1;
            // }
        }
    }
    #[cfg(debug_assertions)]
    debug!("未找到旷工 {}", worker);
    return Ok(());
}

#[test]
fn test_remove_worker() {
    let mut a = RwLock::new(State::new());

    let mut worker_name: String;
    {
        worker_name = "test00001".to_string();
    }
}
