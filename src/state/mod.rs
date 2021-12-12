use std::collections::{HashMap, HashSet};

//TODO 分成四个变量是否可以提升速度。减少同一变量写锁的时间
// 或者就保留目前的全局状态值。方便处理。
#[derive(Debug)]
pub struct State {
    pub report_hashrate: HashMap<String, String>,
    pub proxy_jobs: HashSet<String>,
    pub proxy_share: u64,
    pub mine_jobs: HashSet<String>,
    pub mine_jobs_queue: HashSet<String>,
    pub mine_share: u64,
    pub develop_jobs: HashSet<String>,
    pub develop_share: u64,
}

impl State {
    pub fn new() -> Self {
        Self {
            report_hashrate: HashMap::new(),
            proxy_jobs: HashSet::new(),
            mine_jobs: HashSet::new(),
            develop_jobs: HashSet::new(),
            proxy_share: 0,
            mine_share: 0,
            develop_share: 0,
            mine_jobs_queue: HashSet::new(),
        }
    }
}

