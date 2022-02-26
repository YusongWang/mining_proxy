use std::sync::Arc;

use tokio::{
    io::{AsyncRead, AsyncWrite, WriteHalf},
    net::TcpStream,
    sync::{broadcast::Sender, mpsc::UnboundedSender, Mutex, RwLock},
};
use tokio_native_tls::TlsStream;

use crate::{state::Worker, util::config::Settings};

pub struct Proxy {
    pub config: Arc<RwLock<Settings>>,
    pub chan: Sender<Vec<String>>,
    pub dev_chan: Sender<Vec<String>>,
    pub proxy_write: Arc<Mutex<WriteHalf<TcpStream>>>,
    pub dev_write: Arc<Mutex<WriteHalf<TlsStream<TcpStream>>>>,
    pub worker_tx: UnboundedSender<Worker>,
}
