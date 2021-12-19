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
    protocol::rpc::eth::{Client, ClientGetWork, Server, ServerId1},
    state::{State, Worker},
    util::{config::Settings, hex_to_int},
};

pub mod tcp;
pub mod tls;

async fn client_to_server<R, W>(
    state: Arc<RwLock<State>>,
    worker: Arc<RwLock<String>>,
    _: Settings,
    mut r: ReadHalf<R>,
    mut w: WriteHalf<W>,
    //state_send: UnboundedSender<String>,
    proxy_fee_sender: UnboundedSender<String>,
    dev_fee_send: UnboundedSender<String>,
    tx: UnboundedSender<ServerId1>,
) -> Result<(), std::io::Error>
where
    R: AsyncRead,
    W: AsyncWrite,
{
    loop {
        let mut buf = vec![0; 1024];
        let len = r.read(&mut buf).await?;
        info!("读取成功{} 字节", len);

        if len == 0 {
            let mut worker_name: String;
            {
                let rw_worker = RwLockReadGuard::map(worker.read().await, |s| s);
                worker_name = rw_worker.clone();
            }

            {
                match remove_worker(state.clone(), worker_name.clone()).await {
                    Ok(_) => {}
                    Err(_) => info!("❗清理全局变量失败 Code: {}", line!()),
                }
            }


            info!("Worker {} 客户端断开连接.", worker_name);
            return Ok(());
        }

        match String::from_utf8(buf[0..len].to_vec()) {
            Ok(rpc) => {
                debug!("矿机 -> 矿池 #{:?}", rpc);
            }
            Err(_) => {
                info!("格式化为字符串失败。{:?}", buf[0..len].to_vec());
                return Ok(());
            }
        }

        // debug!(
        //     "矿机 -> 矿池 #{:?}",
        //     String::from_utf8(buf[0..len].to_vec()).unwrap()
        // );

        if len > 5 {
            if let Ok(client_json_rpc) = serde_json::from_slice::<Client>(&buf[0..len]) {
                if client_json_rpc.method == "eth_submitWork" {
                    let mut rpc_id = 0;
                    //TODO 重构随机数函数。
                    {
                        //新增一个share
                        let mut workers =
                            RwLockWriteGuard::map(state.write().await, |s| &mut s.workers);

                        let rw_worker = RwLockReadGuard::map(worker.read().await, |s| s);
                        for w in &mut *workers {
                            if w.worker == *rw_worker {
                                w.share_index = w.share_index + 1;
                                rpc_id = w.share_index;
                            }
                        }
                        //debug!("✅ Worker :{} Share #{}", client_json_rpc.worker, *mapped);
                    }

                    if let Some(job_id) = client_json_rpc.params.get(1) {
                        {
                            let mut mine =
                                RwLockWriteGuard::map(state.write().await, |s| &mut s.mine_jobs);
                            if mine.contains(job_id) {
                                mine.remove(job_id);

                                let rpc = serde_json::to_string(&client_json_rpc)?;
                                // TODO
                                //debug!("------- 收到 指派任务。可以提交给矿池了 {:?}", job_id);
                                proxy_fee_sender
                                    .send(rpc)
                                    .expect("可以提交给矿池任务失败。通道异常了");

                                let s = ServerId1 {
                                    id: client_json_rpc.id,
                                    //jsonrpc: "2.0".into(),
                                    result: true,
                                };

                                tx.send(s).expect("可以提交矿机结果失败。通道异常了");
                                continue;
                            }
                            //debug!("✅ Worker :{} Share #{}", client_json_rpc.worker, *mapped);
                        }

                        {
                            let mut mine =
                                RwLockWriteGuard::map(state.write().await, |s| &mut s.develop_jobs);
                            if mine.contains(job_id) {
                                mine.remove(job_id);

                                let rpc = serde_json::to_string(&client_json_rpc)?;
                                //debug!("------- 收到 指派任务。可以提交给矿池了 {:?}", job_id);
                                dev_fee_send
                                    .send(rpc)
                                    .expect("可以提交给矿池任务失败。通道异常了");

                                let s = ServerId1 {
                                    id: client_json_rpc.id,
                                    //jsonrpc: "2.0".into(),
                                    result: true,
                                };

                                tx.send(s).expect("可以提交给矿机结果失败。通道异常了");
                                continue;
                            }
                            //debug!("✅ Worker :{} Share #{}", client_json_rpc.worker, *mapped);
                        }
                    }
                    {
                        let rw_worker = RwLockReadGuard::map(worker.read().await, |s| s);
                        info!("✅ Worker :{} Share #{}", rw_worker, rpc_id);
                    }
                } else if client_json_rpc.method == "eth_submitHashrate" {
                    if let Some(hashrate) = client_json_rpc.params.get(0) {
                        {
                            let mut workers =
                                RwLockWriteGuard::map(state.write().await, |s| &mut s.workers);

                            let rw_worker = RwLockReadGuard::map(worker.read().await, |s| s);
                            for w in &mut *workers {
                                if w.worker == *rw_worker {
                                    if let Some(h) = hex_to_int(&hashrate[2..hashrate.len()]) {
                                        w.hash = (h as u64) / 1000 / 1000;
                                    }
                                }
                            }
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
                    if let Some(wallet) = client_json_rpc.params.get(0) {
                        let mut temp_worker = wallet.clone();
                        temp_worker.push_str(".");
                        temp_worker = temp_worker + client_json_rpc.worker.as_str();
                        let mut rw_worker = RwLockWriteGuard::map(worker.write().await, |s| s);
                        *rw_worker = temp_worker.clone();
                        info!("✅ Worker :{} 请求登录", *rw_worker);
                    } else {
                        //debug!("❎ 登录错误，未找到登录参数");
                    }
                } else {
                    //debug!("❎ Worker {} 传递未知RPC :{:?}", worker, client_json_rpc);
                }

                let write_len = w.write(&buf[0..len]).await?;
                if write_len == 0 {
                    let worker_name: String;
                    {
                        let rw_worker = RwLockReadGuard::map(worker.read().await, |s| s);
                        worker_name = rw_worker.clone();
                    }

                    match remove_worker(state.clone(), worker_name.clone()).await {
                        Ok(_) => {}
                        Err(_) => info!("❗清理全局变量失败 Code: {}", line!()),
                    }

                    info!("✅ Worker: {} 服务器断开连接.", worker_name);
                    return Ok(());
                }
            } else if let Ok(_) = serde_json::from_slice::<ClientGetWork>(&buf[0..len]) {
                //debug!("获得任务:{:?}", client_json_rpc);

                //info!("🚜 Worker: {} 请求计算任务", worker);
                let write_len = w.write(&buf[0..len]).await?;
                if write_len == 0 {
                    let worker_name: String;
                    {
                        let rw_worker = RwLockReadGuard::map(worker.read().await, |s| s);
                        worker_name = rw_worker.clone();
                    }

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

async fn server_to_client<R, W>(
    state: Arc<RwLock<State>>,
    worker: Arc<RwLock<String>>,
    mut config: Settings,
    _: broadcast::Receiver<String>,
    mut r: ReadHalf<R>,
    mut w: WriteHalf<W>,
    _: UnboundedSender<String>,
    state_send: UnboundedSender<String>,
    dev_state_send: UnboundedSender<String>,
    mut rx: UnboundedReceiver<ServerId1>,
) -> Result<(), std::io::Error>
where
    R: AsyncRead,
    W: AsyncWrite,
{
    let mut is_login = false;

    loop {
        let mut buf = vec![0; 1024];

        tokio::select! {
            len = r.read(&mut buf) => {
                let len = match len{
                    Ok(len) => len,
                    Err(e) => {
                        info!("{:?}",e);
                        return anyhow::private::Err(e);
                    },
                };


                if len == 0 {
                    let worker_name: String;
                    {
                        let rw_worker = RwLockReadGuard::map(worker.read().await, |s| s);
                        worker_name = rw_worker.clone();
                    }

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
                let buffer = buf[0..len].split(|c| *c == b'\n');
                for buf in buffer {
                    if buf.is_empty() {
                        continue;
                    }

                debug!("Got jobs {}",String::from_utf8(buf.to_vec()).unwrap());
                if !is_login {
                    if let Ok(server_json_rpc) = serde_json::from_slice::<ServerId1>(&buf) {
                        if server_json_rpc.id == 1 && server_json_rpc.result {
                            let rw_worker = RwLockReadGuard::map(worker.read().await, |s| s);
                            let wallet:Vec<_>= rw_worker.split(".").collect();
                            let mut workers =
                            RwLockWriteGuard::map(state.write().await, |s| &mut s.workers);
                            workers.push(Worker::new(
                                rw_worker.clone(),
                                wallet[1].clone().to_string(),
                                wallet[0].clone().to_string(),
                            ));
                            is_login = true;
                            info!("✅ {} 登录成功",rw_worker);
                        } else {
                            let rw_worker = RwLockReadGuard::map(worker.read().await, |s| s);
                            info!("❎ {} 登录失败",rw_worker);
                            return Ok(());
                        }

                    } else {
                        let rw_worker = RwLockReadGuard::map(worker.read().await, |s| s);
                        info!("❎ {} 登录失败 01",rw_worker);
                        // debug!(
                        //     "❎ 登录失败{:?}",
                        //     String::from_utf8(buf.clone().to_vec()).unwrap()
                        // );
                        return Ok(());
                    }
                } else {
                    if let Ok(server_json_rpc) = serde_json::from_slice::<ServerId1>(&buf) {

                        let rw_worker = RwLockReadGuard::map(worker.read().await, |s| s);
                        let mut workers =
                        RwLockWriteGuard::map(state.write().await, |s| &mut s.workers);
                        if server_json_rpc.id == 6{
                            continue;
                        } else if server_json_rpc.result {
                            let mut rpc_id = 0;
                            for w in &mut *workers {
                                if w.worker == *rw_worker {
                                    w.accept_index = w.accept_index + 1;
                                    rpc_id = w.share_index;
                                }
                            }
                            info!("👍 Worker :{} Share #{} Accept", rw_worker,rpc_id);
                        } else {
                            let mut rpc_id = 0;
                            for w in &mut *workers {
                                if w.worker == *rw_worker {
                                    w.invalid_index = w.invalid_index + 1;
                                    rpc_id = w.share_index;
                                }
                            }

                            info!("❗ Worker :{} Share #{} Reject", rw_worker,rpc_id);
                        }
                    } else if let Ok(_) = serde_json::from_slice::<Server>(&buf) {

                            if config.share != 0 {
                                {
                                    let mut rng = ChaCha20Rng::from_entropy();
                                    let secret_number = rng.gen_range(1..1000);

                                    let max = (1000.0 * crate::FEE) as u32;
                                    let max = 1000 - max; //900
                                    match secret_number.cmp(&max) {
                                        Ordering::Less => {}
                                        _ => {
                                                //debug!("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!");
                                                // let mut jobs_queue =
                                                //      RwLockWriteGuard::map(state.write().await, |s| &mut s);
                                                //state.lock().await();
                                                // 将任务加入队列。
                                                {
                                                    let mut jobs_queue =
                                                    RwLockWriteGuard::map(state.write().await, |s| &mut s.develop_jobs_queue);
                                                    if jobs_queue.len() > 0 {
                                                        let a = jobs_queue.pop_back().unwrap();
                                                        let job = serde_json::from_str::<Server>(&*a)?;
                                                        //debug!("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!! {:?}",job);
                                                        let rpc = serde_json::to_vec(&job).expect("格式化RPC失败");
                                                        let mut byte = BytesMut::new();
                                                        byte.put_slice(&rpc[..]);
                                                        byte.put_u8(b'\n');
                                                        //debug!("发送指派任务给矿机 {:?}",job);
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

                                                        let b = a.clone();
                                                        dev_state_send.send(b).expect("发送任务给开发者失败。");

                                                        continue;
                                                    } else {
                                                        //几率不高。但是要打日志出来。
                                                        debug!("------------- 跳过本次抽水。没有任务处理了。。。3");
                                                    }
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
                                            //debug!("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!");
                                            // let mut jobs_queue =
                                            //      RwLockWriteGuard::map(state.write().await, |s| &mut s);
                                            //state.lock().await();
                                            // 将任务加入队列。
                                            {
                                                let mut jobs_queue =
                                                RwLockWriteGuard::map(state.write().await, |s| &mut s.mine_jobs_queue);
                                                if jobs_queue.len() > 0{
                                                    let a = jobs_queue.pop_back().unwrap();
                                                    let job = serde_json::from_str::<Server>(&*a)?;
                                                    //debug!("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!! {:?}",job);
                                                    let rpc = serde_json::to_vec(&job).expect("格式化RPC失败");
                                                    let mut byte = BytesMut::new();
                                                    byte.put_slice(&rpc[..]);
                                                    byte.put_u8(b'\n');
                                                    //debug!("发送指派任务给矿机 {:?}",job);
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

                                                    let b = a.clone();
                                                    state_send.send(b).expect("发送任务给抽水矿工失败。");

                                                    continue;
                                                } else {
                                                    //几率不高。但是要打日志出来。
                                                    debug!("------------- 跳过本次抽水。没有任务处理了。。。3");
                                                }
                                            }
                                }
                            }
                        }

                    } else {
                        debug!(
                            "❗ ------未捕获封包:{:?}",
                            String::from_utf8(buf.clone().to_vec()).unwrap()
                        );
                    }
                }

                let len = w.write(&buf).await?;
                if len == 0 {
                    info!("❗ 服务端写入失败 断开连接.");
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
            }
            },
            id1 = rx.recv() => {
                let msg = id1.expect("解析Server封包错误");

                let rpc = serde_json::to_vec(&msg)?;
                let mut byte = BytesMut::new();
                byte.put_slice(&rpc[0..rpc.len()]);
                byte.put_u8(b'\n');
                let w_len = w.write_buf(&mut byte).await?;
                if w_len == 0 {
                    return Ok(());
                }
            },
            // job = jobs_recv.recv() => {
            //     let job = job.expect("解析Server封包错误");
            //     let rpc = serde_json::from_str::<Server>(&job)?;
            //     let rpc = serde_json::to_vec(&rpc).expect("格式化RPC失败");
            //     //TODO 判断work是发送给那个矿机的。目前全部接受。
            //     debug!("发送指派任务给矿机 {:?}",job);
            //     let mut byte = BytesMut::new();
            //     byte.put_slice(&rpc[..]);
            //     byte.put_u8(b'\n');
            //     let w_len = w.write_buf(&mut byte).await?;
            //     if w_len == 0 {
            //         return Ok(());
            //     }
            // }
        }
    }
}

async fn remove_worker(state: Arc<RwLock<State>>, worker: String) -> Result<()> {

    info!("旷工下线 {}", worker);
    let mut workers = RwLockWriteGuard::map(state.write().await, |s| &mut s.workers);
    if !worker.is_empty() {
        let idx: usize = 0;
        while idx <= workers.len() {
            if workers[idx].worker == worker {
                workers.remove(idx);
                return Ok(());
            }
        }
    }
    info!("未找到旷工 {}", worker);
    return Ok(());
}

#[test]
fn test_remove_worker() {
    let mut a = Arc::new(RwLock::new(State::new()));
    {}

    let mut worker_name: String;
    {
        worker_name = "test00001".to_string();
    }
}
