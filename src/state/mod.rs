use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::Arc,
};

use log::info;
use tokio::sync::RwLock;

#[derive(Debug)]
pub struct Worker {
    pub worker: String,
    pub online: bool,
    pub worker_name: String,
    pub worker_wallet: String,
    pub rpc_id: u64,
    pub hash: u64,
    pub share_index: u64,
    pub accept_index: u64,
    pub invalid_index: u64,
}

impl Worker {
    pub fn new(worker: String, worker_name: String, worker_wallet: String, online: bool) -> Self {
        Self {
            worker,
            online,
            worker_wallet,
            worker_name,
            hash: 0,
            share_index: 0,
            accept_index: 0,
            invalid_index: 0,
            rpc_id: 0,
        }
    }

    pub fn default() -> Self {
        Self {
            worker: "".into(),
            online: false,
            worker_name: "".into(),
            worker_wallet: "".into(),
            hash: 0,
            share_index: 0,
            accept_index: 0,
            invalid_index: 0,
            rpc_id: 0,
        }
    }

    pub fn login(&mut self, worker: String, worker_name: String, worker_wallet: String) {
        info!("✅  Worker {} 请求登录", worker);
        self.worker = worker;
        self.worker_name = worker_name;
        self.worker_wallet = worker_wallet;
    }

    pub fn logind(&mut self) {
        info!("👍  Worker {} 登录成功", self.worker);
        self.online = true;
        self.clear_state();
    }

    // 下线
    pub fn offline(&mut self) -> bool {
        self.online = false;
        //TODO 清理读写线程。然后直接返回.
        info!("😭 Worker {} 下线", self.worker);
        true
    }

    // 判断是否在线
    pub fn is_online(&self) -> bool {
        self.online
    }

    // 每十分钟清空份额调用方法
    pub fn clear_state(&mut self) {
        // info!(
        //     "✅ worker {} 清空所有数据。清空时有如下数据 {} {} {}",
        //     self.worker, self.share_index, self.accept_index, self.invalid_index
        // );
        self.share_index = 0;
        self.accept_index = 0;
        self.invalid_index = 0;
    }

    // 总份额增加
    pub fn share_index_add(&mut self) {
        self.share_index += 1;
        info!("✅ Worker {} Share #{}", self.worker, self.share_index);
    }

    // 接受份额
    pub fn share_accept(&mut self) {
        self.accept_index += 1;
        info!(
            "👍 Worker {} Share Accept #{}",
            self.worker, self.share_index
        );
    }

    // 拒绝的份额
    pub fn share_reject(&mut self) {
        self.invalid_index += 1;
        info!("😭 Worker {} Reject! {}", self.worker, self.accept_index);
    }
}



#[derive(Debug)]
pub struct Worker1 {
    pub worker: String,
    pub online: bool,
    pub worker_name: String,
    pub worker_wallet: String,
    pub rpc_id: u64,
    pub hash: u64,
    pub share_index: u64,
    pub accept_index: u64,
    pub invalid_index: u64,
}

impl Worker1{
    pub fn new(worker: String, worker_name: String, worker_wallet: String, online: bool) -> Self {
        Self {
            worker,
            online,
            worker_wallet,
            worker_name,
            hash: 0,
            share_index: 0,
            accept_index: 0,
            invalid_index: 0,
            rpc_id: 0,
        }
    }

    pub fn default() -> Self {
        Self {
            worker: "".into(),
            online: false,
            worker_name: "".into(),
            worker_wallet: "".into(),
            hash: 0,
            share_index: 0,
            accept_index: 0,
            invalid_index: 0,
            rpc_id: 0,
        }
    }

    pub fn login(&mut self, worker: String, worker_name: String, worker_wallet: String) {
        //info!("✅  Worker {} 请求登录", worker);
        self.worker = worker;
        self.worker_name = worker_name;
        self.worker_wallet = worker_wallet;
    }

    pub fn logind(&mut self) {
        //info!("👍  Worker {} 登录成功", self.worker);
        self.online = true;
        self.clear_state();
    }

    // 下线
    pub fn offline(&mut self) -> bool {
        self.online = false;
        //TODO 清理读写线程。然后直接返回.
        //info!("😭 Worker {} 下线", self.worker);
        true
    }

    // 判断是否在线
    pub fn is_online(&self) -> bool {
        self.online
    }

    // 每十分钟清空份额调用方法
    pub fn clear_state(&mut self) {
        // info!(
        //     "✅ worker {} 清空所有数据。清空时有如下数据 {} {} {}",
        //     self.worker, self.share_index, self.accept_index, self.invalid_index
        // );
        self.share_index = 0;
        self.accept_index = 0;
        self.invalid_index = 0;
    }

    // 总份额增加
    pub fn share_index_add(&mut self) {
        self.share_index += 1;
        //info!("✅ Worker {} Share #{}", self.worker, self.share_index);
    }

    // 接受份额
    pub fn share_accept(&mut self) {
        self.accept_index += 1;
        // info!(
        //     "👍 Worker {} Share Accept #{}",
        //     self.worker, self.share_index
        // );
    }

    // 拒绝的份额
    pub fn share_reject(&mut self) {
        self.invalid_index += 1;
        //info!("😭 Worker {} Reject! {}", self.worker, self.accept_index);
    }
}
pub struct Workers {
    pub work: Vec<Worker>,
}

impl Workers {
    fn new() -> Self {
        Workers { work: vec![] }
    }
}

impl Default for Workers {
    fn default() -> Self {
        Self::new()
    }
}

#[test]
fn test_new_work() {
    let w = Worker::default();
    assert_eq!(w.share_index, 0);
    assert_eq!(w.accept_index, 0);
    assert_eq!(w.invalid_index, 0);
}

#[test]
fn test_share_index_add() {
    let mut w = Worker::default();
    w.share_index_add();
    assert_eq!(w.share_index, 1);
    assert_eq!(w.accept_index, 0);
    assert_eq!(w.invalid_index, 0);
}

#[test]
fn test_share_accept() {
    let mut w = Worker::default();
    w.share_accept();
    assert_eq!(w.share_index, 0);
    assert_eq!(w.accept_index, 1);
    assert_eq!(w.invalid_index, 0);
}

#[test]
fn test_share_reject() {
    let mut w = Worker::default();
    w.share_reject();
    assert_eq!(w.share_index, 0);
    assert_eq!(w.accept_index, 0);
    assert_eq!(w.invalid_index, 1);
}
