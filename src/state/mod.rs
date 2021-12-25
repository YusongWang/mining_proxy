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
        info!("worker {} 请求登录", worker);
        self.worker = worker;
        self.worker_name = worker_name;
        self.worker_wallet = worker_wallet;
    }

    pub fn logind(&mut self) {
        info!("worker {} 登录成功", self.worker);
        self.online = true;
        self.clear_state();
    }

    // 下线
    pub fn offline(&mut self) -> bool {
        self.online = false;
        //TODO 清理读写线程。然后直接返回.
        info!("worker {} 离开了。", self.worker);
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
        info!("✅ worker {} 份额 增加 {}", self.worker, self.share_index);
    }
    // 接受份额
    pub fn share_accept(&mut self) {
        self.accept_index += 1;
        info!("✅ worker {} 接受份额 {}", self.worker, self.accept_index);
    }

    // 拒绝的份额
    pub fn share_reject(&mut self) {
        self.invalid_index += 1;
        info!("❗ worker {} 拒绝份额 {}", self.worker, self.invalid_index);
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

pub struct InnerJobs {
    pub mine_jobs: Arc<RwLock<HashMap<String, u64>>>,
}

//TODO 分成四个变量是否可以提升速度。减少同一变量写锁的时间
// 或者就保留目前的全局状态值。方便处理。
#[derive(Debug)]
pub struct State {
    pub report_hashrate: HashMap<String, String>,
    pub workers: HashMap<String, Worker>,
    pub proxy_jobs: HashSet<String>,
    pub proxy_share: u64,
    pub mine_jobs: HashMap<String, u64>,
    pub mine_jobs_queue: VecDeque<(u64, String)>,
    pub mine_share: u64,
    pub develop_jobs: HashMap<String, u64>,
    pub develop_jobs_queue: VecDeque<(u64, String)>,
    pub develop_share: u64,
}

impl State {
    pub fn new() -> Self {
        Self {
            report_hashrate: HashMap::new(),
            proxy_jobs: HashSet::new(),
            mine_jobs: HashMap::new(),
            develop_jobs: HashMap::new(),
            proxy_share: 0,
            mine_share: 0,
            develop_share: 0,
            mine_jobs_queue: VecDeque::new(),
            develop_jobs_queue: VecDeque::new(),
            workers: HashMap::new(),
        }
    }
}
