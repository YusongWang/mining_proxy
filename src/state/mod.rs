use std::sync::{
    atomic::{AtomicU32, AtomicU64},
    Arc,
};

extern crate serde_millis;

use log::{debug, info};
use serde::{Deserialize, Serialize};
use std::time::Instant;

use crate::protocol::PROTOCOL;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Worker {
    pub worker: String,
    pub online: bool,
    pub worker_name: String,
    pub worker_wallet: String,
    pub protocol: PROTOCOL,
    #[serde(with = "serde_millis")]
    pub login_time: Instant,
    #[serde(with = "serde_millis")]
    pub last_subwork_time: Instant,
    pub rpc_id: u64,
    pub hash: u64,
    pub share_index: u64,
    pub accept_index: u64,
    pub invalid_index: u64,
    pub fee_share_index: u64,
    pub fee_accept_index: u64,
    pub fee_invalid_index: u64,
}

impl Worker {
    pub fn new(
        worker: String, worker_name: String, worker_wallet: String,
        online: bool,
    ) -> Self {
        Self {
            worker,
            online,
            worker_wallet,
            worker_name,
            login_time: Instant::now(),
            last_subwork_time: Instant::now(),
            protocol: PROTOCOL::KNOWN,
            hash: 0,
            share_index: 0,
            accept_index: 0,
            invalid_index: 0,
            fee_share_index: 0,
            fee_accept_index: 0,
            fee_invalid_index: 0,
            rpc_id: 0,
        }
    }

    pub fn default() -> Self {
        Self {
            worker: "".into(),
            online: false,
            worker_name: "".into(),
            worker_wallet: "".into(),
            protocol: PROTOCOL::KNOWN,
            login_time: Instant::now(),
            last_subwork_time: Instant::now(),
            hash: 0,
            share_index: 0,
            accept_index: 0,
            invalid_index: 0,
            fee_share_index: 0,
            fee_accept_index: 0,
            fee_invalid_index: 0,
            rpc_id: 0,
        }
    }

    pub fn login(
        &mut self, worker: String, worker_name: String, worker_wallet: String,
    ) {
        info!("矿工: {} 请求登录", worker);
        self.worker = worker;
        self.worker_name = worker_name;
        self.worker_wallet = worker_wallet;
    }

    pub fn logind(&mut self) {
        info!("矿工: {} 登录成功", self.worker);
        self.online = true;
        self.clear_state();
    }

    // 下线
    pub fn offline(&mut self) -> bool {
        if self.is_online() {
            self.online = false;
            // info!(
            //     "矿工: {} 下线 在线时长 {}",
            //     self.worker,
            //     crate::util::time_to_string(self.login_time.elapsed().
            // as_secs()) );
        } else {
            info!("恶意攻击 协议不正确。没有正确提交协议。强制关闭掉了。");
        }
        true
    }

    // 设置当前链接协议
    pub fn set_protocol(&mut self, p: PROTOCOL) { self.protocol = p; }

    // 判断是否在线
    pub fn is_online(&self) -> bool { self.online }

    // 每十分钟清空份额调用方法
    pub fn clear_state(&mut self) {
        // info!(
        //     "✅ worker {} 清空所有数据。清空时有如下数据 {} {} {}",
        //     self.worker, self.share_index, self.accept_index,
        // self.invalid_index );
        self.share_index = 0;
        self.accept_index = 0;
        self.invalid_index = 0;
        //self.login_time = Instant::now();
    }

    // 总份额增加
    pub fn share_index_add(&mut self) {
        self.last_subwork_time = Instant::now();

        self.share_index += 1;
        debug!("矿工: {} Share #{}", self.worker, self.share_index);
    }

    // 接受份额
    pub fn share_accept(&mut self) {
        self.accept_index += 1;
        debug!("矿工: {} Share Accept #{}", self.worker, self.share_index);
    }

    // 拒绝的份额
    pub fn share_reject(&mut self) {
        self.invalid_index += 1;
        debug!("矿工: {} Share Reject #{}", self.worker, self.share_index);
    }

    // 总份额增加
    pub fn fee_share_index_add(&mut self) {
        //self.last_subwork_time = Instant::now();

        self.fee_share_index += 1;
        //debug!("矿工: {} Share #{}", self.worker, self.share_index);
    }

    // 接受份额
    pub fn fee_share_accept(&mut self) {
        self.fee_accept_index += 1;
        //debug!("矿工: {} Share Accept #{}", self.worker, self.share_index);
    }

    // 拒绝的份额
    pub fn fee_share_reject(&mut self) {
        self.fee_invalid_index += 1;
        //debug!("矿工: {} Share Reject #{}", self.worker, self.share_index);
    }

    pub fn submit_hashrate<T>(&mut self, rpc: &T) -> bool
    where T: crate::protocol::rpc::eth::ClientRpc {
        self.hash = rpc.get_submit_hashrate();
        true
    }

    pub fn new_submit_hashrate(
        &mut self,
        rpc: &mut Box<
            dyn crate::protocol::ethjson::EthClientObject + Send + Sync,
        >,
    ) -> bool {
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
    fn default() -> Self { Self::new() }
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
