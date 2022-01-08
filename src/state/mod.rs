use std::{
    sync::{
        atomic::{AtomicU32, AtomicU64},
        Arc,
    },
    time::Instant,
};

use log::{debug, info};
#[derive(Debug, Clone, PartialEq)]
pub struct Worker {
    pub worker: String,
    pub online: bool,
    pub worker_name: String,
    pub worker_wallet: String,
    pub login_time: Instant,
    pub last_subwork_time: Instant,
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
            login_time: Instant::now(),
            last_subwork_time: Instant::now(),
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
            login_time: Instant::now(),
            last_subwork_time: Instant::now(),
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
        if self.is_online() {
            info!(
                "矿工 {} 下线 在线时长 {}",
                self.worker,
                crate::util::time_to_string(self.login_time.elapsed().as_secs())
            );
        } else {
            info!("恶意攻击");
        }

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
        self.last_subwork_time = Instant::now();

        self.share_index += 1;
        debug!("✅ Worker {} Share #{}", self.worker, self.share_index);
    }

    // 接受份额
    pub fn share_accept(&mut self) {
        self.accept_index += 1;
        debug!(
            "👍 Worker {} Share Accept #{}",
            self.worker, self.share_index
        );
    }

    // 拒绝的份额
    pub fn share_reject(&mut self) {
        self.invalid_index += 1;
        debug!("😭 Worker {} Reject! {}", self.worker, self.accept_index);
    }

    pub fn submit_hashrate<T>(&mut self, rpc: &T) -> bool
    where
        T: crate::protocol::rpc::eth::ClientRpc,
    {
        self.hash = rpc.get_submit_hashrate();
        true
    }
}

pub type State = Arc<GlobalState>;

pub struct GlobalState {
    pub online: AtomicU32,
    pub proxy_share: AtomicU64,
    pub proxy_accept: AtomicU64,
    pub proxy_reject: AtomicU64,
    pub develop_share: AtomicU64,
    pub develop_accept: AtomicU64,
    pub develop_reject: AtomicU64,
}

impl GlobalState {
    pub fn new() -> Self {
        GlobalState {
            online: AtomicU32::new(0),
            proxy_share: AtomicU64::new(0),
            proxy_accept: AtomicU64::new(0),
            proxy_reject: AtomicU64::new(0),
            develop_share: AtomicU64::new(0),
            develop_accept: AtomicU64::new(0),
            develop_reject: AtomicU64::new(0),
        }
    }
}

impl Default for GlobalState {
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
