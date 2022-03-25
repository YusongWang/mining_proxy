use std::sync::Arc;

use tokio::{
    sync::{broadcast::Sender, mpsc::UnboundedSender, RwLock},
};

//use tokio_native_tls::TlsStream;

use crate::{
    state::Worker, util::config::Settings,
};

pub struct Proxy {
    pub config: Arc<RwLock<Settings>>,
    pub chan: Sender<Vec<String>>,
    pub dev_chan: Sender<Vec<String>>,
    pub tx: tokio::sync::mpsc::Sender<Vec<String>>,
    pub dev_tx:
        tokio::sync::mpsc::Sender<Vec<String>>,
    pub worker_tx: UnboundedSender<Worker>,
    // pub proxy_write: Arc<Mutex<Box<dyn AsyncWrite + Send + Sync + Unpin>>>,
    // pub dev_write: Arc<Mutex<Box<dyn AsyncWrite + Send + Sync + Unpin>>>,
}
