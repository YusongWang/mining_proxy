use std::{sync::Arc, collections::VecDeque};

use tokio::sync::{broadcast::Sender, mpsc::UnboundedSender, RwLock, Mutex};

use crate::{state::Worker, util::config::Settings};

pub type Job =    Arc<RwLock<VecDeque<Vec<String>>>>;


pub struct Proxy {
    pub config: Arc<RwLock<Settings>>,
    // pub chan: Sender<Vec<String>>,
    // pub dev_chan: Sender<Vec<String>>,
    pub fee_job:Job,
    pub develop_job:Job,
    pub tx: tokio::sync::mpsc::Sender<Vec<String>>,
    pub dev_tx: tokio::sync::mpsc::Sender<Vec<String>>,
    pub worker_tx: UnboundedSender<Worker>,
    // pub proxy_write: Arc<Mutex<Box<dyn AsyncWrite + Send + Sync + Unpin>>>,
    // pub dev_write: Arc<Mutex<Box<dyn AsyncWrite + Send + Sync + Unpin>>>,
}
