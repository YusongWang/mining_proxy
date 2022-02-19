use std::sync::Arc;

use async_channel::{Receiver, Sender};
use tokio::sync::RwLock;

use crate::util::config::Settings;

pub struct Proxy {
    pub fee_job: Arc<RwLock<Vec<String>>>,
    pub dev_fee_job: Arc<RwLock<Vec<String>>>,
    pub config: Arc<RwLock<Settings>>,
    pub recv: Receiver<crate::client::FEE>,
    pub send: Sender<crate::client::FEE>,
}
