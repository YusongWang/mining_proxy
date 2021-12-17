use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::{mpsc::UnboundedSender, Mutex};
use anyhow::Result;

use super::worker::Worker;

#[derive(Debug)]
pub struct Pool {
    pub config: crate::util::config::Settings,
    pub fee_sender: UnboundedSender<String>,
    pub dev_fee_sender: UnboundedSender<String>,
    pub workers: Arc<Mutex<VecDeque<Worker>>>,
}

impl Pool {
    pub fn new(
        config: crate::util::config::Settings,
        fee_sender: UnboundedSender<String>,
        dev_fee_sender: UnboundedSender<String>,
    ) -> Self {
        Self {
            config,
            fee_sender,
            dev_fee_sender,
            workers: Default::default(),
        }
    }


    pub async fn serve(&mut self) -> Result<()>{
        
        Ok(())
    }
}
