use std::sync::Arc;

use async_channel::{Receiver, Sender};
use broadcaster::BroadcastChannel;
use tokio::{
    io::WriteHalf,
    net::TcpStream,
    sync::{Mutex, RwLock},
};

use crate::util::config::Settings;

pub struct Proxy {
    pub config: Arc<RwLock<Settings>>,
    pub chan: BroadcastChannel<Vec<String>>,
    pub job: Arc<RwLock<VecDeque<Vec<String>>>>,
    pub job_recv: Receiver<Vec<String>>,
    pub job_send: Sender<Vec<String>>,
    pub proxy_write: Arc<Mutex<WriteHalf<TcpStream>>>,
}
