use std::{collections::VecDeque, sync::Arc};

use async_channel::{Receiver, Sender};
use broadcaster::BroadcastChannel;
use tokio::{
    io::WriteHalf,
    net::TcpStream,
    sync::{mpsc::UnboundedSender, Mutex, RwLock},
};

use crate::{state::Worker, util::config::Settings};

pub struct Proxy {
    pub config: Arc<RwLock<Settings>>,
    pub chan: BroadcastChannel<Vec<String>>,
    pub dev_chan: BroadcastChannel<Vec<String>>,
    // pub job: Arc<RwLock<VecDeque<Vec<String>>>>,
    // pub job_recv: Receiver<Vec<String>>,
    // pub job_send: Sender<Vec<String>>,
    pub proxy_write: Arc<Mutex<WriteHalf<TcpStream>>>,
    pub dev_write: Arc<Mutex<WriteHalf<TcpStream>>>,
    pub worker_tx: UnboundedSender<Worker>,
}
