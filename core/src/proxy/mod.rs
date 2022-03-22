use std::sync::Arc;

use tokio::{
    io::{AsyncWrite, WriteHalf},
    //net::TcpStream,
    sync::{broadcast::Sender, mpsc::UnboundedSender, Mutex, RwLock},
};

//use tokio_native_tls::TlsStream;

use crate::{
    protocol::ethjson::EthClientObject, state::Worker, util::config::Settings,
};

pub struct Proxy {
    pub config: Arc<RwLock<Settings>>,
    pub chan: Sender<Vec<String>>,
    pub dev_chan: Sender<Vec<String>>,
    pub tx: tokio::sync::mpsc::Sender<Box<dyn EthClientObject + Send + Sync>>,
    pub dev_tx:
        tokio::sync::mpsc::Sender<Box<dyn EthClientObject + Send + Sync>>,
    pub worker_tx: UnboundedSender<Worker>,

    pub proxy_write: Arc<Mutex<Box<dyn AsyncWrite + Send + Sync>>>,
    pub dev_write: Arc<Mutex<Box<dyn AsyncWrite + Send + Sync>>>,
}
